<img src="docs/viiper.svg" align="right" width="128" alt="VIIPER logo" />
<br />

[![Build Status](https://github.com/DualSenseClient/VIIPER/actions/workflows/snapshots.yml/badge.svg)](https://github.com/DualSenseClient/VIIPER/actions/workflows/snapshots.yml)
[![License: GPL-3.0](https://img.shields.io/github/license/DualSenseClient/VIIPER)](https://github.com/DualSenseClient/VIIPER/blob/main/LICENSE.txt)
[![Release](https://img.shields.io/github/v/release/DualSenseClient/VIIPER?include_prereleases&sort=semver)](https://github.com/DualSenseClient/VIIPER/releases)
[![Issues](https://img.shields.io/github/issues/DualSenseClient/VIIPER)](https://github.com/DualSenseClient/VIIPER/issues)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](https://github.com/DualSenseClient/VIIPER/pulls)

# VIIPER 🐍

**Virtual** **I**nput over **IP** **E**mulato**R**

A **cross-platform virtual USB input framework** for creating virtual USB input devices (game controllers, keyboards, mice and more)
that are indistinguishable from real hardware to the operating system and applications.

VIIPER lets developers create and programmatically control virtual USB input devices (using USBIP under the hood),
enabling seamless integration for gaming, automation, testing and remote control scenarios.

This repository is a fork of [Alia5/VIIPER](https://github.com/Alia5/VIIPER) and serves as the embedded backend
for the **DualSense Client** project, providing virtual controller output. DualSense Client embeds VIIPER directly as a library
instead of running it as a separate app.

## 🍴 What this fork adds

This fork brings full DualShock 4 and DualSense (including DualSense Edge) support:

- **Complete report handling** — separate DualSense / DualSense Edge HID descriptors, the
  full 64B input report (touch contact and tracking IDs, temperature byte, host mute-light
  echo, Edge firmware echo) and the full 48B output report decode including trigger effects,
  plus the whole feature-report table served over `GET_REPORT`; on DualShock 4, a DS4-reference
  HID descriptor, feature-report parity with payload-only `GET_REPORT`, correct touch packet
  counters and the full 32B output report tolerance.
- **USB audio** — UAC1 speaker and microphone with isochronous endpoints for both controllers,
  feeder-driven speaker PCM with a per-generation reset barrier, microphone PCM queues, and a
  realtime haptics lane from the DualSense rear voice-coil channels.
- **USBIP isochronous transport** — completions paced to the USB frame clock and corrected
  audio descriptors, so audio survives the trip to the host.
- **Raw input passthrough** — a feeder can forward a real controller's exact input report
  (`SetDualSenseRawInputReport` / `SetDS4RawInputReport`, or the `bus/{id}/{deviceid}/raw`
  TCP stream) instead of the synthesized one; synthetic reports stay the default.
- **Runtime metadata merge-update** — serial, battery and color changes applied to a live
  device without recreating it.
- **Conformance tests** — descriptor/report sweeps for both devices.

## ✨ Features

- Runs on Linux and Windows.
- Pure C API callable from any language with C FFI support.
- VIIPER abstracts away all USB / USBIP details.
- VIIPER is portable and runs entirely in userspace.
    - Utilizes a generic USBIP kernel mode driver
      (built into Linux; on Windows [usbip-win2](https://github.com/vadimgrn/usbip-win2) provides a signed kernel mode driver)
      New device types never require touching kernel code.
- After installing USBIP once, VIIPER can run without additional dependencies or system-wide installation.

## 🍦 Two flavors

- **VIIPER TCP Server** — portable `viiper server` (`:3241` USBIP, `:3242`
  management API). Drive it over TCP with MIT-licensed clients in
  [`clients/`](clients) or the Go examples in [`examples/go`](examples/go).
  See [Client Libraries](docs/server/client-libraries.md).
- **libVIIPER** — a single shared library (`libVIIPER.dll` / `libVIIPER.so`)
  embedding the stack in-process via a pure C API (GPL-3.0). See
  [libVIIPER documentation](docs/libviiper/overview.md).

## 🎮 Emulatable devices

- Xbox 360 controller emulation; see [Devices › Xbox 360 Controller](docs/devices/xbox360.md)
- HID Keyboard with N-key rollover and LED feedback; see [Devices › Keyboard](docs/devices/keyboard.md)
- HID Mouse with 5 buttons and horizontal/vertical wheel; see [Devices › Mouse](docs/devices/mouse.md)
- PS4 controller emulation; see [Devices › DualShock 4 Controller](docs/devices/dualshock4.md)
- PS5 DualSense controller emulation (including Edge variant); see [Devices › DualSense Controller](docs/devices/dualsense.md)
- Nintendo Switch 2 Pro Controller emulation; see [Devices › Switch 2 Pro Controller](docs/devices/ns2pro.md)

## 🏗️ Architecture

```text
Physical controller
        |
        | HID input and feedback
        v
Feeder application
        |
        | viiper TCP API (:3242) or libVIIPER C API (in-process)
        v
VIIPER userspace USB device
        |
        | USBIP (:3241)
        v
usbip-win2 virtual host controller
        |
        v
Windows, games, and services
```

VIIPER does not emulate a Bluetooth radio and does not make the virtual device appear wirelessly paired. The game sees
a native-style USB controller. A separate app, such as DualSense Client or DS4Windows, is then responsible for
translating and forwarding supported feedback between that virtual USB device and the physical USB or Bluetooth
controller.

## 💻 Installation

- **Server**: download `viiper-windows-<arch>.zip` / `viiper-linux-<arch>.tar.gz`
  from the [latest DualSenseClient release](https://github.com/DualSenseClient/VIIPER/releases/latest),
  or run `irm https://dualsenseclient.github.io/VIIPER/stable/install.ps1 | iex`
  (Windows, `%LOCALAPPDATA%\VIIPER`) / install.sh (Linux, `/usr/local/bin`).
- **Library**: download the `libVIIPER` artifact (`libVIIPER.dll`/`libVIIPER.so`,
  `libVIIPER.h`, import definition) from the same release page.

Both need [`usbip-win2 0.9.8.1`](https://github.com/vadimgrn/usbip-win2/releases/tag/v.0.9.8.1)
on Windows (signed kernel driver).

## 🔌 Requirements

**Linux:**

- **Arch Linux:**
    - Install: `sudo pacman -S usbip`
    - Docs: [Arch Wiki: USBIP](https://wiki.archlinux.org/title/USB/IP)
- **Ubuntu / Debian:**
    - Install: `sudo apt install linux-tools-generic`
    - Docs: [Ubuntu USBIP Manual](https://manpages.ubuntu.com/manpages/noble/man8/usbip.8.html)

**Windows:**

- Windows 10 or Windows 11 x64
- [usbip-win2](https://github.com/vadimgrn/usbip-win2) — by far the most complete implementation of USBIP for Windows
  (comes with a **SIGNED** kernel mode driver)

## 🥫 Feeder application development

Two paths:

- **TCP Server** (`examples/go`, `clients/`): run `viiper server`, manage
  buses/devices over TCP (`bus/list`, `bus/create`, `bus/{id}/add`,
  `bus/{busId}/{deviceid}` streams). MIT-licensed clients.
- **Embedded library** (`examples/libVIIPER`, C/C#): link libVIIPER (GPL-3.0)
  and drive devices in-process.

### 🔌 API

- **Server TCP API**: `ping`, `bus/list`, `bus/create`, `bus/remove`,
  `bus/{id}/list`, `bus/{id}/add`, `bus/{id}/remove`, binary device streams.
  See [Client Libraries](docs/server/client-libraries.md).
- **libVIIPER C API** (`libVIIPER.h`):

- **Server lifecycle** — `NewUSBServer`, `CloseUSBServer`
- **Bus management** — `CreateUSBBus`, `RemoveUSBBus`
- **Device creation** — one `Create<Device>Device` per emulatable device type, with a `meta` parameter to control
  identity (serial number, battery, colors, …)
- **Input feeding** — `Set<Device>DeviceState` to push input (buttons, sticks, touch, IMU, …)
- **Host feedback** — output callbacks for rumble, LEDs (DualSense full output
  state incl. trigger effects and player LEDs;
  DualShock 4 `updateFlags` + flash), Xbox rumble, keyboard LEDs, Switch 2 HD
  rumble/LEDs. DualSense audio is feeder-driven: speaker PCM and the rear
  voice-coil haptics pair are delivered to callbacks (with a per-generation
  speaker-reset barrier), and `SetDualSenseMicrophonePCM` queues 192B mic
  frames (silence on underrun).

All functions return `bool` and are callable from any language with C FFI support. VIIPER takes care of all USBIP
protocol details, so you can focus on implementing the device logic only. On `localhost`, libVIIPER also automatically
attaches the USBIP client, so you don't have to worry about USBIP details at all.

See the [libVIIPER documentation](docs/libviiper/overview.md) for the complete API reference, the
[C# Bindings](docs/libviiper/csharp.md) for the full C# P/Invoke reference, and
[Client Libraries](docs/server/client-libraries.md) if you prefer driving a standalone
`viiper server` over TCP.

## 🛠️ VIIPER development

### 🧰 Prerequisites

- [Go](https://go.dev/) 1.26 or newer
- USBIP installed
- (Optional) [just](https://github.com/casey/just)
    - Windows: `winget install --id Casey.Just --exact`
    - Linux: use your package manager (`sudo pacman -S just`, `sudo apt install just`, ...)
- Windows compiler (required for `build-libVIIPER`):
    - `winget install -e --id MartinStorsjo.LLVM-MinGW.UCRT --accept-package-agreements --accept-source-agreements`

### 🔄 Building from source

```bash
git clone https://github.com/DualSenseClient/VIIPER.git
cd VIIPER
just build-libVIIPER
```

The output is written to `dist/libVIIPER/` (`libVIIPER.dll`/`libVIIPER.so` plus the generated `libVIIPER.h` header).
Building libVIIPER requires CGO (`CGO_ENABLED=1`) and a C compiler (GCC / MSVC / Clang) in `PATH`.

For more build options:

```bash
just --list            # Show all available targets
just test              # Run tests
go test ./...          # Run tests directly
```

## 🤝 Contributing

Contributions are welcome!
Please open issues or pull requests on GitHub.
See the [issues page](https://github.com/DualSenseClient/VIIPER/issues) for bugs and feature requests.

## ❓ FAQ

### What is USBIP and why does VIIPER use it?

USBIP is a protocol that allows USB devices to be shared over a network.
VIIPER uses it because it's already built into Linux and available for Windows, making virtual device emulation
possible without writing custom kernel drivers yourself.

### Why should my application be GPL-3.0 compatible?

libVIIPER is licensed under **GPL-3.0**. Linking against it (as a shared library) requires your application to be
GPL-3.0 compatible.

### Can I use VIIPER for gaming?

Yes! VIIPER can create virtual input devices that appear as real hardware to games and applications.
This works with Steam, native Windows games and any other application that supports the emulated device types.

### How is VIIPER different from other controller emulators?

Many controller emulation approaches require writing a custom kernel driver for every device type you want to support.
VIIPER uses USBIP to handle the USB protocol layer, so device emulation code lives entirely in userspace.

USBIP itself does require a kernel driver. On Linux, the USBIP driver is built into the kernel. On Windows,
[usbip-win2](https://github.com/vadimgrn/usbip-win2) provides a signed kernel mode driver. That driver is generic and
does not need to know anything about specific device types — all device-type logic stays in userspace.

This makes VIIPER portable, easier to extend and simpler to bundle with applications. Adding a new device type never
requires touching kernel code.

### Can I add support for other device types?

Yes! VIIPER's architecture is designed to be extensible.
Check the [xbox360 device implementation](./device/xbox360/) as a reference for creating new device types.

### What about input latency?

End-to-end input latency for virtual devices created with VIIPER is typically well below 1 millisecond on a modern
desktop. To not stress the CPU excessively, reports get batched and sent every millisecond, so the best you will achieve
is a 1000 Hz update rate — more than enough, and more than what most real hardware devices provide.
_Note_: Actual device polling rates may be lower depending on the device type and configuration.

## 🔧 Troubleshooting

Report backend issues at [DualSenseClient/VIIPER Issues](https://github.com/DualSenseClient/VIIPER/issues). Report
controller mapping or DS4Windows UI issues at [hbashton/DS4Windows Issues](https://github.com/hbashton/DS4Windows/issues).

## 📄 License

```license
VIIPER - Virtual Input over IP EmulatoR

Copyright (C) 2025-2026 Peter Repukat

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <https://www.gnu.org/licenses/>.
```

libVIIPER and the VIIPER core are licensed under GPL-3.0-or-later; see [`LICENSE.txt`](LICENSE.txt) for the full text.

## Credits

VIIPER was originally created by Peter Repukat and the [Alia5/VIIPER](https://github.com/Alia5/VIIPER) contributors,
and this fork builds on this project alongside another fork created by Hunter Ashton and the
[hbashton/VIIPER](https://github.com/hbashton/VIIPER) contributors.

- [ViGEmBus](https://github.com/nefarius/ViGEmBus)
  (retired, but still widely used) — Windows kernel-mode driver emulating well-known USB game controllers.
  Shoutout and thank you to @nefarius for paving the way.
- [Valve Software](https://www.valvesoftware.com/)
  for creating the OG Steam Controller (2015) and Steam Input, which sent this project down the rabbit hole in the first place.
- **USBIP** — without it, VIIPER would not be possible.
    - [USBIP](https://usbip.sourceforge.net/)
    - [usbip-win2](https://github.com/vadimgrn/usbip-win2)
- [SDL](https://www.libsdl.org/)
  for their excellent work on input device handling, reducing reversing efforts to a minimum.
- [DS5Dongle](https://github.com/awalol/DS5Dongle) — protocol research behind this fork's
  DualSense and DualSense Edge support.
- [DS4Dongle](https://github.com/snipem/DS4Dongle) — protocol research behind this fork's
  DualShock 4 support.
- [Linux's kernel driver for Sony PlayStation controllers](https://github.com/torvalds/linux/blob/master/drivers/hid/hid-playstation.c) — Used as a reference for
  protocol behavior.

It also depends on controller/audio protocol research shared by SAxense, DualSense reverse-engineering projects, and
the wider open-source community.
