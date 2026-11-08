# OpenDS on Ubuntu

OpenDS has a native Linux backend and a GTK desktop application. It keeps the shared DualSense report,
feedback, Opus audio, haptics, microphone, input-history and CSV recording code.
Windows builds continue to use the existing Windows GUI and drivers.

## Install and run

Install stable Rust, Go 1.27.3 or newer, and `build-essential`, then run from the
project directory:

```bash
./scripts/build-linux.sh
./scripts/setup-ubuntu.sh  # one-time Ubuntu administrator authentication
./opends doctor
python3 scripts/install-linux.py  # install for your account, including Show Apps
./opends                  # or open the desktop window from this checkout
```

After installation, open **Show Apps**, search for **OpenDS** and launch it. The
application and its icon are installed under `~/.local`; it opens without a terminal.
Reinstall after rebuilding to update the installed copy.

Pair your DualSense in Ubuntu's Bluetooth settings. Disconnect its USB cable,
hold Create and PS until the light bar flashes, then select Wireless Controller.
Click Refresh in OpenDS, choose the controller, save settings and start the bridge
before starting a game. Ubuntu asks for administrator authentication when USB/IP
attaches or detaches the virtual device. Keep OpenDS running while playing.

The build uses Rust and Go from `PATH`. If you maintain project-local toolchains
in the ignored `.tools/` directory, `./scripts/build-linux.sh` recognizes them
automatically; downloaded toolchains are not included in the repository.

## Building another checkout

Install stable Rust, Go 1.27.3 or newer, and `build-essential`. Then:

```bash
./scripts/build-linux.sh
./scripts/setup-ubuntu.sh
./opends doctor
```

The build produces `target/release/opends` and `target/release/libVIIPER.so`.
The installer copies the binary, shared library, graphics module, icon and setup
script into `~/.local/lib/opends`, and registers
`~/.local/share/applications/io.github.OpenDS.desktop`. It also creates
`~/.local/bin/opends`. Run `python3 scripts/install-linux.py --prefix /some/path`
for a different installation prefix. Settings are preserved when updating.

The desktop window uses Ubuntu's `/usr/bin/python3` with GTK 3, Cairo and Ayatana
AppIndicator; it does not depend on Conda or its Python packages. The bridge,
live history, classification and recording run in the same shared Rust code as
the Windows version, controlled by a local stdin/stdout desktop service. The setup
script installs missing Ubuntu runtime packages, loads `vhci_hcd`, enables it at
boot and installs `/etc/udev/rules.d/70-opends.rules`. Access rules apply only to
physical Bluetooth DualSense and DualSense Edge devices. Reconnect the pad after
setup. USB/IP can require kernel-matching `linux-tools` and `linux-modules-extra`
packages; the script installs these when missing.

The bundled VIIPER source is compiled as a Linux shared library; Windows DLLs,
USB/IP Win2 and HidHide are not used on Ubuntu. Dynamic audio loads Ubuntu's
`libopus.so.0`. The optional `bundled-opus` Cargo feature remains available.

## Terminal operation

```bash
./opends list
./opends probe
./opends bridge             # Ctrl+C or SIGTERM stops and cleans up
./opends bridge 30          # stop after 30 seconds
./opends bridge --record /tmp/session.csv
./opends config path
./opends config init
./opends config check /tmp/edited.ini
./opends config import /tmp/edited.ini
```

Recording also writes a matching `.commands.csv`. Use a new filename for each
session. The window's Advanced configuration tab exposes the shared INI settings
and validates them before saving. Stop the bridge before changing settings.

Configuration defaults to `$XDG_CONFIG_HOME/opends/opends.ini`, or
`~/.config/opends/opends.ini`. An existing `opends.ini` beside the binary takes
precedence. Logs are in `$XDG_STATE_HOME/opends/logs/opends.log`, or
`~/.local/state/opends/logs/opends.log`, with the existing bounded rotation.
Language packs belong in the configuration directory's `locales/` subdirectory.

## Linux behavior and limits

The desktop follows the Windows application's six-page layout: Overview, Live
monitor, Connection & feedback, Device hiding, Diagnostics & logs, and General
settings. GTK provides native Ubuntu window decorations and controls.

- **Overview:** bridge controls, connection details, input/output/audio counters,
  battery status and recent activity.
- **Live monitor:** the Windows controller geometry, real buttons, sticks,
  triggers, touchpad, motion and Edge controls; source/forwarded speaker, haptics
  and microphone peak envelopes; independent capture, clear, sampling, refresh
  and history controls; CSV and command-event recording.
