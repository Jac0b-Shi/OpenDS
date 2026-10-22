# DualSense Controller

VIIPER emulates USB-connected DualSense and DualSense Edge gamepads,
including sticks, triggers, buttons, touchpad, motion, lightbar, rumble,
player LEDs, and the UAC1 speaker/microphone audio interfaces
(IF0 control, IF1 4ch/48kHz OUT, IF2 2ch/48kHz IN, IF3 HID).

This branch has no audio-only or gamepad-only variants: only
`CreateDualSenseDevice` and `CreateDualSenseEdgeDevice` exist.
Audio is feeder-driven: speaker PCM and the rear voice-coil haptics pair
are delivered to callbacks, and `SetDualSenseMicrophonePCM` queues feeder
mic frames (silence on underrun). See the sections below.

All functions are part of the [libVIIPER C API](../libviiper/overview.md).

## API

| Function | Description |
| --- | --- |
| `CreateDualSenseDevice(serverHandle, &handle, busID, autoAttach, vid, pid, meta)` | Create a virtual DualSense gamepad |
| `CreateDualSenseEdgeDevice(serverHandle, &handle, busID, autoAttach, vid, pid, meta)` | Create a virtual DualSense Edge gamepad |
| `SetDualSenseDeviceState(handle, state)` | Push an input state to the device |
| `SetDualSenseOutputCallback(handle, cb)` | Register a callback for the full output state (rumble, trigger effects, lightbar, player LEDs) |
| `SetDualSenseAudioOutCallback(handle, cb)` | Register a callback for speaker PCM (exact host bytes, 4ch S16LE @48kHz) |
| `SetDualSenseSpeakerResetCallback(handle, cb)` | Register a callback fired once per streaming generation change (flush PCM) |
| `SetDualSenseRealtimeHapticsCallback(handle, cb)` | Register a callback for the rear voice-coil pair (2ch S16LE @48kHz, low latency) |
| `SetDualSenseMetaState(handle, meta)` | Merge-update identity/battery metadata at runtime (USB serial refreshes too) |
| `SetDualSenseRawInputReport(handle, data, length)` | Pipe one exact 64B USB input report 0x01 (real bytes verbatim while set) |
| `ClearDualSenseRawInputReport(handle)` | Drop the raw passthrough, restore synthetic reports |
| `SetDualSenseMicrophonePCM(handle, data, length)` | Queue one 192B mic frame (2ch S16LE @48kHz) |
| `RemoveDualSenseDevice(handle)` | Remove the device |
Only one output callback may be active at a time; pass `NULL` to clear it.

## Input state

```c
typedef struct {
    int8_t   LX;
    int8_t   LY;
    int8_t   RX;
    int8_t   RY;
    uint32_t Buttons;
    uint8_t  DPad;
    uint8_t  L2;
    uint8_t  R2;
    uint16_t Touch1X;
    uint16_t Touch1Y;
    uint8_t  Touch1Active;
    uint8_t  Touch1Tracking; // feeder-supplied contact ID, verbatim to USB
    uint16_t Touch2X;
    uint16_t Touch2Y;
    uint8_t  Touch2Active;
    uint8_t  Touch2Tracking;
    int16_t  GyroX;
    int16_t  GyroY;
    int16_t  GyroZ;
    int16_t  AccelX;
    int16_t  AccelY;
    int16_t  AccelZ;
} DSDeviceState;
```

Button bits:

| Control | Value |
| --- | --- |
| Square | `0x00000010` |
| Cross | `0x00000020` |
| Circle | `0x00000040` |
| Triangle | `0x00000080` |
| L1 / R1 | `0x00000100` / `0x00000200` |
| L2 / R2 | `0x00000400` / `0x00000800` |
| Create / Options | `0x00001000` / `0x00002000` |
| L3 / R3 | `0x00004000` / `0x00008000` |
| PS | `0x00010000` |
| Touchpad click | `0x00020000` |
| Mic mute | `0x00040000` |
| Edge LFn / RFn | `0x00100000` / `0x00200000` |
| Edge L4 / R4 | `0x00400000` / `0x00800000` |

D-pad bits are **Up** `0x01`, **Down** `0x02`, **Left** `0x04`, **Right** `0x08`.

### Input report byte map (USB report `0x01`, 64B)

