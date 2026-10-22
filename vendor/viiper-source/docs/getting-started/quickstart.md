# Quick Start

Choose a flavor: **VIIPER TCP Server** (separate process, MIT clients) or
**libVIIPER** (in-process C API, GPL-3.0).

## VIIPER TCP Server

1. **USBIP installed** ([Installation](installation.md#requirements),
   Windows usbip-win2 `0.9.8.1`).
2. Start the server: `viiper server` (`:3241` USBIP, `:3242` API).
3. Run a feeder, e.g. the Go Xbox 360 example:

```bash
go run ./examples/go/virtual_x360_pad localhost:3242
```

It lists/creates a bus (`bus/list`, `bus/create`), adds an `xbox360` device
(`bus/{id}/add`), opens the binary stream (`bus/{busId}/{deviceid}`), pushes
input state, and prints rumble feedback. See
[`examples/go/`](https://github.com/DualSenseClient/VIIPER/tree/main/examples/go)
and [Client Libraries](../server/client-libraries.md) for C#, C++, Rust,
TypeScript, and Go.

## libVIIPER

## 📋 Prerequisites

Ensure you have:

1. **USBIP installed** on your system (see [Installation](installation.md#requirements))
2. **libVIIPER** downloaded from [DualSenseClient/VIIPER Releases](https://github.com/DualSenseClient/VIIPER/releases) or [built from source](installation.md#building-from-source)
3. The `libVIIPER.h` header (shipped with the library) or the P/Invoke declarations shown below

## 🎮 Creating Your First Virtual Device

libVIIPER exposes a pure C API. The general lifecycle is:

1. `NewUSBServer` — start the USBIP server in the background
2. `CreateUSBBus` — create a bus to hold devices
3. `Create<Device>Device` — add a virtual device (auto-attach to the local machine when `autoAttach` is `true`)
4. `Set<Device>DeviceState` — push input state to the device
5. Register output callbacks (rumble, LEDs, ...) to react to host commands
6. `CloseUSBServer` — clean up when done

### C

```c
#include <stdio.h>
#include "libVIIPER.h"

static void logCallback(VIIPERLogLevel level, const char* message) {
    printf("libVIIPER [%d] %s\n", level, message);
}

static void rumbleCallback(Xbox360DeviceHandle handle, uint8_t left, uint8_t right) {
    printf("<- Rumble: Left=%u, Right=%u\n", left, right);
}

int main(void) {
    USBServerConfig conf = { .addr = "localhost:3245" };
    USBServerHandle serverHandle = 0;
    if (!NewUSBServer(&conf, &serverHandle, logCallback)) {
        fprintf(stderr, "Failed to create USB server.\n");
        return 1;
    }

    uint32_t busID = 0;
    CreateUSBBus(serverHandle, &busID);

    Xbox360DeviceHandle deviceHandle = 0;
    if (!CreateXbox360Device(serverHandle, &deviceHandle, busID,
                             /*autoAttach=*/true, 0, 0, 0)) {
        fprintf(stderr, "Failed to create Xbox 360 device.\n");
        return 1;
    }

    SetXbox360RumbleCallback(deviceHandle, rumbleCallback);

    Xbox360DeviceState state = {0};
    for (unsigned frame = 0; frame < 600; frame++) {
        // Only required when an actual change occurs
        state.Buttons = XBOX360_BUTTON_A;
        state.LT      = 128;
        state.LX      = 20000;
        SetXbox360DeviceState(deviceHandle, state);
        Sleep(16);
    }

    CloseUSBServer(serverHandle);
    return 0;
}
```

See [`examples/libVIIPER/C/`](https://github.com/DualSenseClient/VIIPER/tree/main/examples/libVIIPER/C) for the full project including CMake setup.

### C\#

```csharp
using System.Runtime.InteropServices;

class Program
{
    static void LogCallback(VIIPERLogLevel level, string message)
        => Console.WriteLine($"libVIIPER [{level}] {message}");

    static void RumbleCallback(nuint handle, byte leftMotor, byte rightMotor)
        => Console.WriteLine($"<- Rumble: Left={leftMotor}, Right={rightMotor}");

    static int Main()
    {
        // Keep delegates alive for the lifetime of the server/device!
        VIIPERLogCallbackDelegate logCb = LogCallback;

        USBServerConfig conf = new() { addr = "localhost:3245" };
        bool success = LibVIIPER.NewUSBServer(ref conf, out nuint serverHandle, logCb);
        if (!success) return 1;

        uint busID = 0;
        LibVIIPER.CreateUSBBus(serverHandle, ref busID);

        success = LibVIIPER.CreateXbox360Device(serverHandle, out nuint deviceHandle, busID,
                                                autoAttachLocalhost: true, 0, 0, 0);
        if (!success) return 1;

        Xbox360RumbleCallbackDelegate rumbleCb = RumbleCallback;
        LibVIIPER.SetXbox360RumbleCallback(deviceHandle, rumbleCb);

        Xbox360DeviceState state = new();
        for (ulong frame = 0; frame < 600; frame++)
        {
            state.Buttons = Xbox360Buttons.A;
            state.LT      = 128;
            state.LX      = 20000;
            LibVIIPER.SetXbox360DeviceState(deviceHandle, state);
            Thread.Sleep(16);
        }

        LibVIIPER.CloseUSBServer(serverHandle);
        return 0;
    }
}

[StructLayout(LayoutKind.Sequential)]
struct USBServerConfig
{
    [MarshalAs(UnmanagedType.LPStr)]
    public string? addr;
    public ulong connection_timeout_ms;
    public ulong device_handler_connect_timeout_ms;
    public uint  write_batch_flush_interval_ms;
}

[StructLayout(LayoutKind.Sequential)]
struct Xbox360DeviceState
{
    public uint  Buttons;
    public byte  LT;
    public byte  RT;
    public short LX;
    public short LY;
    public short RX;
    public short RY;
    public byte  Reserved0, Reserved1, Reserved2, Reserved3, Reserved4, Reserved5;
}

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
delegate void VIIPERLogCallbackDelegate(VIIPERLogLevel level, [MarshalAs(UnmanagedType.LPStr)] string message);

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
delegate void Xbox360RumbleCallbackDelegate(nuint handle, byte leftMotor, byte rightMotor);

static class LibVIIPER
{
    const string Lib = "libVIIPER";

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool NewUSBServer([In] ref USBServerConfig config, out nuint outHandle, VIIPERLogCallbackDelegate? logCallback);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool CloseUSBServer(nuint handle);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool CreateUSBBus(nuint serverHandle, ref uint busID);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool CreateXbox360Device(nuint serverHandle, out nuint outDeviceHandle, uint busID, [MarshalAs(UnmanagedType.I1)] bool autoAttachLocalhost, ushort idVendor, ushort idProduct, byte xinputSubType);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool SetXbox360DeviceState(nuint deviceHandle, Xbox360DeviceState state);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool SetXbox360RumbleCallback(nuint deviceHandle, Xbox360RumbleCallbackDelegate? callback);
}
```

See [`examples/libVIIPER/csharp/`](https://github.com/DualSenseClient/VIIPER/tree/main/examples/libVIIPER/csharp) for the full project including all P/Invoke declarations.

For a complete C# reference covering every device type, see the
[C# Bindings](../libviiper/csharp.md) page.

## 📦 Distribution

Ship `libVIIPER.dll` (or `libVIIPER.so`) next to your application. The library is portable —
no installation required beyond USBIP (see [Installation](installation.md)).

!!! info "Linux Permissions"
    On Linux, attaching devices via USBIP requires root permissions.
    Run your application with `sudo` or configure appropriate udev rules to allow non-root users to attach devices.

## 🧰 Available Device Types

libVIIPER supports multiple virtual device types including keyboards, mice, and game controllers.
Each device type has its own input state struct and output callbacks.

For a complete list of supported devices, their state structs, and constants, see the
[libVIIPER Device Reference](../libviiper/overview.md#devices).

## ➡️ Next Steps

1. **Explore Examples**: Check the [`examples/libVIIPER/`](https://github.com/DualSenseClient/VIIPER/tree/main/examples/libVIIPER) directory for complete working programs in C and C#
2. **Read the API Overview**: Learn about the full [libVIIPER API](../libviiper/overview.md)
3. **C# deep-dive**: Get all P/Invoke declarations for every device type in the [C# Bindings](../libviiper/csharp.md) reference
4. **TCP clients**: Prefer a managed client over P/Invoke? See [Client Libraries](../server/client-libraries.md) for C#, C++, Rust, TypeScript and Go
5. **Review Device Specs**: Understand device-specific states and callbacks in the [Devices](../devices/xbox360.md) documentation

## 🆘 Troubleshooting

### Server Won't Start

**Port already in use:**

```c
USBServerConfig conf = { .addr = "localhost:3245" };
```

Change the `addr` to use a free port.

**Permission denied (Linux):**

Use a port above 1024 or run your application with `sudo`.

### Auto-Attach Not Working

**Linux - USBIP tool not found:**

```bash
# Ubuntu/Debian
sudo apt install linux-tools-generic

# Arch Linux
sudo pacman -S usbip
```

**Linux - Kernel module not loaded:**

```bash
# Load for current session
sudo modprobe vhci-hcd

# Or configure persistent loading (see Installation guide)
```

**Windows - USBIP tool not found:**

Install the signed [usbip-win2 0.9.8.1 x64 release](https://github.com/vadimgrn/usbip-win2/releases/tag/v.0.9.8.1).
Do not mix it with another usbip-win2 userspace or driver version.

### Device Not Attaching

**USBIP tool not found:**

Make sure USBIP is installed and in your PATH (see [Installation requirements](installation.md#requirements)).

**Connection refused:**

Verify the `NewUSBServer` call succeeded and the USBIP server is listening on the expected address.

### Device Not Working

**No input response:**

Ensure the device was created with `autoAttachLocalhost` set to `true` (or the device is attached via USBIP manually) AND you are sending input state with `Set<Device>DeviceState`.

## 🔗 See Also

- [libVIIPER Overview](../libviiper/overview.md) - Complete C API reference
- [Devices](../devices/xbox360.md) - Device-specific documentation