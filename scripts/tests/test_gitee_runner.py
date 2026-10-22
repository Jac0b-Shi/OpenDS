"""Exercise the Gitee Ubuntu runner without credentials or package installation."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
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
        self.log = self.root / "commands"
        self.env = os.environ.copy()
        self.env.update(PATH=str(self.bin), WORKSPACE=str(self.workspace), TEST_BIN=str(self.bin),
                        TEST_LOG=str(self.log), OPENDS_GITEE_TOKEN="fixture-secret-never-log")
        self.bash = shutil.which("bash")
        for name in ("env", "id"):
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

    def run_runner(self):
        result = subprocess.run([self.bash, "-x", str(ROOT / "scripts/Run-GiteeRelease.sh")],
                                cwd="/", env=self.env, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
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
        self.env["WORKSPACE"] = str(self.bin)
        result = self.run_runner()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("checkout is missing", result.stderr)
        self.assertEqual(self.commands(), "")


if __name__ == "__main__":
    unittest.main()
