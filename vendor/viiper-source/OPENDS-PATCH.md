# OpenDS USB profile patch

Upstream: DualSenseClient/VIIPER v1.0.1, commit
`25515e1ef405f189b73d5e2707a4fbb0798e9110` (GPL-3.0-or-later).

This source is included to reproduce the modified OpenDS runtime. Upstream
reference checkouts and the original runtime are preserved separately.

The new C API `UseDualSensePhysicalUSBProfile(handle)` must be called before
USB attachment. It opts a standard DualSense into the wired profile observed
in a real USB startup capture: no USB serial-number index, a 289-byte HID
report descriptor without unsupported F6-F9 entries, and a 4ms full-speed HID
interval. USB/IP Win2 translates that interval to high-speed bInterval 6.
Default upstream profiles and Edge remain unchanged.

The new Go regression test compares the complete HID descriptor against the
captured wired descriptor. OpenDS separately validates and removes the A3
Bluetooth feature CRC before preloading USB feature reports 05, 09 and 20.
No capture containing controller input or audio is included in this source.

Build the library from this folder with Go 1.27.3 and a Windows C compiler:

```powershell
$env:CGO_ENABLED='1'
go build -buildmode=c-shared -trimpath -buildvcs=false -ldflags '-s -w -buildid= -extldflags=-Wl,--no-insert-timestamp' -o ../../target/viiper-build/libVIIPER.dll ./lib/viiper
go test ./device/dualsense
```

Use an explicit proxy for dependency/toolchain downloads as required by the
user. The runtime API addition is resolved by name; an older DLL is rejected
for the standard DualSense profile rather than silently serving the old
descriptors.

The OpenDS source requires Go 1.27.3 so rebuilt runtimes include the October
2026 standard-library security fixes (including GO-2026-6809). The module-level
GO-2026-5932 notice concerns the deprecated x/crypto/openpgp packages; these
packages are not imported by lib/viiper. No USB descriptor or controller report
format changes are made by the toolchain update.

The same opt-in profile now paces raw interrupt IN completions using a
capacity-one notification channel. A request waits for a fresh feeder report
or cancellation rather than returning the same atomic snapshot in a busy
loop. GET_REPORT remains an immediate snapshot query. USB descriptors and
input bytes are unchanged. Tests cover pending-read cancellation, fresh
publication wakeup, and a simultaneous GET_REPORT. Standard upstream and
Edge profiles retain their prior scheduling. The host-facing USB server may
still return its cached frame after the endpoint interval expires.

## Auditable patch

`vendor/opends-viiper.patch` applies to the upstream commit above. From an
unmodified v1.0.1 checkout, run `git apply /path/to/vendor/opends-viiper.patch`.
The patch excludes differences consisting only of CRLF/LF line endings.
The patched tree is verified against all bundled source files after that
normalization; no controller capture is needed or included.

To regenerate and verify it from the OpenDS repository root, using a local
official v1.0.1 checkout (download through an explicit proxy if needed):

```powershell
python scripts/Make-VIIPERPatch.py --upstream /path/to/VIIPER-v1.0.1
```

## Bundled DLL build record

From the OpenDS repository root, use `scripts/Build-VIIPER.ps1 -Compiler
<gcc.exe> -Offline -VerifyReproducible` with cached dependencies, or replace
`-Offline` with `-Proxy <url>`. This generates `vendor/viiper/build-info.json`
with the exact build arguments, toolchain versions and source fingerprint.
It also updates the record below. Regenerate the patch after each rebuild.
`scripts/Test-VIIPERBuildInfo.ps1` checks both the source and bundled DLL;
release packaging runs this check automatically.

The forced rebuild check recompiles with `-a` to a separate output path.
Matching hashes demonstrate repeatability with the recorded local toolchain;
they do not promise byte identity across different Go or C compiler versions.
The hash is an integrity check, not an independent attestation of provenance.
The build record excludes this notes file from the source fingerprint to
avoid a circular dependency on the DLL hash recorded here.

<!-- BUILD-INFO:BEGIN -->
DLL SHA-256: `28481ea329a98e80ff1e159c91ed5d30847e48213c7bb567cba172239cd00094`

Source SHA-256 (rules in build-info.json): `5cb73d8cba5d763769ea6584ea3a3265553cac75fff8fd1315cfde4160c8a57c`

Toolchain: go version go1.27.3 windows/amd64; gcc.exe (GCC) 16.2.0.

Independent forced rebuild matched: True.
<!-- BUILD-INFO:END -->
