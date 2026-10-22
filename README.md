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

## License

GPL-3.0-or-later. Full text in [LICENSE](LICENSE); third-party components in [THIRD-PARTY.md](THIRD-PARTY.md).

When distributing binaries, include `vendor/viiper-source`; the release script packages it automatically.

## Documentation

- [User guide](docs/guides/user-guide.md)
- [Live monitor](docs/guides/monitor.md)
- [Microphone](docs/guides/microphone.md)
- [Developer guide](docs/guides/developer-guide.md)
- [Protocol notes](docs/guides/protocol.md)
- [Resident mode](docs/guides/resident.md)

## Legal Notice

DualSense and PlayStation are registered trademarks of Sony Interactive Entertainment Inc. (SIE).

This is an unofficial third-party project. It is not affiliated with, authorized, sponsored, or endorsed by SIE. Other trademarks are the property of their respective owners.
