"""Desktop RPC and install checks; no physical device or administrator access."""
import json
import os
from pathlib import Path
import queue
import subprocess
import tempfile
import threading
import time
import unittest

ROOT = Path(__file__).resolve().parents[2]
EXE = ROOT / "target/release/opends"


class DesktopService(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="opends desktop 中文 ")
        self.addCleanup(self.temp.cleanup)
        self.env = {**os.environ, "XDG_CONFIG_HOME": self.temp.name, "XDG_STATE_HOME": self.temp.name, "LC_ALL": "C.UTF-8"}
        self.process = subprocess.Popen([str(EXE), "service"], env=self.env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        self.addCleanup(self.close)
        self.messages = queue.Queue()
        def read():
            for line in self.process.stdout:
                self.messages.put(json.loads(line))
        self.reader = threading.Thread(target=read, daemon=True)
        self.reader.start()
        self.ready = self.until(lambda message: message["type"] == "ready")
        self.number = 0

    def close(self):
        if self.process.poll() is None:
            self.process.stdin.write('{"op":"quit"}\n')
            self.process.stdin.flush()
            self.process.wait(timeout=10)
        self.reader.join(timeout=2)
        self.process.stdin.close()
        self.process.stdout.close()
        self.process.stderr.close()

    def until(self, predicate):
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            message = self.messages.get(timeout=max(.01, deadline-time.monotonic()))
            if predicate(message):
                return message
        raise AssertionError("Desktop service response timed out")

    def request(self, operation, ok=True, **values):
        self.number += 1
        self.process.stdin.write(json.dumps({"id": self.number, "op": operation, **values}) + "\n")
        self.process.stdin.flush()
        message = self.until(lambda m: m["type"] == "reply" and m["id"] == self.number)
        self.assertEqual(message["ok"], ok, message)
        return message

    def test_monitor_recording_settings_and_localized_desktop_protocol(self):
        self.assertIn("gui-overview", self.ready["strings"])
        self.assertIn("gui-overview", self.ready["english_strings"])
        self.assertIn("zh-CN", [language["code"] for language in self.ready["languages"]])
        self.request("configure", text="[opends]\nmonitor_sample_hz=200\ncommand_right=true\ncommand_bg_alpha=35\n")
        self.request("capture_start")
        state = self.until(lambda m: m["type"] == "state" and m["capture"])
        self.assertEqual(len(state["waves"]), 240)
        self.assertEqual(len(state["input"]["sticks"]), 4)
        self.assertEqual(state["counts"], [0, 0, 0])
        self.request("signal", forwarded=True, enabled=True)
        self.until(lambda m: m["type"] == "state" and m["forwarded"])
        path = Path(self.temp.name) / "输入记录.csv"
        self.request("record_start", path=str(path))
        self.until(lambda m: m["type"] == "state" and m["recording"]["active"] and not m["recording"]["starting"])
        time.sleep(.08)
        self.request("record_stop")
        self.until(lambda m: m["type"] == "state" and not m["recording"]["active"] and not m["recording"]["finishing"])
        self.request("history_clear")
        self.request("capture_stop")
        self.assertTrue(path.is_file())
        self.assertTrue(path.with_suffix(".commands.csv").is_file())
        self.assertGreater(len(path.read_text().splitlines()), 1)
        config = Path(self.ready["config"])
        previous = config.read_text()
        self.request("configure", ok=False, text="[opends]\ncommand_rows=0\n")
        self.assertEqual(config.read_text(), previous)
        self.request("mic_capture", ok=False)
        self.request("mic_cancel")
        self.request("visible", value=False)
        self.request("quit")
        self.assertEqual(self.process.wait(timeout=8), 0)

    def test_show_apps_install_is_valid_and_independent_of_checkout(self):
        prefix = Path(self.temp.name) / "installed app"
        subprocess.run(["/usr/bin/python3", str(ROOT / "scripts/install-linux.py"), "--prefix", str(prefix)], check=True, capture_output=True)
        entry = prefix / "share/applications/io.github.OpenDS.desktop"
        self.assertTrue(entry.is_file())
        subprocess.run(["desktop-file-validate", str(entry)], check=True)
        executable = prefix / "lib/opends/opends"
        self.assertTrue((executable.parent / "ui/graphics.py").is_file())
        result = subprocess.run([str(executable), "doctor"], env=self.env, cwd="/tmp", capture_output=True, text=True, check=True)
        self.assertIn("VIIPER: available", result.stdout)
        self.assertIn("Icon=opends", entry.read_text())
        self.assertIn("Terminal=false", entry.read_text())


if __name__ == "__main__":
    unittest.main()
