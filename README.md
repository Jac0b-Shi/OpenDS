# OpenDS

**English** | [简体中文](README.zh-CN.md)

A lightweight DualSense Bluetooth bridge for Windows. It maps a real Bluetooth controller to a local virtual USB DualSense and relays the game's output — lightbar, adaptive triggers, rumble, speaker audio and native haptics — back to the physical pad. The settings UI is drawn with NWG native controls and GDI; no WebView is involved.

## Getting Started

1. **Extract `OpenDS-…-windows-x64.zip` to a local disk in full**, keeping `opends.exe`, both DLLs and the `scripts` directory together. For a fixed installation, right-click and run `scripts/Install-OpenDS.ps1` with PowerShell; it installs under your user profile and creates a Start Menu shortcut.
2. Pair the DualSense in Windows Bluetooth settings. Close any other bridge tool that claims the same controller.
3. Double-click `opends.exe`. If dependencies are missing, the app opens the Diagnostics page where you can launch the official USB/IP Win2 and HidHide installers and follow their prompts to reboot. Driver installation requires administrator rights; everyday use does not.
4. In **Connection & Feedback**, select the controller and save, then return to Overview and click **Start Bridge**. Setting changes take effect the next time the bridge starts.
5. Enable native DualSense support in the game. If Steam Input takes over the controller, native audio and haptics may never reach the virtual device. OpenDS forwards the feedback a game actually produces; it cannot synthesize native haptics for games that do not support them.

Closing the settings window keeps OpenDS in the tray. Stopping the bridge or exiting from the tray releases held keys, tears down feedback, removes the virtual device, and reverts the hiding state this session added.

## Dependency Download and Offline Install

The install button drives `scripts/Install-Drivers.ps1`. It downloads only from pinned official release URLs, verifies SHA-256 and the Authenticode publisher, and then runs the vendor installer. USB/IP is installed in compact mode, skipping the GUI, SDK and debug symbols.

Downloads honor `HTTPS_PROXY` / `HTTP_PROXY` / `ALL_PROXY` or the Windows system proxy. **If no proxy is detected the script stops and prints manual download URLs instead.** A proxy can also be passed explicitly:

```powershell
.\scripts\Install-Drivers.ps1 -Proxy http://127.0.0.1:7890
.\scripts\Install-Drivers.ps1 -ManualDirectory D:\Downloads -Install
```

The first form only downloads and verifies; it does not install. File names, URLs and digests for manual downloads are listed in `scripts/drivers.json`. A failed or cancelled install will not fall through to starting the bridge — reopen OpenDS and check again afterwards. Release archives already bundle libVIIPER and Opus; you do not need to hunt for DLLs separately.

## Settings and Recovery

The default configuration lives at `%LOCALAPPDATA%\OpenDS\opends.ini`. For a portable setup, place `opends.ini` next to the executable. Paths do not depend on the terminal's working directory. "Start the bridge when OpenDS opens" and "Run in the tray at Windows sign-in" are configured independently. Sign-in startup uses the current user's Run key and needs no administrator rights; background mode waits for a Bluetooth controller without showing the main window. Install to a fixed directory before enabling startup, so the entry does not point at a temporary extraction folder.

During installation you can pass `scripts/Install-OpenDS.ps1 -Startup`; from a fixed install directory you can also run `opends.exe startup enable`, `startup disable`, or `startup status`. Running `opends.exe tray` enters the same background mode manually.

The HidHide page exposes the global toggle and an application whitelist. Program paths are converted automatically to the NT volume paths the driver requires. Whitelist entries managed by OpenDS cannot be edited while the bridge is running. Only the physical controller from the current connection is hidden; a clean shutdown removes just the entries this session added and leaves your other configuration untouched.

After an abnormal exit, the next launch repairs state automatically. To recover manually, exit every OpenDS instance first and then run:

```powershell
.\opends.exe restore
```

Or run `scripts/Restore-Visibility.ps1`. The recovery journal is at `%LOCALAPPDATA%\OpenDS\hidhide-state.txt`. If the driver fails or read-back mismatches, the journal is kept so recovery can be retried — **do not delete it manually**. Killing the process or losing power prevents immediate recovery; run the program again to fix it.

## Feedback and Audio