| Bytes | Source | Notes |
| --- | --- | --- |
| `b[1:5]` | Feeder sticks | `LX/LY/RX/RY` + 128 |
| `b[5:7]` | Feeder triggers | `L2`/`R2` analog |
| `b[7]` | Core sequence | `++` per report |
| `b[8:11]` | Feeder D-pad + buttons | Hat nibble + face/shoulder/PS bits |
| `b[11:16]` | Zero | Touch timestamps (no hardware capture) |
| `b[16:28]` | Feeder gyro/accel | Raw counts |
| `b[28:32]` | Core timestamp | µs since boot × 3 |
| `b[32]` | Meta temperature | int8 Celsius from metadata (controller-populated on hardware) |
| `b[33:41]` | Feeder touch | Coords + verbatim tracking IDs, inactive mask |
| `b[41:48]` | Zero | Trigger-effect echo (no hardware capture) |
| `b[49]` | Fixed `0x10` | Historical reserved marker |
| `b[50:52]` | Zero | Reserved |
| `b[53]` | Meta battery | `BatteryStatus` |
| `b[54]` bit 2 | Synthesized mute LED | From the last host output (see below); bit 0 (headset) and the rest stay zero |
| `b[55:63]` | Zero | Audio/headset flags, reserved tail |

The mute LED bit follows the host-converged value: the last output with
`AllowMuteLight` set and mode `Off`/`On`/`Breathing` turns the bit off/on;
`DoNothing`/`NoAction` leave it unchanged. The dongle forwards the
controller-reported bit over BT; the virtual device has no backing
controller, so it echoes what the host asked for.

## Raw input passthrough

Feeders with a real controller pipe the exact 64B USB input report `0x01`
(report ID + 63B payload, e.g. captured hardware or DS5Dongle `main.cpp`
passthrough): `SetDualSenseRawInputReport` serves it verbatim on interrupt
IN and `GET_REPORT` (real seq, timestamps, touch, trigger echo, headset,
AES all preserved). `ClearDualSenseRawInputReport` restores the synthetic
builder above. Never setting raw keeps today's behavior; invalid lengths or
IDs are rejected. Over TCP, open `bus/{busId}/{deviceid}/raw` and write
exact 64B frames (`OpenRawStream`/`WriteFrame`); closing the stream keeps
the last raw (clear explicitly), and an invalid frame is skipped without
ending the stream.

While a raw report is set the synthetic builder stops running, so the core
sequence counter and sensor timestamp freeze and states pushed via
`SetDualSenseDeviceState` are buffered but not reported. Clearing raw
resumes synthetic reporting from those frozen values.

## Meta state

Optional metadata passed to the create functions. Fields left at
their zero value (`NULL`/`0`) use defaults.

```c
typedef struct {
    const char* SerialNumber;       // NULL = use default
    const char* MACAddress;         // NULL = use default
    const char* Board;              // NULL = use default
    uint8_t     BatteryStatus;      // 0 = use default
    double      TemperatureCelsius; // 0 = use default
    double      BatteryVoltage;     // 0 = use default
    const char* ShellColor;         // NULL = use default (2-char code, e.g. "00", "Z1")
} DSMetaState;
```

### Shell colors

`DS_SHELL_COLOR_*` constants are defined for hardware variants, e.g.
`DS_SHELL_COLOR_WHITE` (`"00"`), `DS_SHELL_COLOR_BLACK` (`"01"`),
`DS_SHELL_COLOR_GOD_OF_WAR_RAGNAROK` (`"Z1"`), `DS_SHELL_COLOR_ASTRO_BOT` (`"Z3"`),
`DS_SHELL_COLOR_GENSHIN_IMPACT` (`"ZE"`), and more — see `libVIIPER.h`.

## Output callback

The callback delivers the full 0x02 output state: flags, rumble, volumes,
adaptive trigger effects (11 bytes per trigger: mode + curve parameters),
lightbar, player LEDs, and mic-mute light. Pointer valid during the call.
Edge hosts send ID+63B; the trailing 16 reserved bytes are ignored.

```c
typedef struct {
    uint8_t  Flags0;
    uint8_t  Flags1;
    uint8_t  RumbleSmall;
    uint8_t  RumbleLarge;
    uint8_t  VolumeHeadphones;
    uint8_t  VolumeSpeaker;
    uint8_t  VolumeMic;
    uint8_t  AudioControl;
    uint8_t  MuteLightMode;
    uint8_t  MuteControl;
    uint8_t  TriggerRight[11];
    uint8_t  TriggerLeft[11];
    uint32_t HostTimestamp;
    uint8_t  MotorPower;
    uint8_t  AudioControl2;
    uint8_t  Flags3;
    uint8_t  HapticFilter;
    uint8_t  UnkByte;
    uint8_t  LightFade;
    uint8_t  LightBrightness;
    uint8_t  PlayerLeds;
    uint8_t  LedRed;
    uint8_t  LedGreen;
    uint8_t  LedBlue;
} DSOutputState;

typedef void (*DSOutputCallback)(DSDeviceHandle handle, const DSOutputState* output);
```

Pass `NULL` to `SetDualSenseOutputCallback` to clear a previously registered callback.

## Isochronous timing

The isochronous audio endpoints are paced to the USB frame clock: one packet
per 1ms frame. This is required for correct playback — without it the host
audio engine consumes the stream as fast as it can submit URBs and anything
slaved to the audio clock (games, video players) runs at high speed. URBs may
be pipelined; each reply is held until its frames have elapsed, and a
multi-packet URB carries one real frame per packet.

