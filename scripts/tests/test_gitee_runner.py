"""Exercise the Gitee Ubuntu runner without credentials or package installation."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import textwrap
import unittest

ROOT = Path(__file__).resolve().parents[2]


@unittest.skipIf(os.name == "nt", "Gitee runner uses Linux shell and package commands")
class RunnerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.workspace = self.root / "workspace"
        (self.workspace / "scripts").mkdir(parents=True)
        (self.workspace / "scripts/mirror_gitee_release.py").touch()
        self.runner = self.workspace / "scripts/Run-GiteeRelease.sh"
        self.runner.write_text((ROOT / "scripts/Run-GiteeRelease.sh").read_text(encoding="utf-8"), encoding="utf-8")
        workflow = (ROOT / ".workflow/GiteeRelease.yml").read_text(encoding="utf-8")
        self.entrypoint = textwrap.dedent(workflow.split("          - |\n", 1)[1])
        self.log = self.root / "commands"
        self.env = os.environ.copy()
        self.env.update(PATH=str(self.bin), WORKSPACE=str(self.workspace), TEST_WORKSPACE=str(self.workspace), TEST_BIN=str(self.bin),
                        TEST_LOG=str(self.log), OPENDS_GITEE_TOKEN="fixture-secret-never-log")
        self.bash = shutil.which("bash")
        for name in ("bash", "env", "id"):
            (self.bin / name).symlink_to(shutil.which(name))
        self.tool("git", "exit 0")
        self.tool("python3", 'printf "publish\\n" >> "$TEST_LOG"')
        self.tool("apt-get", 'printf "apt %s\\n" "$*" >> "$TEST_LOG"\n'
                  'case "$1" in install) /bin/ln -s /bin/true "$TEST_BIN/gpg"; '
                  '/bin/ln -s /bin/true "$TEST_BIN/gpgconf";; esac')
        # Simulate the root container from the provided Gitee log.
        (self.bin / "id").unlink()
        self.tool("id", "echo 0")

    def tool(self, name, script):
        path = self.bin / name
        path.write_text("#!/bin/bash\nset -eu\n" + script + "\n")
        path.chmod(0o700)

    def run_runner(self, *, workflow=False, cwd="/", prelude=""):
        command = [self.bash, "-x", "-c", prelude + self.entrypoint] if workflow else [self.bash, "-x", str(self.runner)]
        result = subprocess.run(command, cwd=cwd, env=self.env, text=True,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.assertNotIn("fixture-secret-never-log", result.stdout + result.stderr)
        return result

    def commands(self):
        return self.log.read_text() if self.log.exists() else ""

    def test_existing_tools_skip_installation(self):
        self.tool("gpg", "exit 0")
        self.tool("gpgconf", "exit 0")
        self.assertEqual(self.run_runner().returncode, 0)
        self.assertEqual(self.commands(), "publish\n")

    def test_missing_gpg_installs_before_publishing(self):
        self.assertEqual(self.run_runner().returncode, 0)
        self.assertEqual(self.commands().splitlines(), ["apt update -qq",
                         "apt install -y --no-install-recommends gnupg ca-certificates", "publish"])

    def test_missing_variable_stops_before_install_or_publish(self):
        self.env.pop("OPENDS_GITEE_TOKEN")
        result = self.run_runner()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Common Variables", result.stderr)
        self.assertEqual(self.commands(), "")

    def test_failed_package_install_stops_before_publishing(self):
        self.tool("apt-get", "exit 23")
        self.assertEqual(self.run_runner().returncode, 23)
        self.assertEqual(self.commands(), "")

    def test_missing_checkout_stops(self):
        (self.workspace / "scripts/mirror_gitee_release.py").unlink()
        result = self.run_runner()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("checkout is missing", result.stderr)
        self.assertEqual(self.commands(), "")

    def test_runner_uses_own_location_without_workspace(self):
        self.env.pop("WORKSPACE")
        self.assertEqual(self.run_runner().returncode, 0)
        self.assertIn("publish\n", self.commands())

    def test_runner_ignores_stale_workspace(self):
        self.env["WORKSPACE"] = str(self.bin)
        self.assertEqual(self.run_runner().returncode, 0)
        self.assertIn("publish\n", self.commands())

    def test_workflow_uses_exported_workspace_from_another_directory(self):
        result = self.run_runner(workflow=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("publish\n", self.commands())

    def test_workflow_uses_checkout_directory_without_workspace(self):
        self.env.pop("WORKSPACE")
        result = self.run_runner(workflow=True, cwd=self.workspace)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("publish\n", self.commands())

    def test_workflow_uses_checkout_directory_with_empty_workspace(self):
        self.env["WORKSPACE"] = ""
        result = self.run_runner(workflow=True, cwd=self.workspace)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_workflow_does_not_need_workspace_exported_to_runner(self):
        self.env.pop("WORKSPACE")
        result = self.run_runner(workflow=True, prelude='WORKSPACE="$TEST_WORKSPACE"\n')
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_workflow_ignores_stale_workspace(self):
        self.env["WORKSPACE"] = str(self.bin)
        result = self.run_runner(workflow=True, cwd=self.workspace)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_workflow_missing_checkout_stops_before_install_or_publish(self):
        self.env.pop("WORKSPACE")
        self.env["GITEE_REPO"] = "fixture-does-not-exist/OpenDS"
        self.env.pop("AGILE_MODULE_NAME", None)
        result = self.run_runner(workflow=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("checkout is missing", result.stderr)
        self.assertEqual(self.commands(), "")


if __name__ == "__main__":
    unittest.main()