- **Separate monitor:** detach and dock the same live canvas, with always-on-top.
- **Input history:** per-stick/trigger thresholds, dead zones and strength modes,
  source text/icons/hidden labels, light/dark colors, content size, duration
  filtering and record separators, using the shared Windows classification.
- **Overlay:** independent transparent input-history window, separate background
  and content opacity, fixed/automatic rows, topmost, lock/click-through and
  restore controls. Lock/unlock is available both in the monitor and tray. X11
  or XWayland is needed for unrestricted positioning and click-through.
- **Connection & feedback:** controller selection, audio routing and volume,
  haptic/rumble gains, Opus complexity, reconnect, hiding and microphone gain and
  jitter-buffer settings. Restart bridging after changing connection options.
- **Microphone test:** record five seconds after a three-second countdown; capture
  the selected virtual USB microphone through PulseAudio/PipeWire and the decoded
  Bluetooth source in parallel, then replay either through the default output.
  Enable microphone forwarding, start bridging and select the virtual controller's
  capture source first. Test audio is cleared on closing the test/application.
- **General settings:** system/light/dark appearance, English/Chinese and imported
  Fluent packs, close behavior, automatic bridging and Ubuntu login startup.
  Language changes require restart; missing translations fall back to English.
- **Tray:** start/stop bridging, open settings, show/lock/restore overlay and exit.

The Windows driver installers and HidHide application allowlist are replaced by
Ubuntu dependency setup and Linux input-grab controls; those OS-specific driver
interfaces are not available on Linux.

Physical-controller hiding uses exclusive evdev grabs. The kernel releases these
when OpenDS exits, including after a crash. Applications that open the physical
`hidraw` interface directly can bypass evdev grabs. Disable Steam Input and, for
SDL games, set the game's environment variable `SDL_JOYSTICK_HIDAPI=0`. A Steam
launch option is `SDL_JOYSTICK_HIDAPI=0 %command%`. Set `hide_device=false` if you
intentionally need to share the physical evdev device.

Audio, rumble and adaptive-trigger support still depends on the game. Windows
games running through Wine/Proton may need game-specific configuration; the Linux
port does not add those capabilities to games that lack them. Controller features
need acceptance testing with a real paired pad.

For startup, enable **Run in the tray when Ubuntu starts** in General settings.
This creates `~/.config/autostart/io.github.OpenDS.desktop` for the current user.
USB/IP still needs administrator authentication when a controller connects.
There is no passwordless privilege policy installed by this project.

## Troubleshooting

- **No controller:** pair it over Bluetooth, remove its USB cable and run
  `./opends list`. USB and virtual pads are deliberately excluded from discovery.
- **Permission denied:** rerun setup, reconnect the pad and ensure you are in an
  active local desktop session so udev's `uaccess` ACL applies.
- **Missing vhci_hcd:** run setup and accept Ubuntu's administrator prompt.
- **USB/IP attach fails:** check `usbip version`, `lsmod | rg vhci` and
  `./opends doctor`. Linux USB/IP ports start at zero.
- **Duplicate controllers:** disable Steam Input and SDL HIDAPI as described above.
- **No desktop display:** use `./opends bridge` from a terminal. The GUI requires
  an X11 or XWayland session and Ubuntu's GTK/Python packages.

## Validation

```bash
export CARGO_HOME="$PWD/.tools/cargo" RUSTUP_HOME="$PWD/.tools/rustup"
export PATH="$CARGO_HOME/bin:$PATH"
cargo test --locked
cargo test --locked viiper::tests -- --ignored --skip linux_kernel_attachment
OPENDS_UI_SMOKE=1 xvfb-run -a ./opends gui
python3 scripts/tests/test_linux_desktop.py
xvfb-run -a /usr/bin/python3 scripts/tests/linux_ui_smoke.py
```

The explicitly invoked VIIPER tests load the native library and start a loopback
server. They do not attach kernel devices or require a controller.
The separate `linux_kernel_attachment` ignored test attaches a synthetic pad and
detaches it again; run it explicitly only when kernel USB/IP setup is complete.
`python3 scripts/tests/test_linux.py` checks the desktop window, configuration,
library loading and, when `vhci_hcd` is loaded without a physical Sony controller,
timed bridge operation, CSV finalization, SIGTERM cleanup and instance locking.
