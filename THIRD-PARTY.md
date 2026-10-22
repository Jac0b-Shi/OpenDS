# Third-party components

OpenDS is distributed under GPL-3.0-or-later (see LICENSE).

| Component | Version | License / source |
| --- | --- | --- |
| VIIPER / libVIIPER.dll | v1.0.1 + OpenDS USB profile patch | GPL-3.0-or-later, https://github.com/DualSenseClient/VIIPER/tree/v1.0.1 |
| libopus-0.dll (MSYS2 MinGW x64 package) | 1.6.1-2 | BSD-3-Clause; https://opus-codec.org/ ; packaging https://github.com/msys2/MINGW-packages/tree/master/mingw-w64-opus |
| native-windows-gui | 1.0.13 | MIT, https://github.com/gabdube/native-windows-gui |
| winapi | 0.3.9 | MIT OR Apache-2.0, https://github.com/retep998/winapi-rs |
| bitflags, lazy_static, newline-converter, unicode-segmentation | Cargo.lock | MIT / Apache-2.0; exact resolved versions and sources are in Cargo.lock and licenses/rust-dependencies.json |
| opusic-sys (optional static Opus) | 0.3.6 | BSL-1.0 binding; bundled Opus BSD; https://github.com/DoumanAsh/opusic-sys |

VIIPER's Go dependency notices are included in licenses/VIIPER-dependencies.txt.
Rust crate license files are collected by the release script. The matching source
archive contains OpenDS sources, Cargo.lock, and the VIIPER v1.0.1 source archive.
The modified runtime's complete source is included in `vendor/viiper-source`,
with changes documented in `OPENDS-PATCH.md` and an applicable upstream diff in
`vendor/opends-viiper.patch`. `vendor/viiper/build-info.json` records the DLL and
source hashes, toolchains and build arguments. `scripts/Build-VIIPER.ps1`
rebuilds the DLL with an explicit proxy or cached offline dependencies; its
`-VerifyReproducible` option checks a separate forced rebuild. Packaging verifies
the recorded source and DLL hashes before creating the release archives.
Rust dependency source packages are identified by Cargo.lock checksums and official
registry URLs. `cargo fetch --locked` retrieves them for rebuilding.

USB/IP Win2 and HidHide are installed separately from official signed installers:
https://github.com/vadimgrn/usbip-win2/releases/tag/v.0.9.8.1
https://github.com/nefarius/HidHide/releases/tag/v1.5.230.0
Pinned installer URLs, publishers and hashes are in scripts/drivers.json.

The Bluetooth microphone packet layout was checked against the implementation
notes in https://github.com/MarcelineVPQ/DS5Dongle-OLED-Edition/blob/master/BLUETOOTH_AUDIO_NOTES.md .
