# C# Bindings

libVIIPER is a pure C shared library (`libVIIPER.dll` on Windows, `libVIIPER.so` on
Linux). From C# you call it through **P/Invoke** — there is no managed wrapper
package for the embedded library, but the complete set of declarations is small,
stable, and documented below.

!!! tip "Two ways to use VIIPER from C#"
    - **libVIIPER (this page)** — embed the whole USB/USBIP stack in your process via P/Invoke.
    - **Viiper.Client** — a generated TCP client library that talks to a standalone
      `viiper server` over the network. See [Client Libraries](../server/client-libraries.md).

## Project setup

Create a regular .NET console/library project and copy the native library next to
your output:

```xml
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <TargetFramework>net10.0</TargetFramework>
    <ImplicitUsings>enable</ImplicitUsings>
    <Nullable>enable</Nullable>
  </PropertyGroup>
  <ItemGroup Condition="$([MSBuild]::IsOSPlatform('Windows'))">
    <None Include="..\..\dist\libVIIPER\libVIIPER.dll">
      <CopyToOutputDirectory>PreserveNewest</CopyToOutputDirectory>
    </None>
  </ItemGroup>
  <ItemGroup Condition="$([MSBuild]::IsOSPlatform('Linux'))">
    <None Include="..\..\dist\libVIIPER\libVIIPER.so">
      <CopyToOutputDirectory>PreserveNewest</CopyToOutputDirectory>
    </None>
  </ItemGroup>
</Project>
```

