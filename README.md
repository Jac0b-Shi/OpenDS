# OpenDS

**English** | [简体中文](README.zh-CN.md)

Turns a Bluetooth DualSense into a local virtual USB DualSense, so games treat it as a wired controller. Lightbar, adaptive triggers, rumble, speaker audio and haptics all come back to the physical pad.

## Quick Start

1. Extract `OpenDS-…-windows-x64.zip` to a local disk and keep every file. For a fixed install, run `scripts/Install-OpenDS.ps1` in PowerShell.
2. Pair the DualSense in Windows Bluetooth settings and close any other tool that claims it. If the controller was paired to another device before, put it in pairing mode first:

   > Make sure the player indicator is off — hold the PS button until it goes dark — and unplug any USB cable. Hold the **Create** button and press the **PS** button until the light bar starts blinking. Then pick the controller in Windows Bluetooth settings.

   These steps come from Sony's [official pairing guide](https://www.playstation.com/en-hk/support/hardware/pair-dualsense-controller-bluetooth/). [Apple's support page](https://support.apple.com/en-euro/HT211813) shows where the Create button sits; the button combination is the same.
3. Double-click `opends.exe`. If drivers are missing, follow the prompt to install USB/IP Win2 and HidHide, then reboot.
4. Select the controller under **Connection & Feedback**, save, then click **Start Bridge** on the overview.
5. Launch the game. Start the bridge first — the controller is only seen as a full device if it is present when the game starts.

Closing the settings window keeps OpenDS in the tray. Stopping the bridge or exiting from the tray releases keys, removes the virtual device and makes the controller visible again.

## Commands

The GUI covers everyday use. The commands below are for scripting and troubleshooting.

```powershell
.\opends.exe              # open the settings window
.\opends.exe tray         # background mode, tray icon only
.\opends.exe bridge       # bridge directly, Ctrl+C to stop
.\opends.exe bridge 30    # bridge for 30 seconds
.\opends.exe list         # list paired Bluetooth controllers
.\opends.exe doctor       # check dependencies
.\opends.exe restore      # restore a hidden controller
```

## Dependencies

Starting with 0.3.2, Diagnostics lets you select installation directories for USB/IP Win2 and HidHide. Selections are saved and checked immediately; Auto clears a manual selection. USB/IP requires `usbip.exe`. The HidHide directory locates its optional configuration client and does not install the driver. Stop bridging before changing directories.

| Component | Purpose |
| --- | --- |
| USB/IP Win2 | creates the virtual USB device |
| HidHide | hides the physical controller so games see one device |

`scripts/Install-Drivers.ps1` pulls both installers from their upstream GitHub releases and verifies the SHA-256 and publisher signature before running them:

| Driver | Upstream repository |
| --- | --- |
| USB/IP Win2 | [vadimgrn/usbip-win2](https://github.com/vadimgrn/usbip-win2) |
| HidHide | [nefarius/HidHide](https://github.com/nefarius/HidHide) |

File names, URLs and digests are listed in `scripts/drivers.json`.

```powershell
.\scripts\Install-Drivers.ps1                          # download
.\scripts\Install-Drivers.ps1 -Install                 # download and install
.\scripts\Install-Drivers.ps1 -Proxy http://127.0.0.1:7890
.\scripts\Install-Drivers.ps1 -ManualDirectory D:\Downloads -Install
```

Release archives already include libVIIPER and Opus.

## Configuration

Settings default to `%LOCALAPPDATA%\OpenDS\opends.ini`. An `opends.ini` next to the executable takes precedence, which gives you portable mode.

```ini
[ds5]
port=3240
buffer=64
audio=1
haptics_gain=10
```

| Key | Meaning |
| --- | --- |
| `port` | virtual device port |
| `buffer` | audio buffer blocks, 16–128. About 10.7 ms each: 64 blocks ≈ 683 ms, 128 ≈ 1.37 s when full. Larger survives stalls, at the cost of sound lagging the picture |
| `audio` | speaker audio toggle |
| `haptics_gain` | haptic intensity, 10–20 |
| `microphone_gain_db` | microphone gain, 0–50 dB, peak-limited |
| `microphone_buffer_ms` | microphone jitter buffer, 16–512 ms |
| `monitor_sample_hz` | monitor sample rate, 20 / 50 / 100 / 200 |
| `monitor_fps` | monitor refresh rate, 15 / 30 / 60 |
| `monitor_seconds` | monitor time span, 30–900 s |
| `language` | UI language, `auto` follows the system, or a code such as `zh-CN` |
| `theme` | appearance, `system` / `light` / `dark` |
| `close_action` | what closing the window does, `ask` / `tray` / `exit` |

Restart the bridge after changing anything.

## Start at Login

```powershell
.\opends.exe startup enable
.\opends.exe startup status
.\opends.exe startup disable
```

Or pass `-Startup` to `scripts/Install-OpenDS.ps1` during install. Startup uses the current user's Run key and needs no administrator rights.

## Troubleshooting

**The controller is missing from Device Manager**

HidHide is hiding it. Exiting OpenDS restores it; after a crash, run `opends.exe restore`.

**No rumble in game**

Start OpenDS before launching the game. Games usually probe the controller type and its capabilities at startup. A controller connected later is still recognized as an input device, but the capability probe does not run again — rumble, adaptive triggers and lighting may stay inactive.

**Rumble but no sound**

The controller speaker has its own volume, set by the game through the output report. Without a game-side option it keeps its previous level. Make sure `audio=1`.

**Audio stutters**

Raise `buffer`, for example to 96 or 128.

**Steam games misbehave**

Disable Steam Input. It takes over the controller and bypasses the virtual device.

**The magnifier or on-screen keyboard opens by itself**

Steam's guide chords turn PS button combinations into system shortcuts — magnifier, on-screen keyboard, screen recording. Turn it off in Steam: Settings → Controller → Advanced Settings → clear "Enable controller guide chords".

If it only happens occasionally, you can also disable the individual Windows accessibility shortcuts.

**How do I troubleshoot a connection problem?**

Logs are in `%LOCALAPPDATA%\OpenDS\logs\opends.log`, shared by startup and manual launches. They record connections, disconnects, bridge errors and audio statistics. Each file is capped at 1 MiB, with `opends.log.1` and `opends.log.2` kept as history. Every line starts with a Unix timestamp in seconds.

## Compatibility

Supports DualSense and DualSense Edge. Edge uses the Edge PID by default; set `edge_as_ds=true` to present it as a standard DualSense. One controller at a time.

The microphone is experimental and off by default, with adjustable gain and jitter buffering — see [Microphone](docs/guides/microphone.md).

The **Live monitor** page shows controller input alongside speaker, haptic and microphone waveforms — see [Live monitor](docs/guides/monitor.md).

## Build

Requires the Rust MSVC toolchain, the Windows SDK and Git.

```powershell
cargo fetch --locked
cargo test --locked --offline
.\scripts\Build-Release.ps1
```

The optional `bundled-opus` feature compiles a static Opus with CMake; CMake 4 needs `$env:CMAKE_POLICY_VERSION_MINIMUM='3.5'`.

Rebuilding the DLL after changing `vendor/viiper-source` needs Go 1.27.3 and a Windows C compiler:

```powershell
.\scripts\Build-VIIPER.ps1 -Compiler <path-to-gcc.exe>
```

## Release Assets

Each version ships four files:

| File | Contents |
| --- | --- |
| `OpenDS-<version>-windows-x64.zip` | executables, DLLs and scripts |
| `OpenDS-<version>-source.zip` | full source: Rust code, `vendor/viiper-source`, dependency licence notices |
| `SHA256SUMS.txt` | checksums for every asset |
| `TAG-SIGNATURE.json` | receipt for the signed tag |

The "Source code (zip)" and "Source code (tar.gz)" links on the GitHub page are generated by the platform and only contain Git-tracked files; dependency licence notices and the VIIPER source archive are missing from them. **Download `source.zip` when you need the source.**

The release pipeline checks that the VIIPER source in `source.zip` matches the DLL shipped in the binary archive.

## License

GPL-3.0-or-later. Full text in [LICENSE](LICENSE); third-party components in [THIRD-PARTY.md](THIRD-PARTY.md).

When distributing binaries, include `vendor/viiper-source`; the release script packages it automatically.

## Documentation

- [User guide](docs/guides/user-guide.md)
- [Live monitor](docs/guides/monitor.md)
- [General settings](docs/guides/settings.md)
- [Microphone](docs/guides/microphone.md)
- [Developer guide](docs/guides/developer-guide.md)
- [Protocol notes](docs/guides/protocol.md)
- [Resident mode](docs/guides/resident.md)
- [Changelog](CHANGELOG.md)

## Acknowledgements

**Runtime dependencies**

| Project | Role |
| --- | --- |
| [VIIPER](https://github.com/DualSenseClient/VIIPER) | virtual USB DualSense device (GPL-3.0-or-later) |
| [USB/IP Win2](https://github.com/vadimgrn/usbip-win2) | creates the virtual USB device |
| [HidHide](https://github.com/nefarius/HidHide) | hides the physical controller |
| [Opus](https://opus-codec.org/) | audio codec (BSD-3-Clause) |
| [native-windows-gui](https://github.com/gabdube/native-windows-gui) | settings UI (MIT) |

**Design references**

- [input-overlay](https://github.com/univrsal/input-overlay): DualSense layout reference; the presets are CC0
- [3d-controller-overlay](https://github.com/larfingshnew/3d-controller-overlay): visual reference for the three-dimensional handset (MIT)

**Ideas**

- [zhonghuihong](https://space.bilibili.com/37635277) on Bilibili: his documentation clarified how the DualSense behaves on PC and how to approach debugging it. That software is closed source; no code from it is used here.

## Legal Notice

DualSense and PlayStation are registered trademarks of Sony Interactive Entertainment Inc. (SIE).

This is an unofficial third-party project. It is not affiliated with, authorized, sponsored, or endorsed by SIE. Other trademarks are the property of their respective owners.
