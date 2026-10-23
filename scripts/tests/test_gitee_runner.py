"""Check Gitee attachment preparation locally, without paid runs or network."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import textwrap
import unittest

ROOT = Path(__file__).resolve().parents[2]
OBJECT = "a" * 40


@unittest.skipIf(os.name == "nt", "Gitee runner uses Linux shell commands")
class RunnerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.workspace = self.root / "workspace"
        (self.workspace / "scripts").mkdir(parents=True)
        (self.workspace / "Cargo.toml").write_text('[package]\nname = "opends"\nversion = "0.2.0"\n')
        self.runner = self.workspace / "scripts/Run-GiteeRelease.sh"
        self.runner.write_text((ROOT / "scripts/Run-GiteeRelease.sh").read_text(encoding="utf-8"))
        self.workflow = (ROOT / ".workflow/GiteeRelease.yml").read_text(encoding="utf-8")
        self.entrypoint = textwrap.dedent(self.workflow.split("          - |\n", 1)[1].split("        artifacts:", 1)[0])
        self.assets = self.root / "assets"
        self.assets.mkdir()
        (self.assets / "OpenDS-0.2.0-windows-x64.zip").write_bytes(b"binary fixture")
        (self.assets / "OpenDS-0.2.0-source.zip").write_bytes(b"source fixture")
        self.receipt = self.assets / "TAG-SIGNATURE.json"
        self.receipt.write_text(json.dumps({"tag": "v0.2.0", "tag_object": OBJECT}, indent=2) + "\n")
        self.manifest()
        self.log = self.root / "requests"
        self.env = os.environ.copy()
        self.env.update(PATH=str(self.bin) + os.pathsep + self.env["PATH"],
                        TEST_ASSETS=str(self.assets), TEST_LOG=str(self.log),
                        WORKSPACE=str(self.workspace), TEST_WORKSPACE=str(self.workspace),
                        GITEE_BRANCH="main", OPENDS_GITEE_TOKEN="fixture-secret-never-log")
        self.env.pop("OPENDS_RELEASE_TAG", None)
        self.bash = shutil.which("bash")
        self.tool("git", 'printf "%s\\trefs/tags/v0.2.0\\n" "' + OBJECT + '"')
        for name in ("apt-get", "gpg", "gpgconf", "python3", "cargo"):
            self.tool(name, 'echo "Unexpected runtime dependency" >&2; exit 99')
        curl = self.bin / "curl"
        curl.write_text("#!" + sys.executable + "\n" + textwrap.dedent('''\
            import os, pathlib, shutil, sys
            args = sys.argv[1:]
            url = next(a for a in args if a.startswith("https://"))
            assert url.startswith("https://github.com/Jac0b-Shi/OpenDS/releases/download/v0.2.0/")
            assert args[args.index("--connect-timeout") + 1] == "5"
            assert args[args.index("--max-time") + 1] == "30"
            assert "--retry" not in args and "--insecure" not in args
            with open(os.environ["TEST_LOG"], "a") as log:
                log.write(url + "\\n")
            if os.environ.get("TEST_DOWNLOAD_FAIL"):
                sys.exit(28)
            shutil.copyfile(pathlib.Path(os.environ["TEST_ASSETS"]) / url.rsplit("/", 1)[1],
                            args[args.index("--output") + 1])
            '''))
        curl.chmod(0o700)

    def tool(self, name, script):
        path = self.bin / name
        path.write_text("#!/bin/bash\nset -eu\n" + script + "\n")
        path.chmod(0o700)

    def manifest(self):
        lines = [hashlib.sha256(p.read_bytes()).hexdigest() + "  "
                 + ("" if p.name == "TAG-SIGNATURE.json" else "./") + p.name + "\n"
                 for p in sorted(self.assets.iterdir()) if p.name != "SHA256SUMS.txt"]
        (self.assets / "SHA256SUMS.txt").write_text("".join(lines))

    def run_runner(self, *, workflow=False, cwd="/", prelude=""):
        command = [self.bash, "-x", "-c", prelude + self.entrypoint] if workflow else [self.bash, "-x", str(self.runner)]
        result = subprocess.run(command, cwd=cwd, env=self.env, text=True,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=10)
        self.assertNotIn("fixture-secret-never-log", result.stdout + result.stderr)
        return result

    def assert_not_prepared(self, result):
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertFalse((self.workspace / "GITEE_PARAMS").exists())
        self.assertFalse((self.workspace / "release-assets").exists())

    def test_prepares_exact_four_assets_and_native_parameters(self):
        result = self.run_runner()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual({p.name for p in (self.workspace / "release-assets").iterdir()},
                         {p.name for p in self.assets.iterdir()})
        for source in self.assets.iterdir():
            self.assertEqual(source.read_bytes(), (self.workspace / "release-assets" / source.name).read_bytes())
        self.assertEqual((self.workspace / "GITEE_PARAMS").read_text(),
                         "OPENDS_RELEASE_TAG=v0.2.0\nOPENDS_RELEASE_VERSION=0.2.0\n")
        self.assertEqual(len(self.log.read_text().splitlines()), 4)

    def test_no_token_or_workspace_required(self):
        self.env.pop("WORKSPACE")
        self.env.pop("OPENDS_GITEE_TOKEN")
        self.assertEqual(self.run_runner().returncode, 0)

    def test_workflow_without_workspace(self):
        self.env.pop("WORKSPACE")
        result = self.run_runner(workflow=True, cwd=self.workspace)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_workflow_workspace_not_exported(self):
        self.env.pop("WORKSPACE")
        result = self.run_runner(workflow=True, prelude='WORKSPACE="$TEST_WORKSPACE"\n')
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_workflow_stale_workspace(self):
        self.env["WORKSPACE"] = str(self.bin)
        result = self.run_runner(workflow=True, cwd=self.workspace)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_explicit_tag_overrides_branch(self):
        self.env.update(OPENDS_RELEASE_TAG="v0.2.0", GITEE_BRANCH="v9.0.0")
        self.assertEqual(self.run_runner().returncode, 0)

    def test_tag_trigger_uses_triggered_tag(self):
        self.env["GITEE_BRANCH"] = "v0.2.0"
        (self.workspace / "Cargo.toml").unlink()
        self.assertEqual(self.run_runner().returncode, 0)

    def test_invalid_tag_stops_before_network(self):
        self.env["OPENDS_RELEASE_TAG"] = "../../main"
        self.assert_not_prepared(self.run_runner())
        self.assertFalse(self.log.exists())

    def test_unmirrored_tag_stops_before_download(self):
        self.tool("git", "exit 2")
        self.assert_not_prepared(self.run_runner())
        self.assertFalse(self.log.exists())

    def test_download_failure_has_no_retry_or_output(self):
        self.env["TEST_DOWNLOAD_FAIL"] = "1"
        self.assert_not_prepared(self.run_runner())
        self.assertEqual(len(self.log.read_text().splitlines()), 4)

    def test_corrupted_archive_is_rejected(self):
        (self.assets / "OpenDS-0.2.0-source.zip").write_bytes(b"modified")
        self.assert_not_prepared(self.run_runner())

    def test_unexpected_manifest_path_is_rejected(self):
        with (self.assets / "SHA256SUMS.txt").open("a") as f:
            f.write("0" * 64 + "  ./../../outside\n")
        self.assert_not_prepared(self.run_runner())

    def test_mirrored_tag_identity_must_match_receipt(self):
        self.receipt.write_text(json.dumps({"tag": "v0.2.0", "tag_object": "b" * 40}, indent=2) + "\n")
        self.manifest()
        self.assert_not_prepared(self.run_runner())

    def test_release_tag_must_match_receipt(self):
        self.receipt.write_text(json.dumps({"tag": "v0.1.0", "tag_object": OBJECT}, indent=2) + "\n")
        self.manifest()
        self.assert_not_prepared(self.run_runner())

    def test_native_release_has_four_files_and_no_token_or_rebuild(self):
        self.assertIn("step: release@gitee", self.workflow)
        self.assertIn("allowUpdate: false", self.workflow)
        self.assertIn("stepTimeout: 3", self.workflow)
        self.assertNotIn("OPENDS_GITEE_TOKEN", self.workflow)
        self.assertNotIn("mirror_gitee_release.py", self.workflow)
        self.assertNotIn("cargo build", self.workflow)
        self.assertNotIn("publish@release_artifacts", self.workflow)
        self.assertEqual(self.workflow.count("step: "), 2)
        self.assertEqual(self.workflow.split("        assertFiles:")[1].split("        prerelease:")[0].count("          - "), 4)


if __name__ == "__main__":
    unittest.main()
