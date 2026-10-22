<img src="viiper.svg" align="right" width="200"/>
<br />

# VIIPER 🐍

**Virtual** **I**nput over **IP** **E**mulato**R**

A **cross-platform virtual USB input framework** for creating virtual USB input devices (game controllers, keyboards, mice and more)
that are indistinguishable from real hardware to the operating system and applications.

## Quick Links

- [Installation](getting-started/installation.md)
- [Quick Start](getting-started/quickstart.md)
- [USBIP Setup](getting-started/usbip.md)
- [VIIPER Server Client Libraries](server/client-libraries.md)
- [libVIIPER API Overview](libviiper/overview.md)
- [C# Bindings](libviiper/csharp.md)
- [GitHub Repository](https://github.com/DualSenseClient/VIIPER)

## Two flavors

VIIPER ships the same USB/USBIP stack in two forms:

- **VIIPER TCP Server** — portable `viiper server` (`:3241` USBIP, `:3242`
  management API). Drive it over TCP with the generated clients in
  [`clients/`](https://github.com/DualSenseClient/VIIPER/tree/main/clients)
  or the Go examples in
  [`examples/go/`](https://github.com/DualSenseClient/VIIPER/tree/main/examples/go).
  MIT-licensed clients; no GPL obligations. See
  [Client Libraries](server/client-libraries.md).
- **libVIIPER** — `libVIIPER.dll` / `libVIIPER.so` embedding the stack
  in-process via a pure C API. GPL-3.0. See
  [libVIIPER Overview](libviiper/overview.md).

## What is libVIIPER?

libVIIPER is a single shared library (`libVIIPER.dll` on Windows, `libVIIPER.so` on Linux) that embeds the full VIIPER USB/USBIP stack directly into your application.

- Pure C API, callable from any language with C FFI support (C, C#, C++, Rust, ...)
- In-process and threadsafe: the USBIP server runs in a background thread inside your application
- Optional auto-attach to the local USBIP client on the same machine
- No separate server process, no TCP protocol to implement

These virtual devices are indistinguishable from real hardware to the operating system and applications.

- Runs on Linux and Windows.
- VIIPER abstracts away all USB / USBIP details.
- VIIPER is portable and runs entirely in userspace.
    - Utilizes a generic USBIP kernel mode driver
      (built into Linux; on Windows [usbip-win2](https://github.com/vadimgrn/usbip-win2) provides a signed kernel mode driver)
      New device types never require touching kernel code.
- After installing USBIP once, libVIIPER can run without additional dependencies or system-wide installation.

## Emulatable devices

- Xbox 360 controller emulation; see [Devices › Xbox 360 Controller](devices/xbox360.md)
- HID Keyboard with N-key rollover and LED feedback; see [Devices › Keyboard](devices/keyboard.md)
- HID Mouse with 5 buttons and horizontal/vertical wheel; see [Devices › Mouse](devices/mouse.md)
- PS4 controller emulation; see [Devices › DualShock 4 Controller](devices/dualshock4.md)
- PS5 DualSense controller emulation (including Edge variant); see [Devices › DualSense Controller](devices/dualsense.md)
- Nintendo Switch 2 Pro Controller emulation; see [Devices › Switch 2 Pro Controller](devices/ns2pro.md)

---

## 🥫 Feeder application development

Two paths, same virtual devices:

- **TCP Server**: run `viiper server`, then use a feeder in any language
  (`examples/go/virtual_x360_pad`, `clients/*`). Best when the feeder and
  the USB stack live in different processes, or to stay MIT-licensed.
- **Embedded library**: link libVIIPER and drive devices in-process:

```c
USBServerConfig conf = { .addr = "localhost:3245" };
USBServerHandle serverHandle = 0;
NewUSBServer(&conf, &serverHandle, logCallback);

uint32_t busID = 0;
CreateUSBBus(serverHandle, &busID);

Xbox360DeviceHandle deviceHandle = 0;
CreateXbox360Device(serverHandle, &deviceHandle, busID, /*autoAttach=*/true, 0, 0, 0);

SetXbox360RumbleCallback(deviceHandle, rumbleCallback);

Xbox360DeviceState state = {0};
state.Buttons = XBOX360_BUTTON_A;
state.LT      = 128;
SetXbox360DeviceState(deviceHandle, state);

CloseUSBServer(serverHandle);
```

See the [libVIIPER documentation](libviiper/overview.md) for usage guides in C and C#,
and [Client Libraries](server/client-libraries.md) for the TCP server protocol.

VIIPER takes care of all USBIP protocol details, so you can focus on implementing the device logic only.
On `localhost`, libVIIPER also automatically attaches the USBIP client, so you don't have to worry about USBIP details at all.

---

## ❓ FAQ

### What is USBIP and why does VIIPER use it?

USBIP is a protocol that allows USB devices to be shared over a network.
VIIPER uses it because it's already built into Linux and available for Windows, making virtual device emulation possible without writing custom kernel drivers yourself.

### Can I use VIIPER for gaming?

Yes! VIIPER can create virtual input devices that appear as real hardware to games and applications.

This works with Steam, native Windows games and any other application that supports the emulated device types.

### How is VIIPER different from other controller emulators?

Many controller emulation approaches require writing a custom kernel driver for every device type you want to support.
VIIPER uses USBIP to handle the USB protocol layer, so device emulation code lives entirely in userspace.

USBIP itself does require a kernel driver.
On Linux, the USBIP driver is built into the kernel.
On Windows, [usbip-win2](https://github.com/vadimgrn/usbip-win2) provides a signed kernel mode driver.
That driver is generic and does not need to know anything about specific device types.
All device-type logic stays in userspace.

This makes VIIPER portable, easier to extend and simpler to bundle with applications.
Adding a new device type never requires touching kernel code.

### Why do I need to accept the GPL-3.0 license when using libVIIPER?

libVIIPER is licensed under **GPL-3.0**.
Linking against it requires your application to be GPL-3.0 compatible.

### What about input latency?

End-to-end input latency for virtual devices created with libVIIPER is typically well below 1 millisecond on a modern desktop.
To not stress the CPU excessively, reports get batched and sent every millisecond. So the best you will achieve is a 1000 Hz update rate, which is more than enough and more than what most real hardware devices provide.
_Note_: Actual device polling rates may be lower depending on the device type and configuration.