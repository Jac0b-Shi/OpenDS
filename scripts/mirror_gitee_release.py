"""Run on Gitee Go: mirror published GitHub assets after verifying the owner tag.

Python 3.9+, Git and GPG; no third-party Python dependencies. Credentials are
read only from OPENDS_GITEE_TOKEN and sent only in Gitee API request bodies.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid

GITHUB = "Jac0b-Shi/OpenDS"
GITEE = "Jac0b_Shi/OpenDS"
FINGERPRINT = "B801AC543CB5B2BBF9407DB2EDDF1F5FEF954168"
TAG_PATTERN = re.compile(r"v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?")


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        # Authenticated requests must never forward their body to another host.
        return None


class ApiError(ValueError):
    def __init__(self, status):
        super().__init__("Release API returned HTTP " + str(status))
        self.status = status


def request(url, payload=None, content_type="application/json"):
    headers = {"User-Agent": "OpenDS-release-mirror", "Accept": "application/json"}
    if payload is not None:
        if not url.startswith("https://gitee.com/api/v5/repos/" + GITEE + "/"):
            raise ValueError("Authenticated requests are restricted to the Gitee repository.")
        headers["Content-Type"] = content_type
    opener = urllib.request.build_opener(NoRedirect()) if payload is not None else urllib.request.build_opener()
    try:
        with opener.open(urllib.request.Request(url, data=payload, headers=headers), timeout=120) as response:
            return response.read()
    except urllib.error.HTTPError as error:
        # Do not echo response bodies, URLs with signed queries, or credentials.
        raise ApiError(error.code) from None
    except (urllib.error.URLError, TimeoutError, OSError):
        raise ValueError("Release network request failed; retry the Gitee pipeline.") from None


def get_json(url):
    return json.loads(request(url))


def git(*args, cwd=None, env=None):
    result = subprocess.run(["git", *args], cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if result.returncode:
        raise ValueError("Git operation failed: " + args[0])
    return result.stdout.decode("utf-8").strip()


def remote_tag(repository, tag):
    lines = git("ls-remote", "https://" + repository + ".git", "refs/tags/" + tag).splitlines()
    if len(lines) != 1:
        raise ValueError("Signed tag has not reached " + repository + "; retry after code synchronization.")
    return lines[0].split()[0]


def published_releases(tag=None, wait_seconds=900):
    """A mirrored tag can arrive before the GitHub build publishes its assets."""
    if tag:
        tags = [tag]
    else:
        refs = git("ls-remote", "--tags", "--refs", "https://gitee.com/" + GITEE + ".git")
        tags = sorted(line.split()[1].removeprefix("refs/tags/") for line in refs.splitlines())
        tags = [name for name in tags if TAG_PATTERN.fullmatch(name)]
    result = []
    deadline = time.monotonic() + wait_seconds
    for name in tags:
        while True:
            try:
                release = get_json("https://api.github.com/repos/" + GITHUB + "/releases/tags/"
                                   + urllib.parse.quote(name, safe=""))
                if release.get("draft"):
                    raise ValueError("Draft releases cannot be mirrored.")
                result.append(release)
                break
            except ApiError as error:
                if error.status != 404:
                    raise
                if time.monotonic() >= deadline:
                    raise ValueError("GitHub release is not published yet: " + name + "; rerun after its build completes.")
                print("Waiting for GitHub release: " + name, flush=True)
                time.sleep(min(30, max(0, deadline - time.monotonic())))
    return result


def verify_tag(tag, directory):
    """Verify the actual signed object, independently of checkout depth/main history."""
    if not TAG_PATTERN.fullmatch(tag):
        raise ValueError("Invalid version tag.")
    github_object = remote_tag("github.com/" + GITHUB, tag)
    if remote_tag("gitee.com/" + GITEE, tag) != github_object:
        raise ValueError("GitHub and Gitee tag objects differ.")
    repo = directory / "repo"
    git("init", "--bare", str(repo))
    git("fetch", "--depth=1", "https://github.com/" + GITHUB + ".git",
        "refs/tags/" + tag + ":refs/tags/" + tag, cwd=repo)
    ref = "refs/tags/" + tag
    if git("rev-parse", ref, cwd=repo) != github_object or git("cat-file", "-t", ref, cwd=repo) != "tag":
        raise ValueError("Release requires the unchanged signed annotated tag.")
    raw = git("cat-file", "-p", ref, cwd=repo)
    headers = dict(line.split(" ", 1) for line in raw.split("\n\n", 1)[0].splitlines())
    if headers.get("tag") != tag or headers.get("type") != "commit":
        raise ValueError("Signed tag name/type mismatch.")
    home = directory / "gpg"
    home.mkdir(mode=0o700)
    public_key = directory / "public.gpg"
    public_key.write_bytes(request("https://github.com/Jac0b-Shi.gpg"))
    env = os.environ.copy()
    env["GNUPGHOME"] = str(home)
    imported = subprocess.run(["gpg", "--batch", "--import", str(public_key)], env=env,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if imported.returncode:
        raise ValueError("Cannot import release verification public key.")
    try:
        verified = subprocess.run(["git", "-c", "gpg.format=openpgp", "-c", "gpg.program=gpg",
                                   "verify-tag", "--raw", ref], cwd=repo, env=env,
                                  stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        status = (verified.stdout + verified.stderr).decode(errors="replace")
        sigs = [line.split() for line in status.splitlines() if line.startswith("[GNUPG:] VALIDSIG ")]
        if verified.returncode or len(sigs) != 1 or FINGERPRINT not in (sigs[0][2], sigs[0][-1]):
            raise ValueError("Release tag is not signed by the authorized owner.")
    finally:
        subprocess.run(["gpgconf", "--homedir", str(home), "--kill", "gpg-agent"],
                       stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    return {"tag": tag, "commit": headers["object"], "tag_object": github_object,
            "authorized_primary_fingerprint": FINGERPRINT}


def asset_names(tag):
    version = tag[1:]
    return {"OpenDS-" + version + "-windows-x64.zip", "OpenDS-" + version + "-source.zip",
            "SHA256SUMS.txt", "TAG-SIGNATURE.json"}


def verify_assets(release, files, receipt):
    tag = receipt["tag"]
    if release.get("draft") or release.get("tag_name") != tag or set(files) != asset_names(tag):
        raise ValueError("Expected a published release and exactly the binary/source/checksum/receipt set.")
    if json.loads(files["TAG-SIGNATURE.json"]) != receipt:
        raise ValueError("Release signature receipt does not match the verified tag.")
    entries = {}
    for line in files["SHA256SUMS.txt"].decode("ascii").splitlines():
        match = re.fullmatch(r"([0-9a-f]{64}) [ *](?:\./)?([^/\\]+)", line)
        if not match or match[2] in entries:
            raise ValueError("Invalid or duplicate checksum entry.")
        entries[match[2]] = match[1]
    if set(entries) != set(files) - {"SHA256SUMS.txt"}:
        raise ValueError("Checksum manifest must cover both archives and tag receipt.")
    for name, digest in entries.items():
        if hashlib.sha256(files[name]).hexdigest() != digest:
            raise ValueError("Release checksum mismatch: " + name)
    assets = release.get("assets", [])
    if len(assets) != len(files) or {a["name"] for a in assets} != set(files):
        raise ValueError("Unexpected release assets.")
    for asset in assets:
        data = files[asset["name"]]
        if asset.get("size") != len(data) or asset.get("digest") != "sha256:" + hashlib.sha256(data).hexdigest():
            raise ValueError("GitHub asset digest/size mismatch: " + asset["name"])


def download_assets(release):
    files = {}
    for asset in release.get("assets", []):
        name = asset["name"]
        if name not in asset_names(release["tag_name"]) or name in files:
            raise ValueError("Unexpected or duplicate release asset name.")
        expected_url = "https://github.com/" + GITHUB + "/releases/download/" + release["tag_name"] + "/" + name
        if asset["browser_download_url"] != expected_url:
            raise ValueError("Unexpected GitHub asset location.")
        files[name] = request(expected_url)
    return files


def multipart(token, name, data):
    boundary = "opends-" + uuid.uuid4().hex
    body = ("--" + boundary + '\r\nContent-Disposition: form-data; name="access_token"\r\n\r\n' + token
            + "\r\n--" + boundary + '\r\nContent-Disposition: form-data; name="file"; filename="'
            + name + '"\r\nContent-Type: application/octet-stream\r\n\r\n').encode()
    return body + data + ("\r\n--" + boundary + "--\r\n").encode(), "multipart/form-data; boundary=" + boundary


def publish(release, files, receipt, token):
    """Resume partial uploads; never delete or overwrite existing attachments."""
    base = "https://gitee.com/api/v5/repos/" + GITEE + "/releases"
    tag = receipt["tag"]
    # Recheck both remote identities immediately before any write.
    for repository in ("github.com/" + GITHUB, "gitee.com/" + GITEE):
        if remote_tag(repository, tag) != receipt["tag_object"]:
            raise ValueError("Remote tag moved after verification.")
    try:
        target = get_json(base + "/tags/" + urllib.parse.quote(tag, safe=""))
    except ApiError as error:
        if error.status != 404:
            raise
        payload = {"access_token": token, "tag_name": tag, "target_commitish": receipt["commit"],
                   "name": release["name"] or tag, "body": (release.get("body") or "") + "\n\n"
                   + "Source: https://github.com/" + GITHUB + "/releases/tag/" + tag
                   + "\nVerify all four attachments against SHA256SUMS.txt before use.",
                   "prerelease": bool(release["prerelease"])}
        target = json.loads(request(base, json.dumps(payload).encode()))
    if target["tag_name"] != tag or target.get("prerelease", False) != release["prerelease"]:
        raise ValueError("Existing Gitee release metadata differs; manual review required.")
    attachments = base + "/" + str(int(target["id"])) + "/attach_files"
    # Four expected files; refuse surprises or duplicate names instead of replacing.
    existing = get_json(attachments + "?per_page=100")
    by_name = {a["name"]: a for a in existing}
    if len(by_name) != len(existing) or set(by_name) - set(files):
        raise ValueError("Unexpected Gitee attachments; manual review required.")
    # Preflight every existing file before uploading anything else.
    for name, asset in by_name.items():
        url = attachments + "/" + str(int(asset["id"])) + "/download"
        if request(url) != files[name]:
            raise ValueError("Existing Gitee attachment differs: " + name)
    # Archives first, checksum manifest last: its presence signals a complete set.
    for name in sorted(files, key=lambda n: n == "SHA256SUMS.txt"):
        if name in by_name:
            continue
        body, content_type = multipart(token, name, files[name])
        asset = json.loads(request(attachments, body, content_type))
        if request(attachments + "/" + str(int(asset["id"])) + "/download") != files[name]:
            raise ValueError("Gitee upload read-back mismatch: " + name)
    final = get_json(attachments + "?per_page=100")
    if len(final) != len(files) or {a["name"] for a in final} != set(files):
        raise ValueError("Gitee attachment set changed during upload.")
    for repository in ("github.com/" + GITHUB, "gitee.com/" + GITEE):
        if remote_tag(repository, tag) != receipt["tag_object"]:
            raise ValueError("Remote tag moved during upload; manual review required.")
    print("Gitee release verified: " + tag + " (four matching attachments)")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", help="Backfill one version; default: all published GitHub versions")
    parser.add_argument("--dry-run", action="store_true", help="Verify without writing or requiring a Gitee token")
    parser.add_argument("--wait-seconds", type=int, default=900, help="Wait for the GitHub build (default: 900)")
    args = parser.parse_args()
    token = os.environ.get("OPENDS_GITEE_TOKEN", "")
    if not args.dry_run and not token:
        raise SystemExit("Bind secret OPENDS_GITEE_TOKEN in Gitee Go, then rerun this pipeline.")
    if args.tag and not TAG_PATTERN.fullmatch(args.tag):
        raise SystemExit("Invalid version tag.")
    for release in published_releases(args.tag, args.wait_seconds):
        with tempfile.TemporaryDirectory(prefix="opends-gitee-") as work:
            receipt = verify_tag(release["tag_name"], Path(work))
            files = download_assets(release)
            verify_assets(release, files, receipt)
            if args.dry_run:
                print("Verified without publishing: " + receipt["tag"])
            else:
                publish(release, files, receipt, token)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError) as error:
        raise SystemExit(str(error)) from None