## Speaker audio

The host streams speaker/haptics PCM to the `IF1` isochronous endpoint as
4ch S16LE @48kHz (front L/R speaker + rear L/R haptics, channel config
`0x0033`), up to 392 bytes per transfer. `SetDualSenseAudioOutCallback`
delivers the exact host-written bytes; the buffer is only valid during the
call, so copy it, and never block (audio thread).

Over TCP, open `bus/{busId}/{deviceid}/audio`: each message is a u16 LE
length followed by that many PCM bytes. Length `0xFFFF` marks a
speaker-reset barrier (stream generation change) with no payload.

## Microphone

The mic (`IF2`) streams 2ch S16LE @48kHz, exactly 192 bytes (48 frames)
per transfer. `SetDualSenseMicrophonePCM` queues one feeder frame;
anything else is rejected. Frames arriving while the host has not opened
the mic interface are dropped (as on DS5Dongle); closing or reopening the
interface flushes the queue, so a reopen never replays stale PCM. The
queue holds 32 frames drop-oldest; underruns serve silence. Over TCP,
open `bus/{busId}/{deviceid}/audio/mic` and write raw 192B frames
(frames while closed are dropped, the stream stays open).

## Speaker reset

`SetDualSenseSpeakerResetCallback` fires once per streaming generation
change: the host opened, closed, or re-alternated an audio interface.
Flush previous-generation PCM on fire (same barrier as the `0xFFFF` TCP
message). Pass `NULL` to clear.

## Audio control requests (UAC1)

Mute/volume for the speaker feature unit (`0x02`) and mic feature unit
(`0x05`), matching DS5Dongle (`usb.cpp`):

| Control | Speaker `0x02` | Mic `0x05` |
| --- | --- | --- |
| Mute `GET_*` | last-SET mute byte | last-SET mute byte |
| Volume `GET_CUR` | last-SET (`0x0000` = 0dB at power-up) | last-SET (`0x3000` = +48dB at power-up) |
| Volume `GET_MIN` | `0x009C` (-100dB) | `0x0000` (0dB) |
| Volume `GET_MAX` | `0x0000` (0dB) | `0x3000` (+48dB) |
| Volume `GET_RES` | `0x0001` (1/256dB) | `0x007A` (122/256dB) |

The channel number is ignored (every request applies to master), and mute
answers every `GET_*` with the mute byte — both matching the dongle. One
accepted delta: speaker `GET_CUR` returns the last-SET value (0dB default)
rather than the dongle config-derived default — the core has no config
store. Volume/mute `SET`s are stored only; with no BT side there is no
controller state to update.

## Realtime haptics

`SetDualSenseRealtimeHapticsCallback` delivers the rear voice-coil pair
(2ch S16LE @48kHz) per USB transfer for minimal-latency forwarding —
the same audio without the speaker path's batching delay. Resampling to
the 3kHz haptics rate is feeder-side. Same buffer/thread rules as speaker
PCM. Over TCP, open `bus/{busId}/{deviceid}/audio/haptics` (same framing,
barriers included).

## Trigger effects

Each adaptive trigger carries an 11-byte block (`TriggerRight`/`TriggerLeft`
in `DSOutputState`): byte 0 is the effect mode, bytes 1–10 are mode
parameters. The core delivers them verbatim (as DS5Dongle does); decode
with `RightTriggerEffect()`/`LeftTriggerEffect()`, which split mode from
parameters. Mode semantics follow the community reverse-engineered `0x02`
layout — the feeder maps modes as before. `MicLED()` and
`LightbarCustom()` expose the mute-light mode and lightbar-takeover flag.

## Feature reports

Every descriptor-listed feature ID resolves at its exact length:
`0x05` calibration, `0x09` pairing (live MAC), `0x20` firmware,
`0x80`/`0x81` subcommands, and zero stubs elsewhere. Synthesized, not
forwarded: `0x20` is built from metadata (`BuildTime` date/time strings,
hardware type, `HwInfo`, firmware version), `0x81` answers the stored
`0x80` subcommand from metadata (serial, battery voltage, temperature),
and `0x09` carries the live MAC. Edge-only IDs (`0x60`–`0x7B`) serve
static stubs — without a backing controller there is no unlock handshake
and no NAK-until-ready gate. Edge `0x65` echoes the `0x20` firmware body
until the host SETs its own payload, which is then served back verbatim.

## Deliberate deltas from DS5Dongle

- USB serial strings are served verbatim from metadata (no trailing `2`
  cache-buster the dongle appends for Windows).
- HID `bInterval` is fixed at 1 (no `polling_rate_mode` tunable).
- BT-side internals (Opus, resampling, `0x35`/`0x36`/`0x39` building,
  profile prefetch, pico commands, wake keyboard, CDC) live in the
  feeder app or are out of scope — never in the core.