A complete runnable project lives at
[`examples/libVIIPER/csharp/`](https://github.com/DualSenseClient/VIIPER/tree/main/examples/libVIIPER/csharp).

## General P/Invoke rules

Apply these to every declaration in this reference:

- `[DllImport("libVIIPER", CallingConvention = CallingConvention.Cdecl)]` on every function.
- `bool` return values and `bool` parameters need `[MarshalAs(UnmanagedType.I1)]`.
- Handles (`USBServerHandle`, `Xbox360DeviceHandle`, …) are opaque `uintptr_t` values → use `nuint`.
- `size_t` parameters → use `nuint`.
- Strings inside config/meta structs are `char*` → `[MarshalAs(UnmanagedType.LPStr)] string?`.
- Callback delegates need `[UnmanagedFunctionPointer(CallingConvention.Cdecl)]`.
- **Keep delegate instances alive** in fields or local variables for the lifetime of
  the server/device they are registered on — the GC cannot see references held by
  native code. Registering a method group inline (`SetXbox360RumbleCallback(handle, RumbleCallback)`)
  creates a temporary delegate that can be collected at any time.
- Structs passed by value must mirror the C layout exactly with
  `[StructLayout(LayoutKind.Sequential)]`.
- Callbacks are invoked from the USBIP server's background threads. Keep them short
  and thread-safe, and **never call into libVIIPER from inside a callback**.
  PCM buffers passed to audio callbacks are only valid during the call — copy the
  data if you need to keep it.

## Constants

### Log levels

```csharp
enum VIIPERLogLevel
{
    Debug = -4,
    Info  = 0,
    Warn  = 4,
    Error = 8,
}
```

### Xbox 360 buttons

```csharp
static class Xbox360Buttons
{
    public const uint DPadUp    = 0x0001;
    public const uint DPadDown  = 0x0002;
    public const uint DPadLeft  = 0x0004;
    public const uint DPadRight = 0x0008;
    public const uint Start     = 0x0010;
    public const uint Back      = 0x0020;
    public const uint LThumb    = 0x0040;
    public const uint RThumb    = 0x0080;
    public const uint LShoulder = 0x0100;
    public const uint RShoulder = 0x0200;
    public const uint Guide     = 0x0400;
    public const uint A         = 0x1000;
    public const uint B         = 0x2000;
    public const uint X         = 0x4000;
    public const uint Y         = 0x8000;
}
```

### DualShock 4 buttons & D-pad

```csharp
static class DS4Buttons
{
    public const ushort Square   = 0x0010;
    public const ushort Cross    = 0x0020;
    public const ushort Circle   = 0x0040;
    public const ushort Triangle = 0x0080;
    public const ushort L1       = 0x0100;
    public const ushort R1       = 0x0200;
    public const ushort L2       = 0x0400;
    public const ushort R2       = 0x0800;
    public const ushort Share    = 0x1000;
    public const ushort Options  = 0x2000;
    public const ushort L3       = 0x4000;
    public const ushort R3       = 0x8000;
    public const ushort PS       = 0x0001;
    public const ushort Touchpad = 0x0002;
}

static class DS4DPad
{
    public const byte Up       = 0x01;
    public const byte Down     = 0x02;
    public const byte Left     = 0x04;
    public const byte Right    = 0x08;
}
```

!!! note "DS4 D-pad encoding"
    `DS4DeviceState.DPad` is a **bitmask** (the values above), which is what the
    `DS4DPad` class here and the [`docs/devices/dualshock4.md`](../devices/dualshock4.md)
    reference use. Be careful not to confuse it with the `DS4_DPAD_*` constants in
    `libVIIPER.h` (e.g. `DS4_DPAD_UP = 0x00`, `DS4_DPAD_NEUTRAL = 0x08`) — those are
    the USB hat-switch encoding used inside the report and must **not** be OR'd into
    the field.

### DualSense buttons & D-pad

```csharp
static class DSButtons
{
    public const uint Square   = 0x00000010;
    public const uint Cross    = 0x00000020;
    public const uint Circle   = 0x00000040;
    public const uint Triangle = 0x00000080;
    public const uint L1       = 0x00000100;
    public const uint R1       = 0x00000200;
    public const uint L2       = 0x00000400;
    public const uint R2       = 0x00000800;
    public const uint Create   = 0x00001000;
    public const uint Options  = 0x00002000;
    public const uint L3       = 0x00004000;
    public const uint R3       = 0x00008000;
    public const uint PS       = 0x00010000;
    public const uint Touchpad = 0x00020000;
    public const uint MicMute  = 0x00040000;
    public const uint LFn      = 0x00100000; // Edge
    public const uint RFn      = 0x00200000; // Edge
    public const uint L4       = 0x00400000; // Edge
    public const uint R4       = 0x00800000; // Edge
}

static class DSDPad
{
    public const byte Up    = 0x01;
    public const byte Down  = 0x02;
    public const byte Left  = 0x04;
    public const byte Right = 0x08;
}

static class DSConnection
{
    public const byte Headphone = 0x01;
    public const byte Mic       = 0x02;
    public const byte MicMuted  = 0x04;
    public const byte USBData   = 0x08;
    public const byte USBPower  = 0x10;
}

static class DSShellColors
{
    public const string White = "00";
    public const string Black = "01";
    public const string CosmicRed = "02";
    public const string NovaPink = "03";
    public const string GalacticPurple = "04";
    public const string StarlightBlue = "05";
    public const string GreyCamouflage = "06";
    public const string VolcanicRed = "07";
    public const string SterlingSilver = "08";
    public const string CobaltBlue = "09";
    public const string ChromaTeal = "10";
    public const string ChromaIndigo = "11";
    public const string ChromaPearl = "12";
    public const string Anniversary30th = "30";
    public const string GodOfWarRagnarok = "Z1";
    public const string SpiderMan2 = "Z2";
    public const string AstroBot = "Z3";
    public const string Fortnite = "Z4";
    public const string MonsterHunterWilds = "Z5";
    public const string TheLastOfUs = "Z6";
    public const string GhostOfYotei = "Z7";
    public const string IconBlueLimitedEdition = "ZB";
    public const string AstroBotJoyfulEdition = "ZC";
    public const string GenshinImpact = "ZE";
}
```

### Switch 2 Pro buttons

```csharp
static class NS2ProButtons
{
    public const uint B          = 0x00000001;
    public const uint A          = 0x00000002;
    public const uint Y          = 0x00000004;
    public const uint X          = 0x00000008;
    public const uint R          = 0x00000010;
    public const uint ZR         = 0x00000020;
    public const uint Plus       = 0x00000040;
    public const uint RightStick = 0x00000080;
    public const uint Down       = 0x00000100;
    public const uint Right      = 0x00000200;
    public const uint Left       = 0x00000400;
    public const uint Up         = 0x00000800;
    public const uint L          = 0x00001000;
    public const uint ZL         = 0x00002000;
    public const uint Minus      = 0x00004000;
    public const uint LeftStick  = 0x00008000;
    public const uint Home       = 0x00010000;
    public const uint Capture    = 0x00020000;
    public const uint GR         = 0x00040000;
    public const uint GL         = 0x00080000;
    public const uint C          = 0x00100000;
    public const uint Headset    = 0x00200000;
}

static class NS2ProSticks
{
    public const ushort Min    = 0x0000;
    public const ushort Center = 0x0800;
    public const ushort Max    = 0x0FFF;
}
```

### Keyboard modifiers & LEDs

```csharp
static class KBModifiers
{
    public const byte LeftCtrl  = 0x01;
    public const byte LeftShift = 0x02;
    public const byte LeftAlt   = 0x04;
    public const byte LeftGUI   = 0x08;
    public const byte RightCtrl = 0x10;
    public const byte RightShift = 0x20;
    public const byte RightAlt  = 0x40;
    public const byte RightGUI  = 0x80;
}

static class KBLEDs
{
    public const byte NumLock    = 0x01;
    public const byte CapsLock   = 0x02;
    public const byte ScrollLock = 0x04;
    public const byte Compose    = 0x08;
    public const byte Kana       = 0x10;
}
```

### Mouse buttons

```csharp
static class MouseButtons
{
    public const byte Left    = 0x01;
    public const byte Right   = 0x02;
    public const byte Middle  = 0x04;
    public const byte Back    = 0x08;
    public const byte Forward = 0x10;
}
```

### Keyboard key codes

`KB_KEY_*` constants are USB HID usage codes (page 0x07, Keyboard/Keypad). Set
bits in `KeyboardDeviceState.KeyBitmap` at the index of the key code divided by 8
(`KeyBitmap[code / 8] |= (byte)(1 << (code % 8))`).

```csharp
static class KBKeys
{
    public const byte A = 0x04;   public const byte B = 0x05;
    public const byte C = 0x06;   public const byte D = 0x07;
    public const byte E = 0x08;   public const byte F = 0x09;
    public const byte G = 0x0A;   public const byte H = 0x0B;
    public const byte I = 0x0C;   public const byte J = 0x0D;
    public const byte K = 0x0E;   public const byte L = 0x0F;
    public const byte M = 0x10;   public const byte N = 0x11;
    public const byte O = 0x12;   public const byte P = 0x13;
    public const byte Q = 0x14;   public const byte R = 0x15;
    public const byte S = 0x16;   public const byte T = 0x17;
    public const byte U = 0x18;   public const byte V = 0x19;
    public const byte W = 0x1A;   public const byte X = 0x1B;
    public const byte Y = 0x1C;   public const byte Z = 0x1D;
    public const byte Digit1 = 0x1E; public const byte Digit2 = 0x1F;
    public const byte Digit3 = 0x20; public const byte Digit4 = 0x21;
    public const byte Digit5 = 0x22; public const byte Digit6 = 0x23;
    public const byte Digit7 = 0x24; public const byte Digit8 = 0x25;
    public const byte Digit9 = 0x26; public const byte Digit0 = 0x27;
    public const byte Enter = 0x28;  public const byte Escape = 0x29;
    public const byte Backspace = 0x2A; public const byte Tab = 0x2B;
    public const byte Space = 0x2C;  public const byte Minus = 0x2D;
    public const byte Equal = 0x2E;  public const byte LeftBrace = 0x2F;
    public const byte RightBrace = 0x30; public const byte Backslash = 0x31;
    public const byte Semicolon = 0x33; public const byte Apostrophe = 0x34;
    public const byte Grave = 0x35;  public const byte Comma = 0x36;
    public const byte Period = 0x37; public const byte Slash = 0x38;
    public const byte CapsLock = 0x39;
    public const byte F1 = 0x3A;  public const byte F2 = 0x3B;
    public const byte F3 = 0x3C;  public const byte F4 = 0x3D;
    public const byte F5 = 0x3E;  public const byte F6 = 0x3F;
    public const byte F7 = 0x40;  public const byte F8 = 0x41;
    public const byte F9 = 0x42;  public const byte F10 = 0x43;
    public const byte F11 = 0x44; public const byte F12 = 0x45;
    public const byte PrintScreen = 0x46; public const byte ScrollLock = 0x47;
    public const byte Pause = 0x48; public const byte Insert = 0x49;
    public const byte Home = 0x4A;  public const byte PageUp = 0x4B;
    public const byte Delete = 0x4C; public const byte End = 0x4D;
    public const byte PageDown = 0x4E;
    public const byte Right = 0x4F; public const byte Left = 0x50;
    public const byte Down = 0x51;  public const byte Up = 0x52;
    public const byte NumLock = 0x53; public const byte KeypadSlash = 0x54;
    public const byte KeypadAsterisk = 0x55; public const byte KeypadMinus = 0x56;
    public const byte KeypadPlus = 0x57; public const byte KeypadEnter = 0x58;
    public const byte Keypad1 = 0x59; public const byte Keypad2 = 0x5A;
    public const byte Keypad3 = 0x5B; public const byte Keypad4 = 0x5C;
    public const byte Keypad5 = 0x5D; public const byte Keypad6 = 0x5E;
    public const byte Keypad7 = 0x5F; public const byte Keypad8 = 0x60;
    public const byte Keypad9 = 0x61; public const byte Keypad0 = 0x62;
    public const byte KeypadDot = 0x63;
    public const byte Mute = 0x7F; public const byte VolumeUp = 0x80;
    public const byte VolumeDown = 0x81;
}
```

See the [USB HID Usage Tables](https://usb.org/sites/default/files/hut1_5.pdf) for the
canonical key-code assignments.

## Structs

```csharp
[StructLayout(LayoutKind.Sequential)]
struct USBServerConfig
{
    [MarshalAs(UnmanagedType.LPStr)]
    public string? addr;                            // default "0.0.0.0:3241"
    public ulong connection_timeout_ms;             // default 30000 (30s)
    public ulong device_handler_connect_timeout_ms; // default 5000 (5s)
    public uint  write_batch_flush_interval_ms;     // default 1 (1ms)
}
```

All device state structs are passed **by value** to `Set<Device>DeviceState`.
Zeroed fields select defaults; a zeroed meta struct is equivalent to passing `NULL`.

```csharp
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
    [MarshalAs(UnmanagedType.ByValArray, SizeConst = 6)]
    public byte[] Reserved;
}

[StructLayout(LayoutKind.Sequential)]
struct DS4DeviceState
{
    public sbyte  LX, LY, RX, RY;
    public ushort Buttons;
    public byte   DPad;
    public byte   L2, R2;
    public ushort Touch1X, Touch1Y;
    public byte   Touch1Active;
    public ushort Touch2X, Touch2Y;
    public byte   Touch2Active;
    public short  GyroX, GyroY, GyroZ;
    public short  AccelX, AccelY, AccelZ;
}

[StructLayout(LayoutKind.Sequential)]
struct DSDeviceState
{
    public sbyte  LX, LY, RX, RY;
    public uint   Buttons;
    public byte   DPad;
    public byte   L2, R2;
    public ushort Touch1X, Touch1Y;
    public byte   Touch1Active;
    public byte   Touch1Tracking; // contact ID, verbatim to USB
    public ushort Touch2X, Touch2Y;
    public byte   Touch2Active;
    public byte   Touch2Tracking;
    public short  GyroX, GyroY, GyroZ;
    public short  AccelX, AccelY, AccelZ;
}

[StructLayout(LayoutKind.Sequential)]
struct NS2ProDeviceState
{
    public uint   Buttons;
    public ushort LX, LY, RX, RY;
    public short  AccelX, AccelY, AccelZ;
    public short  GyroX, GyroY, GyroZ;
}

[StructLayout(LayoutKind.Sequential)]
struct KeyboardDeviceState
{
    public byte Modifiers;
    [MarshalAs(UnmanagedType.ByValArray, SizeConst = 32)]
    public byte[] KeyBitmap; // 256-bit bitmap, one bit per HID key code
}

[StructLayout(LayoutKind.Sequential)]
struct MouseDeviceState
{
    public byte  Buttons;
    public short DX, DY;
    public short Wheel, Pan;
}
```

Meta structs are passed **by pointer**; fields left at zero value keep defaults:

```csharp
[StructLayout(LayoutKind.Sequential)]
struct DS4MetaState
{
    [MarshalAs(UnmanagedType.LPStr)] public string? SerialNumber;
    [MarshalAs(UnmanagedType.LPStr)] public string? Board;
    public byte   BatteryStatus;
    public double TemperatureCelsius;
    public double BatteryVoltage;
}

[StructLayout(LayoutKind.Sequential)]
struct DSMetaState
{
    [MarshalAs(UnmanagedType.LPStr)] public string? SerialNumber;
    [MarshalAs(UnmanagedType.LPStr)] public string? MACAddress;
    [MarshalAs(UnmanagedType.LPStr)] public string? Board;
    public byte   BatteryStatus;
    public double TemperatureCelsius;
    public double BatteryVoltage;
    [MarshalAs(UnmanagedType.LPStr)] public string? ShellColor; // 2-char code, e.g. "00", "Z1"
}

[StructLayout(LayoutKind.Sequential)]
struct NS2ProMetaState
{
    [MarshalAs(UnmanagedType.LPStr)] public string? SerialNumber;
    public byte   BatteryLevel;  // 0-9; 0 = use default
    public byte   Charging;      // 0 = not charging
    public byte   ExternalPower; // 0 = battery only
    public ushort BatteryVolts;  // mV; 0 = use default
}
```

[StructLayout(LayoutKind.Sequential)]
struct NS2ProOutputState
{
    [MarshalAs(UnmanagedType.ByValArray, SizeConst = 16)]
    public byte[] LeftRumble;
    [MarshalAs(UnmanagedType.ByValArray, SizeConst = 16)]
    public byte[] RightRumble;
    public byte Flags;        // bit 0 = rumble update, bit 1 = player LED update
    public byte PlayerLedMask;
}

[StructLayout(LayoutKind.Sequential)]
struct DSOutputState
{
    public byte Flags0;
    public byte Flags1;
    public byte RumbleSmall;
    public byte RumbleLarge;
    public byte VolumeHeadphones;
    public byte VolumeSpeaker;
    public byte VolumeMic;
    public byte AudioControl;
    public byte MuteLightMode;
    public byte MuteControl;
    [MarshalAs(UnmanagedType.ByValArray, SizeConst = 11)]
    public byte[] TriggerRight; // adaptive trigger effect mode + parameters
    [MarshalAs(UnmanagedType.ByValArray, SizeConst = 11)]
    public byte[] TriggerLeft;
    public uint HostTimestamp;
    public byte MotorPower;
    public byte AudioControl2;
    public byte Flags3;
    public byte HapticFilter;
    public byte UnkByte;
    public byte LightFade;
    public byte LightBrightness;
    public byte PlayerLeds;
    public byte LedRed;
    public byte LedGreen;
    public byte LedBlue;
}
```

## Callback delegates

```csharp
[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
delegate void VIIPERLogCallbackDelegate(VIIPERLogLevel level, [MarshalAs(UnmanagedType.LPStr)] string message);

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
delegate void Xbox360RumbleCallbackDelegate(nuint handle, byte leftMotor, byte rightMotor);

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
delegate void KeyboardLEDCallbackDelegate(nuint handle, byte leds);

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
delegate void DS4OutputCallbackDelegate(nuint handle, byte updateFlags, byte rumbleSmall, byte rumbleLarge,
    byte ledRed, byte ledGreen, byte ledBlue, byte flashOn, byte flashOff);

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
delegate void DSOutputCallbackDelegate(nuint handle, in DSOutputState output);

// Speaker PCM: exact host-written bytes, 4ch S16LE @48kHz, up to 392B per
// call. The pointer is only valid during the call — copy with Marshal.Copy.
// Runs on the audio thread: never block.
[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
delegate void DSAudioCallbackDelegate(nuint handle, IntPtr pcm, nuint length);

// Generation barrier: flush buffered PCM on fire.
[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
delegate void DSSpeakerResetCallbackDelegate(nuint handle);

// Rear voice-coil pair, 2ch S16LE @48kHz per call. Same lifetime/thread
// rules as the speaker callback; resampling to 3kHz is feeder-side.
[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
delegate void DSRealtimeHapticsCallbackDelegate(nuint handle, IntPtr pcm, nuint length);

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
delegate void NS2ProOutputCallbackDelegate(nuint handle, NS2ProOutputState output);
```

## P/Invoke declarations

### Server lifecycle

```csharp
static class LibVIIPER
{
    const string Lib = "libVIIPER";

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool NewUSBServer([In] ref USBServerConfig config,
        out nuint outHandle, VIIPERLogCallbackDelegate? logCallback);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool CloseUSBServer(nuint handle);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool CreateUSBBus(nuint serverHandle, ref uint busID);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool RemoveUSBBus(nuint serverHandle, uint busID);
```

### Xbox 360

```csharp
    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool CreateXbox360Device(nuint serverHandle, out nuint outDeviceHandle,
        uint busID, [MarshalAs(UnmanagedType.I1)] bool autoAttachLocalhost,
        ushort idVendor, ushort idProduct, byte xinputSubType);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool SetXbox360DeviceState(nuint deviceHandle, Xbox360DeviceState state);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool SetXbox360RumbleCallback(nuint deviceHandle, Xbox360RumbleCallbackDelegate? callback);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool RemoveXbox360Device(nuint deviceHandle);
```

### DualShock 4

```csharp
    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool CreateDS4Device(nuint serverHandle, out nuint outDeviceHandle,
        uint busID, [MarshalAs(UnmanagedType.I1)] bool autoAttachLocalhost,
        ushort idVendor, ushort idProduct, ref DS4MetaState meta);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool SetDS4DeviceState(nuint deviceHandle, DS4DeviceState state);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool SetDS4OutputCallback(nuint deviceHandle, DS4OutputCallbackDelegate? callback);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool SetDS4SpeakerCallback(nuint deviceHandle, DSAudioCallbackDelegate? callback);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool SetDS4SpeakerResetCallback(nuint deviceHandle, DSSpeakerResetCallbackDelegate? callback);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool SetDS4MicrophonePCM(nuint deviceHandle, [In] byte[] data, nuint length);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool SetDS4MetaState(nuint deviceHandle, ref DS4MetaState meta);

    // Raw input passthrough: report[] must be exactly 64 bytes with
    // report[0] == 0x01 (see DS4_RAW_REPORT_SIZE in the C header).
    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool SetDS4RawInputReport(nuint deviceHandle, [In] byte[] report, nuint length);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool ClearDS4RawInputReport(nuint deviceHandle);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool RemoveDS4Device(nuint deviceHandle);
```

### DualSense (and Edge)

Only `Device` and `EdgeDevice` variants exist in this branch
(no audio-only / gamepad-only / by-type / raw-input / atomic-audio
APIs).
Output arrives as the full `DSOutputState` struct; speaker PCM streams to
the audio-out callback and mic frames queue at 192B.

```csharp
    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool CreateDualSenseDevice(nuint serverHandle, out nuint outDeviceHandle,
        uint busID, [MarshalAs(UnmanagedType.I1)] bool autoAttachLocalhost,
        ushort idVendor, ushort idProduct, ref DSMetaState meta);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool CreateDualSenseEdgeDevice(nuint serverHandle, out nuint outDeviceHandle,
        uint busID, [MarshalAs(UnmanagedType.I1)] bool autoAttachLocalhost,
        ushort idVendor, ushort idProduct, ref DSMetaState meta);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool SetDualSenseDeviceState(nuint deviceHandle, DSDeviceState state);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool SetDualSenseOutputCallback(nuint deviceHandle, DSOutputCallbackDelegate? callback);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool SetDualSenseAudioOutCallback(nuint deviceHandle, DSAudioCallbackDelegate? callback);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool SetDualSenseSpeakerResetCallback(nuint deviceHandle, DSSpeakerResetCallbackDelegate? callback);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool SetDualSenseRealtimeHapticsCallback(nuint deviceHandle, DSRealtimeHapticsCallbackDelegate? callback);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool SetDualSenseMetaState(nuint deviceHandle, ref DSMetaState meta);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool SetDualSenseMicrophonePCM(nuint deviceHandle, [In] byte[] data, nuint length);

    // Raw input passthrough: report[] must be exactly 64 bytes with
    // report[0] == 0x01 (see DS_RAW_REPORT_SIZE in the C header).
    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool SetDualSenseRawInputReport(nuint deviceHandle, [In] byte[] report, nuint length);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool ClearDualSenseRawInputReport(nuint deviceHandle);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool RemoveDualSenseDevice(nuint deviceHandle);
```

### Switch 2 Pro

```csharp
    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool CreateNS2ProDevice(nuint serverHandle, out nuint outDeviceHandle,
        uint busID, [MarshalAs(UnmanagedType.I1)] bool autoAttachLocalhost,
        ushort idVendor, ushort idProduct, ref NS2ProMetaState meta);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool SetNS2ProDeviceState(nuint deviceHandle, NS2ProDeviceState state);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool SetNS2ProOutputCallback(nuint deviceHandle, NS2ProOutputCallbackDelegate? callback);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool RemoveNS2ProDevice(nuint deviceHandle);
```

### Keyboard

```csharp
    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool CreateKeyboardDevice(nuint serverHandle, out nuint outDeviceHandle,
        uint busID, [MarshalAs(UnmanagedType.I1)] bool autoAttachLocalhost,
        ushort idVendor, ushort idProduct);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool SetKeyboardDeviceState(nuint deviceHandle, KeyboardDeviceState state);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool SetKeyboardLEDCallback(nuint deviceHandle, KeyboardLEDCallbackDelegate? callback);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool RemoveKeyboardDevice(nuint deviceHandle);
```

### Mouse

```csharp
    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool CreateMouseDevice(nuint serverHandle, out nuint outDeviceHandle,
        uint busID, [MarshalAs(UnmanagedType.I1)] bool autoAttachLocalhost,
        ushort idVendor, ushort idProduct);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool SetMouseDeviceState(nuint deviceHandle, MouseDeviceState state);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool RemoveMouseDevice(nuint deviceHandle);
}
```

!!! note "Optional meta parameters"
    Create/meta functions taking `ref DS4MetaState` / `ref DSMetaState` /
    `ref NS2ProMetaState` accept `NULL` in C. In C#, pass a zeroed struct — every
    zeroed field selects the device default, which is exactly what `NULL` means.

## Complete example

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
        if (!LibVIIPER.NewUSBServer(ref conf, out nuint serverHandle, logCb))
            return 1;

        uint busID = 0;
        LibVIIPER.CreateUSBBus(serverHandle, ref busID);

        if (!LibVIIPER.CreateXbox360Device(serverHandle, out nuint deviceHandle, busID,
                autoAttachLocalhost: true, 0, 0, 0))
            return 1;

        Xbox360RumbleCallbackDelegate rumbleCb = RumbleCallback;
        LibVIIPER.SetXbox360RumbleCallback(deviceHandle, rumbleCb);

        Xbox360DeviceState state = new()
        {
            Buttons = Xbox360Buttons.A,
            LT = 128,
            LX = 20000,
        };
        LibVIIPER.SetXbox360DeviceState(deviceHandle, state);

        Console.WriteLine("Press Enter to exit...");
        Console.ReadLine();

        LibVIIPER.CloseUSBServer(serverHandle);
        return 0;
    }
}
```

## DualSense output example

The DualSense reports the full output state (rumble, volumes, adaptive
trigger effects, lightbar, player LEDs) through a single output callback:

```csharp
// Keep all delegates alive for the lifetime of the device!
DSOutputCallbackDelegate outputCb = (handle, in DSOutputState output) =>
{
    Console.WriteLine($"<- Rumble: {output.RumbleSmall}/{output.RumbleLarge}, " +
        $"LED: #{output.LedRed:X2}{output.LedGreen:X2}{output.LedBlue:X2}, " +
        $"Players: 0x{output.PlayerLeds:X2}, " +
        $"Triggers: R[0]={output.TriggerRight[0]} L[0]={output.TriggerLeft[0]}");
};
};

DSMetaState meta = new() { SerialNumber = "MY-DS-0001" };
if (!LibVIIPER.CreateDualSenseDevice(serverHandle, out nuint dsHandle, busID,
        autoAttachLocalhost: true, 0, 0, ref meta))
    return 1;

LibVIIPER.SetDualSenseOutputCallback(dsHandle, outputCb);
```

### Speaker PCM capture

```csharp
// Keep the delegate alive for the lifetime of the device!
DSAudioCallbackDelegate audioCb = (handle, pcm, length) =>
{
    // 4ch S16LE @48kHz: front L/R speaker + rear L/R haptics.
    var bytes = new byte[(int)length];
    Marshal.Copy(pcm, bytes, 0, bytes.Length);
    // hand bytes to your Opus encoder / ring — never block here.
};

LibVIIPER.SetDualSenseAudioOutCallback(dsHandle, audioCb);

// Flush previous-generation PCM on each barrier.
DSSpeakerResetCallbackDelegate resetCb = handle => { /* flush */ };
LibVIIPER.SetDualSenseSpeakerResetCallback(dsHandle, resetCb);

// Low-latency rear haptics (bypasses speaker-path batching).
DSRealtimeHapticsCallbackDelegate hapticsCb = (handle, pcm, length) =>
{
    var rear = new byte[(int)length];
    Marshal.Copy(pcm, rear, 0, rear.Length);
    // resample 48kHz -> 3kHz and forward ...
};
LibVIIPER.SetDualSenseRealtimeHapticsCallback(dsHandle, hapticsCb);
```

### Microphone feed

```csharp
// Exactly 192 bytes per call (48 frames of 2ch S16LE @48kHz); anything
// else returns false, as do calls while the host has not opened the mic
// (dropped, as on hardware). Opus-decode in your app, queue here.
var micFrame = new byte[192];
// ... fill from capture ...
LibVIIPER.SetDualSenseMicrophonePCM(dsHandle, micFrame, (nuint)micFrame.Length);
```

### Raw input passthrough

Pipe real controller bytes instead of synthesizing. The device serves the
report verbatim on interrupt IN and `GET_REPORT` until you clear it.

```csharp
// Exactly 64 bytes (DS_RAW_REPORT_SIZE), report[0] == 0x01. Call once per
// real report; anything else returns false.
var report = new byte[64];
report[0] = 0x01;
// ... copy a captured/piped USB input report into report ...
if (!LibVIIPER.SetDualSenseRawInputReport(dsHandle, report, (nuint)report.Length))
    return 1;

// Back to synthetic reports (the report counter resumes from its
// current, frozen value).
LibVIIPER.ClearDualSenseRawInputReport(dsHandle);
```

## DualShock 4 output example

The DualShock 4 output callback adds an `updateFlags` byte selecting which
groups changed (`0x01` rumble, `0x02` LED, `0x04` flash).

```csharp
// Keep all delegates alive for the lifetime of the device!
DS4OutputCallbackDelegate outputCb = (handle, updateFlags, rumbleSmall, rumbleLarge,
    ledRed, ledGreen, ledBlue, flashOn, flashOff) =>
{
    Console.WriteLine($"<- Rumble: {rumbleSmall}/{rumbleLarge}, " +
        $"LED: #{ledRed:X2}{ledGreen:X2}{ledBlue:X2}, Flash: {flashOn}/{flashOff}");
};

DS4MetaState meta = new() { SerialNumber = "MY-DS4-0001" };
if (!LibVIIPER.CreateDS4Device(serverHandle, out nuint ds4Handle, busID,
        autoAttachLocalhost: true, 0, 0, ref meta))
    return 1;

LibVIIPER.SetDS4OutputCallback(ds4Handle, outputCb);
```

### DualShock 4 speaker and microphone

```csharp
// Keep the delegate alive for the lifetime of the device!
DSAudioCallbackDelegate ds4AudioCb = (handle, pcm, length) =>
{
    // 2ch S16LE @32kHz, up to 132 bytes: the DS4's native rate, no
    // resampling on either side.
    var bytes = new byte[(int)length];
    Marshal.Copy(pcm, bytes, 0, bytes.Length);
    // hand bytes to your SBC encoder / ring — never block here.
};

LibVIIPER.SetDS4SpeakerCallback(ds4Handle, ds4AudioCb);

// Flush previous-generation PCM on each barrier.
DSSpeakerResetCallbackDelegate ds4ResetCb = handle => { /* flush */ };
LibVIIPER.SetDS4SpeakerResetCallback(ds4Handle, ds4ResetCb);

// Exactly 32 bytes per call (16 frames of mono S16LE @16kHz); anything
// else returns false, as do calls while the host has not opened the mic
// (dropped, as on hardware). SBC-decode in your app, queue here.
var ds4MicFrame = new byte[32];
// ... fill from capture ...
LibVIIPER.SetDS4MicrophonePCM(ds4Handle, ds4MicFrame, (nuint)ds4MicFrame.Length);
```

!!! warning "Never call into libVIIPER from a callback"
    Audio/output callbacks run on the USBIP server's background threads. Do not
    call `Set*` / `Create*` / `Remove*` functions from inside them — marshal the
    work to your own thread instead.

See also the device reference pages for the exact semantics of every input state,
meta state and callback:

- [Xbox 360 Controller](../devices/xbox360.md)
- [DualShock 4](../devices/dualshock4.md)
- [DualSense (and Edge)](../devices/dualsense.md)
- [Switch 2 Pro Controller](../devices/ns2pro.md)
- [Keyboard](../devices/keyboard.md)
- [Mouse](../devices/mouse.md)
