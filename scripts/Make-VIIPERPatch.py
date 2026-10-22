"""Create and verify the VIIPER patch from a local official v1.0.1 checkout.

No network access is performed. Python 3.12+ and Git are required.
"""
import argparse
import io
from pathlib import Path
import subprocess
import tarfile
import tempfile
import shutil

ROOT = Path(__file__).resolve().parents[1]
UPSTREAM = "25515e1ef405f189b73d5e2707a4fbb0798e9110"


def run(*args, cwd=None):
    return subprocess.run(args, cwd=cwd, check=True, stdout=subprocess.PIPE).stdout


def canonical(data):
    if b"\0" not in data:
        try:
            data.decode("utf-8")
            return data.replace(b"\r\n", b"\n")
        except UnicodeDecodeError:
            pass
    return data


def tree(path):
    return {
        p.relative_to(path).as_posix(): canonical(p.read_bytes())
        for p in path.rglob("*")
        if p.is_file() and ".git" not in p.relative_to(path).parts
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", required=True, type=Path)
    parser.add_argument("--check", action="store_true", help="Verify the bundled patch without regenerating its text")
    args = parser.parse_args()
    upstream = args.upstream.resolve()
    commit = run("git", "rev-parse", "HEAD", cwd=upstream).decode().strip()
    if commit != UPSTREAM:
        raise SystemExit(f"Expected VIIPER v1.0.1 commit {UPSTREAM}, got {commit}")
    archive = run("git", "archive", "HEAD", cwd=upstream)
    (ROOT / "target").mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="viiper-patch-", dir=ROOT / "target") as temp:
        temp = Path(temp)
        baseline, modified, applied = (temp / name for name in ("a", "b", "applied"))
        with tarfile.open(fileobj=io.BytesIO(archive)) as source:
            source.extractall(baseline, filter="data")
        shutil.copytree(ROOT / "vendor/viiper-source", modified)
        output = ROOT / "vendor/opends-viiper.patch"
        if args.check:
            patch_bytes = output.read_bytes()
        else:
            result = subprocess.run(
                ["git", "-c", "core.autocrlf=false", "diff", "--no-index",
                 "--no-color", "--no-ext-diff", "--no-textconv", "--full-index",
                 "--ignore-space-at-eol", "--src-prefix=", "--dst-prefix=", "a", "b"],
                cwd=temp, stdout=subprocess.PIPE,
            )
            if result.returncode != 1:
                raise SystemExit(f"Expected a nonempty diff, exit code {result.returncode}")
            patch_bytes = result.stdout
        patch = temp / "opends-viiper.patch"
        patch.write_bytes(patch_bytes)
        shutil.copytree(baseline, applied)
        # Isolate from OpenDS's parent repository so git apply cannot skip paths.
        run("git", "init", "--quiet", cwd=applied)
        run("git", "apply", "--check", str(patch), cwd=applied)
        run("git", "apply", str(patch), cwd=applied)
        expected, actual = tree(modified), tree(applied)
        differences = sorted(n for n in expected.keys() | actual.keys()
                             if expected.get(n) != actual.get(n))
        if differences:
            raise SystemExit("Patched tree differs: " + ", ".join(differences))
        if not args.check:
            output.write_bytes(patch_bytes)
        print(f"Verified patch: {len(actual)} files match (UTF-8 CRLF/LF normalized).")
        print(f"Upstream commit: {commit}; patch bytes: {output.stat().st_size}")


if __name__ == "__main__":
    main()
