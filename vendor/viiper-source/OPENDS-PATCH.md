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

Build the library from this folder with Go 1.27.1 and a Windows C compiler:

```powershell
$env:CGO_ENABLED='1'
go build -buildmode=c-shared -trimpath -ldflags '-s -w' -o ../../target/viiper-build/libVIIPER.dll ./lib/viiper
go test ./device/dualsense
```

Use an explicit proxy for dependency/toolchain downloads as required by the
user. The runtime API addition is resolved by name; an older DLL is rejected
for the standard DualSense profile rather than silently serving the old
descriptors.

The same opt-in profile now paces raw interrupt IN completions using a
capacity-one notification channel. A request waits for a fresh feeder report
or cancellation rather than returning the same atomic snapshot in a busy
loop. GET_REPORT remains an immediate snapshot query. USB descriptors and
input bytes are unchanged. Tests cover pending-read cancellation, fresh
publication wakeup, and a simultaneous GET_REPORT. Standard upstream and
Edge profiles retain their prior scheduling. The host-facing USB server may
still return its cached frame after the endpoint interval expires.
