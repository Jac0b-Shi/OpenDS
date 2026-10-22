"""Local HTTP/proxy integration tests; no external network or driver install."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import shutil
import os
import json
import socket
import subprocess
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[2]
PAYLOAD = b"OpenDS local driver download fixture"


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_GET(self):
        if self.path == "/redirect":
            self.send_response(302)
            self.send_header("Location", "http://download.opends.invalid/asset")
            self.end_headers()
            return
        if self.path == "/missing":
            self.send_error(404)
            return
        data = b"untrusted content" if self.path == "/tampered" else PAYLOAD
        self.send_response(200)
        self.send_header("Content-Length", str(len(data) + (100 if self.path == "/truncated" else 0)))
        self.end_headers()
        if self.path == "/stall":
            time.sleep(5)
            return
        self.wfile.write(data)


def main():
    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    with socket.socket() as reserved:
        reserved.bind(("127.0.0.1", 0))
        dead_port = reserved.getsockname()[1]
        # Keep the port reserved but not listening throughout the tests.
        url = f"http://127.0.0.1:{server.server_port}"
        try:
            for shell in ("powershell.exe", "pwsh.exe"):
                executable = shutil.which(shell)
                if not executable:
                    raise SystemExit(f"Required Windows test runtime not found: {shell}")
                (ROOT / "target").mkdir(exist_ok=True)
                with tempfile.TemporaryDirectory(prefix="driver-tests-", dir=ROOT / "target") as temp:
                    environment = os.environ.copy()
                    # A Python child of PS7 otherwise passes PS7 module paths to
                    # Windows PowerShell 5.1, unlike an ordinary desktop launch.
                    environment.pop("PSModulePath", None)
                    subprocess.run(
                        [executable, "-NoProfile", "-NonInteractive", "-File",
                         str(ROOT / "scripts/tests/Driver-Download.Tests.ps1"),
                         "-Origin", url, "-FixtureProxy", url,
                         "-BrokenProxy", f"http://127.0.0.1:{dead_port}",
                         "-OutputDirectory", temp],
                        check=True, timeout=90, env=environment,
                    )
                    # Exercise the real entry point and manifest on both hosts,
                    # without downloading or launching an installer.
                    manual = Path(temp) / "empty-manual"
                    manual.mkdir()
                    assets = json.loads((ROOT / "scripts/drivers.json").read_text())
                    for asset in assets:
                        result = subprocess.run(
                            [executable, "-NoProfile", "-NonInteractive", "-File",
                             str(ROOT / "scripts/Install-Drivers.ps1"),
                             "-Component", asset["id"], "-ManualDirectory", str(manual),
                             "-Cache", str(manual)],
                            env=environment, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                            timeout=30,
                        )
                        if result.returncode == 0 or asset["url"].encode() not in result.stdout:
                            raise SystemExit(f"Manifest/manual mode failed on {shell}: {result.stdout!r}")
        finally:
            server.shutdown()
            server.server_close()


if __name__ == "__main__":
    main()