- Compatibility and full input reports are mapped separately; full reports are CRC-verified; disconnects release held keys, and stop/reconnect is supported.
- Lightbar, both adaptive triggers and classic rumble are forwarded. Native haptics keep independent left/right channels, and triggers and lighting keep updating while audio is paused.
- Speaker audio and native haptics can be disabled independently. Routing can follow the game or headset plug state automatically, or be pinned to speaker or headset. Windows' default audio device is never modified.
- The microphone is an **experimental feature, off by default**. When enabled it decodes Bluetooth Opus into virtual USB microphone frames. Reports are demultiplexed first so audio packets cannot corrupt stick input. That Bluetooth capture state may persist until the connection ends; to stop physical capture completely after disabling the option, stop the bridge and reconnect the controller.
- A standard DualSense keeps its own identity; Edge uses the Edge PID by default, and `edge_as_ds=true` in the configuration presents it as a standard DualSense instead. Only one controller is supported at a time.
- DSX UDP, DSX autostart and Xbox/DS4 appearances are not implemented; enabling the corresponding legacy settings produces an explicit error.

## Verification Scope

Automated tests cover report mapping and CRC, left/right haptics, queue boundaries and reset, output-thread warm-up and pause, real Windows asynchronous I/O cancellation, recovery-journal retention, atomic configuration writes and multi-instance exclusion. The binary embeds a Per-Monitor V2 manifest and rebuilds fonts and layout per display DPI; layout math is tested at 100% / 150% / 200% / 250%.

On real hardware the virtual USB device has been verified for gyroscope data, lightbar, adaptive triggers, classic rumble, independent left/right haptic channels, haptic PCM playback and speaker output. Microphone capture, headset routing, all adaptive-trigger modes and cross-monitor scaling still need hardware acceptance. Device identity fields are aligned with the real USB form and were read back through the Windows HID API for comparison.

Windows keeps audio endpoints and channel configuration per device instance. Connect the bridge before launching the game, and confirm the endpoint is four-channel with all channels set to full range; do not change USB identity between runs. `scripts/Check-AudioIdentity.ps1 -OutputPath baseline.json` saves a read-only snapshot, and `-CompareTo baseline.json` checks after reconnecting whether endpoints and settings were preserved. The tool never changes the default audio output. Clean up stale device instances after closing the game; renaming a device does not merge instances.

## Commands and Build

```powershell
.\opends.exe doctor          # read-only dependency diagnostics
.\opends.exe list            # enumerate Bluetooth controllers
.\opends.exe bridge          # run until Ctrl+C
.\opends.exe bridge 30       # run for at most 30 seconds
cargo test --locked --offline
.\scripts\Build-Release.ps1
```

Building requires the Rust MSVC toolchain, the Windows SDK (including the resource compiler) and Git. Fetching Rust dependencies for the first time needs a working proxy; run `cargo fetch --locked` first. The default build uses the dynamic Opus that ships with the package. The optional `bundled-opus` backend compiles a static Opus with CMake — CMake 4 needs `$env:CMAKE_POLICY_VERSION_MINIMUM='3.5'`. Both backends apply the complexity setting.

Sources for the bundled VIIPER, including local modifications, live in `vendor/viiper-source`. Rebuilding the DLL additionally needs Go 1.27.3 and a Windows C compiler; use `scripts/Build-VIIPER.ps1 -Compiler <path-to-gcc.exe> -Proxy <proxy-url>`, or `-Offline` when dependencies are already cached. Then run the release script so the new DLL is packaged together with OpenDS. These build tools are development-only and are not runtime dependencies.

This is a local test release; the OpenDS executable is not Authenticode-signed. Driver installers carry their upstream vendor signatures. The license is GPL-3.0-or-later; the full license text, third-party notices and corresponding source archive ship with the release artifacts. There is no auto-update and no telemetry.

Release executables statically link the VC runtime, so no VC++ Redistributable is required. Exit the old OpenDS from the tray before starting a new version.

## License

This project is licensed under the **GNU General Public License v3.0 or later**
(SPDX: `GPL-3.0-or-later`). See [LICENSE](LICENSE) for the full text.

This project uses `GPL-3.0-or-later`, matching the bundled VIIPER:
downstream may use it under version 3 of the GPL or any later version.

Third-party components and their licenses are listed in [THIRD-PARTY.md](THIRD-PARTY.md)
and under `licenses/`. When distributing binaries you must also provide
`vendor/viiper-source` (the GPL-3.0 corresponding source); the release script
packages it automatically.
