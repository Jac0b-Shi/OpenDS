#!/usr/bin/env python3
"""Install the built app for the current user, including the GNOME Show Apps entry."""
import argparse
import os
from pathlib import Path
import shlex
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
APP_ID = "io.github.OpenDS"


def desktop_quote(value):
    return '"' + str(value).replace('\\', '\\\\').replace('"', '\\"').replace('`', '\\`').replace('$', '\\$').replace('%', '%%') + '"'


def copy(source, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = destination.with_name(destination.name + ".installing")
    shutil.copy2(source, temporary)
    temporary.replace(destination)


def install(prefix=None):
    prefix = Path(prefix).expanduser().resolve() if prefix else Path.home() / ".local"
    data = prefix / "share" if prefix != Path.home() / ".local" else Path(os.environ.get("XDG_DATA_HOME", str(prefix / "share")))
    app = prefix / "lib/opends"
    build = ROOT / "target/release"
    required = ["opends", "libVIIPER.so", "opends.png", "setup-ubuntu.sh", "ui/graphics.py"]
    for filename in required:
        if not (build / filename).is_file():
            raise SystemExit(f"Missing {filename}. Run ./scripts/build-linux.sh first.")
    for filename in required:
        copy(build / filename, app / filename)
    for filename in ["LICENSE", "THIRD-PARTY.md"]:
        copy(ROOT / filename, app / filename)
    copy(ROOT / "docs/guides/ubuntu.md", app / "ubuntu.md")
    executable = app / "opends"
    executable.chmod(0o755)
    (app / "setup-ubuntu.sh").chmod(0o755)
    entry = data / "applications" / (APP_ID + ".desktop")
    entry.parent.mkdir(parents=True, exist_ok=True)
    desktop = "\n".join([
        "[Desktop Entry]", "Version=1.0", "Type=Application", "Name=OpenDS",
        "GenericName=DualSense controller bridge", "Comment=DualSense bridge, live input monitor, audio and haptics",
        "Exec=" + desktop_quote(executable) + " gui", "Icon=opends", "Terminal=false",
        "Categories=Game;", "Keywords=DualSense;PlayStation;controller;gamepad;haptics;",
        "StartupNotify=true", "StartupWMClass=" + APP_ID, "",
    ])
    temporary = entry.with_name("." + APP_ID + ".installing.desktop")
    temporary.write_text(desktop, encoding="utf-8")
    if shutil.which("desktop-file-validate"):
        subprocess.run(["desktop-file-validate", str(temporary)], check=True)
    temporary.replace(entry)
    copy(build / "opends.png", data / "icons/hicolor/256x256/apps/opends.png")
    binpath = prefix / "bin/opends"
    binpath.parent.mkdir(parents=True, exist_ok=True)
    binpath.write_text("#!/bin/sh\nexec " + shlex.quote(str(executable)) + ' "$@"\n', encoding="utf-8")
    binpath.chmod(0o755)
    if shutil.which("update-desktop-database"):
        subprocess.run(["update-desktop-database", str(entry.parent)], check=True)
    print(f"Installed OpenDS: {executable}\nShow Apps entry: {entry}\nSearch for OpenDS in Show Apps.")
    return executable, entry


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prefix", help="Optional installation prefix (default: ~/.local)")
    install(parser.parse_args().prefix)
