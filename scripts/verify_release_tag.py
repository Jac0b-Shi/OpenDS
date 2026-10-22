"""Fail closed unless an annotated release tag has the authorized GPG signature."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import tomllib

OWNER_FINGERPRINT = "B801AC543CB5B2BBF9407DB2EDDF1F5FEF954168"
SEMVER = re.compile(
    r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
    r"(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?"
    r"(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?"
)


def command(*args, env=None):
    result = subprocess.run(args, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if result.returncode:
        raise ValueError(result.stderr.decode(errors="replace").strip() or f"Command failed: {args[0]}")
    return result.stdout.decode("utf-8", errors="replace").strip()


def verify(tag, event_sha, key_file, fingerprint=OWNER_FINGERPRINT, event_tag_object=None):
    version = tag.removeprefix("v")
    match = SEMVER.fullmatch(version)
    if not tag.startswith("v") or not match:
        raise ValueError("Release tag must be a v-prefixed semantic version.")
    prerelease = match.group(4)
    if prerelease and any(p.isdigit() and len(p) > 1 and p.startswith("0") for p in prerelease.split(".")):
        raise ValueError("Numeric prerelease identifiers must not have leading zeroes.")
    ref = "refs/tags/" + tag
    if command("git", "cat-file", "-t", ref) != "tag":
        raise ValueError("Release requires a signed annotated tag; lightweight tags are rejected.")
    tag_object = command("git", "rev-parse", ref)
    headers = dict(line.split(" ", 1) for line in command("git", "cat-file", "-p", ref).split("\n\n", 1)[0].splitlines())
    if headers.get("type") != "commit" or headers.get("tag") != tag:
        raise ValueError("Signed tag must directly name a commit and match the release tag name.")
    commit = headers["object"]
    if commit != event_sha:
        raise ValueError("Tag target differs from the triggering commit.")
    # Push payloads can identify either the annotated object or its commit.
    # Pin the verified object in outputs and recheck it immediately at publish.
    if event_tag_object and event_tag_object not in (tag_object, commit):
        raise ValueError("Tag object moved after the push event.")
    command("git", "merge-base", "--is-ancestor", commit, "refs/remotes/origin/main")
    manifest = tomllib.loads(command("git", "show", commit + ":Cargo.toml"))
    if manifest["package"]["version"] != version:
        raise ValueError("Release tag version differs from Cargo.toml.")

    # Isolate verification from existing trust databases and private keys.
    temp_root = Path(os.environ.get("RUNNER_TEMP", tempfile.gettempdir())).resolve()
    with tempfile.TemporaryDirectory(prefix="opends-tag-", dir=temp_root) as home:
        assert Path(home).resolve().parent == temp_root
        Path(home).chmod(0o700)
        env = os.environ.copy()
        env["GNUPGHOME"] = home
        env["GIT_CONFIG_NOSYSTEM"] = "1"
        command("gpg", "--batch", "--import", str(Path(key_file).resolve()), env=env)
        keys = command("gpg", "--batch", "--with-colons", "--fingerprint", "--list-keys", env=env)
        if fingerprint not in {line.split(":")[9].upper() for line in keys.splitlines() if line.startswith("fpr:")}:
            raise ValueError("Public key set does not contain the pinned release key.")
        result = subprocess.run(
            ["git", "-c", "gpg.format=openpgp", "-c", "gpg.program=gpg", "verify-tag", "--raw", ref],
            env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        )
        status = (result.stdout + result.stderr).decode(errors="replace")
        signatures = [line.split() for line in status.splitlines() if line.startswith("[GNUPG:] VALIDSIG ")]
        if result.returncode or len(signatures) != 1:
            raise ValueError("GPG signature verification failed.")
        fields = signatures[0]
        if fingerprint not in (fields[2].upper(), fields[-1].upper()):
            raise ValueError("Tag was signed by an unauthorized key.")
    return {"version": version, "prerelease": bool(prerelease), "commit": commit,
            "tag_object": tag_object, "tag": tag, "fingerprint": fingerprint}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--event-sha", required=True)
    parser.add_argument("--key-file", required=True)
    parser.add_argument("--event-file", type=Path)
    args = parser.parse_args()
    event_object = None
    if args.event_file:
        event = json.loads(args.event_file.read_text(encoding="utf-8"))
        if event.get("deleted") or event.get("ref") != "refs/tags/" + args.tag or not event.get("after"):
            raise SystemExit("A non-deletion tag push event is required.")
        event_object = event["after"]
    try:
        result = verify(args.tag, args.event_sha, args.key_file, event_tag_object=event_object)
    except (ValueError, KeyError) as error:
        raise SystemExit(str(error)) from error
    print(json.dumps(result, indent=2))
    if output := os.environ.get("GITHUB_OUTPUT"):
        with open(output, "a", encoding="utf-8") as file:
            for name, value in result.items():
                file.write(f"{name}={str(value).lower() if isinstance(value, bool) else value}\n")


if __name__ == "__main__":
    main()
