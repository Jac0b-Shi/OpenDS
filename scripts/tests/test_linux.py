"""End-to-end Linux CLI/settings/UI checks without root or a physical controller."""
import os
from pathlib import Path
import subprocess
import signal
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[2]
EXE = ROOT / "target/release/opends"


class LinuxRuntime(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="opends-linux-中文 空格-")
        self.addCleanup(self.directory.cleanup)
        self.env = {**os.environ, "XDG_CONFIG_HOME": self.directory.name,
                    "XDG_STATE_HOME": self.directory.name, "LC_ALL": "C.UTF-8"}

    def run_cli(self, *args, ok=True):
        result = subprocess.run([str(EXE), *args], env=self.env, cwd="/tmp",
                                capture_output=True, text=True, timeout=20)
        if ok:
            self.assertEqual(result.returncode, 0, result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0)
        return result

    def test_config_validation_and_atomic_import_preserve_previous_settings(self):
        self.run_cli("config", "init")
        path = Path(self.run_cli("config", "path").stdout.strip())
        original = path.read_text()
        candidate = Path(self.directory.name) / "candidate.ini"
        # Use a known supported setting and reject unsupported DSX emulation.
        candidate.write_text("[opends]\nbuffer=32\naudio_enabled=false\n")
        self.run_cli("config", "import", str(candidate))
        self.assertIn("buffer=32\n", path.read_text())
        self.assertIn("audio_enabled=false\n", path.read_text())
        saved = path.read_text()
        self.assertNotEqual(original, saved)
        candidate.write_text("[opends]\ndsx_udp=true\n")
        self.run_cli("config", "import", str(candidate), ok=False)
        self.assertEqual(saved, path.read_text())
        self.run_cli("config", "check", "/tmp/opends-does-not-exist.ini", ok=False)

    def test_linux_runtime_loads_libraries_from_an_unrelated_working_directory(self):
        report = self.run_cli("doctor").stdout
        self.assertIn("VIIPER: available", report)
        self.assertIn("Opus: available", report)
        self.assertNotIn("usbip.exe", report)
        self.assertIn("Linux", self.run_cli("--help").stdout)
        self.run_cli("bridge", "-1", ok=False)
        self.run_cli("bridge", "--record", ok=False)
        self.run_cli("not-a-command", ok=False)

    def test_gui_opens_and_closes_cleanly(self):
        env = {**self.env, "OPENDS_UI_SMOKE": "1"}
        result = subprocess.run(["xvfb-run", "-a", str(EXE), "gui"], env=env,
                                capture_output=True, text=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn("Traceback", result.stderr)

    @unittest.skipUnless(Path("/sys/module/vhci_hcd").exists(), "requires vhci_hcd")
    def test_idle_bridge_recording_signal_shutdown_and_instance_lock(self):
        # Exercise waiting/reconnect without touching a real controller.
        ids = Path("/sys/class/hidraw").glob("*/device/uevent")
        if any("0005:0000054C:00000" in p.read_text() for p in ids):
            self.skipTest("A physical Sony Bluetooth controller is connected")
        csv = Path(self.directory.name) / "session.csv"
        result = self.run_cli("bridge", "1", "--record", str(csv))
        self.assertIn("[status]", result.stdout)
        self.assertTrue(csv.is_file())
        self.assertTrue(csv.with_suffix(".commands.csv").is_file())
        child = subprocess.Popen([str(EXE), "bridge"], env=self.env, stdout=subprocess.PIPE,
                                 stderr=subprocess.STDOUT, text=True)
        try:
            deadline = time.monotonic() + 5
            lock = Path(self.directory.name) / "opends/controller.lock"
            while not lock.exists() and time.monotonic() < deadline:
                time.sleep(0.01)
            # Wait for one status line: the instance lock and signal handlers are ready.
            self.assertIn("[status]", child.stdout.readline())
            second = self.run_cli("bridge", "1", ok=False)
            self.assertIn("already running", second.stderr)
            child.send_signal(signal.SIGTERM)
            child.communicate(timeout=10)
            self.assertEqual(child.returncode, 0)
            self.run_cli("bridge", "1")  # lock released after shutdown
        finally:
            if child.poll() is None:
                child.send_signal(signal.SIGTERM)
                child.communicate(timeout=10)


if __name__ == "__main__":
    unittest.main()
