# DualShock 4 Controller

VIIPER emulates a USB-connected DualShock 4 v2 (CUH-ZCT2, PID `0x09CC`),
including sticks, triggers, buttons, touchpad, motion, lightbar, rumble,
and the UAC1 speaker/microphone audio interfaces
(IF0 control, IF1 2ch/32kHz OUT, IF2 1ch/16kHz IN, IF3 HID).

Audio is feeder-driven: speaker PCM is delivered to callbacks, and
`SetDS4MicrophonePCM` queues feeder mic frames (silence on underrun).
See the sections below.

All functions are part of the [libVIIPER C API](../libviiper/overview.md).

## API

| Function | Description |
| --- | --- |
| `CreateDS4Device(serverHandle, &handle, busID, autoAttach, vid, pid, meta)` | Create a virtual DualShock 4 |
| `SetDS4DeviceState(handle, state)` | Push an input state to the device |
| `SetDS4RawInputReport(handle, data, length)` | Pipe one exact 64B USB input report 0x01 (real bytes verbatim while set) |
| `ClearDS4RawInputReport(handle)` | Drop the raw passthrough, restore synthetic reports |
| `SetDS4OutputCallback(handle, cb)` | Register a callback for rumble, LED and flash output (`updateFlags` selects which fields changed) |
| `SetDS4SpeakerCallback(handle, cb)` | Register a callback for speaker PCM (exact host bytes, 2ch S16LE @32kHz) |
| `SetDS4SpeakerResetCallback(handle, cb)` | Register a callback fired once per streaming generation change (flush PCM) |
| `SetDS4MicrophonePCM(handle, data, length)` | Queue one 32B mic frame (mono S16LE @16kHz) |
| `SetDS4MetaState(handle, meta)` | Merge-update identity/battery metadata at runtime |
| `RemoveDS4Device(handle)` | Remove the device |

## Input state

```c
typedef struct {
    int8_t   LX;
    int8_t   LY;
    int8_t   RX;
    int8_t   RY;
    uint16_t Buttons;
    uint8_t  DPad;
    uint8_t  L2;
    uint8_t  R2;
    uint16_t Touch1X;
    uint16_t Touch1Y;
    uint8_t  Touch1Active;
    uint16_t Touch2X;
    uint16_t Touch2Y;
    uint8_t  Touch2Active;
    int16_t  GyroX;
    int16_t  GyroY;
    int16_t  GyroZ;
    int16_t  AccelX;
    int16_t  AccelY;
    int16_t  AccelZ;
} DS4DeviceState;
```

- Sticks: -128 to 127 per axis (-128=min, 0=center, 127=max)
- Triggers: 0-255 (0=not pressed, 255=fully pressed)
- Touch coordinates are clamped to the DS4 range: X **0..1920**, Y **0..942**

### Button constants

| Button | Hex Value |
| --- | --- |
| Square button | 0x0010 |
| Cross (X) button | 0x0020 |
| Circle button | 0x0040 |
| Triangle button | 0x0080 |
| L1 (Left bumper) | 0x0100 |
| R1 (Right bumper) | 0x0200 |
| L2 button | 0x0400 |
| R2 button | 0x0800 |
| Share button | 0x1000 |
| Options button | 0x2000 |
| L3 (Left stick button) | 0x4000 |
| R3 (Right stick button) | 0x8000 |
| PS button | 0x0001 |
| Touchpad click | 0x0002 |

### D-Pad constants

| D-Pad Direction | Hex Value |
| --- | --- |
| Up | 0x01 |
| Down | 0x02 |
| Left | 0x04 |
| Right | 0x08 |

### IMU (Gyro + Accelerometer)

IMU values are fixed-point:

- `GyroCountsPerDps = 16` → resolution `0.0625 deg/s`, max approx `2048 deg/s`
- `AccelCountsPerMS2 = 512` → resolution approx `0.00195 m/s2`, max approx `64 m/s2`

On device creation, the accelerometer is initialized to a controller lying flat
with gravity downwards (`AccelZ = -5023`, i.e. `round(-9.81 * 512)`).

### Input report byte map (USB report `0x01`, 64B)

| Bytes | Source | Notes |
| --- | --- | --- |
| `b[1:5]` | Feeder sticks | `LX/LY/RX/RY` + 128 |
| `b[5]` | Feeder D-pad + face | Hat nibble + Square/Cross/Circle/Triangle |
| `b[6]` | Feeder buttons | L1/R1/L2-click/R2-click/Share/Options/L3/R3 |
| `b[7]` | Feeder PS/touch + core counter | PS bit 0, touchpad-click bit 1, 6-bit report counter |
| `b[8:10]` | Feeder triggers | `L2`/`R2` analog |
| `b[10:12]` | Core timestamp | Synthesized (the dongle forwards controller values) |
| `b[12]` | Fixed `0x09` | Status: touchpad connected, no extension |
| `b[13:19]` | Feeder gyro | Raw counts |
| `b[19:25]` | Feeder accel | Raw counts |
| `b[30]` | Meta battery | Level nibble + cable/charging flag |
| `b[33:35]` | Fixed `0x01` | Synthesized (forwarded from the controller on hardware) |
| `b[35:43]` | Feeder touch | Coords; contact counter increments while active, `0x80` on release (unverified — DS4Dongle forwards controller bytes and documents no counter semantics) |
| rest | Zero | Reserved |

## Raw input passthrough

Feeders with a real controller pipe the exact 64B USB input report `0x01`
(report ID + 63B payload, matching DS4Dongle's BT `0x11` payload copy in
`main.cpp on_bt_data`): `SetDS4RawInputReport` serves it verbatim on
interrupt IN and `GET_REPORT` (real counter, timestamp, touch, battery);
`ClearDS4RawInputReport` restores the synthetic builder above. Never
setting raw keeps today's behavior; invalid lengths or IDs are rejected.
Over TCP, open `bus/{busId}/{deviceid}/raw` and write exact 64B frames
(`OpenRawStream`/`WriteFrame`); closing the stream keeps the last raw
(clear explicitly), and an invalid frame is skipped without ending the
stream.

While a raw report is set the synthetic builder stops running, so the core
report counter and sensor timestamp freeze and states pushed via
`SetDS4DeviceState` are buffered but not reported. Clearing raw resumes
synthetic reporting from those frozen values.

## Meta state

Optional metadata passed to `CreateDS4Device`. Fields left at
their zero value (`NULL`/`0`) use defaults. `SetDS4MetaState` applies the
same merge rules at runtime (only the meta struct pointer is needed,
as with the DualSense bindings).

## Meta state

Optional metadata passed to `CreateDS4Device`. Fields left at
their zero value (`NULL`/`0`) use defaults.

```c
typedef struct {
    const char* SerialNumber;       // NULL = use default (short hex zero-pads left)
    const char* Board;              // NULL = use default
    uint8_t     BatteryStatus;      // 0 = use default
    double      TemperatureCelsius; // 0 = use default
    double      BatteryVoltage;     // 0 = use default
} DS4MetaState;
```

## Output callback

Called when the host sends rumble or LED commands to the device. The host
sends ID+31B; headset/speaker/mic volume bytes are accepted but ignored
(no feeder channel, BT-side on the dongle).

```c
#define DS4_OUTPUT_UPDATE_RUMBLE 0x01u
#define DS4_OUTPUT_UPDATE_LED    0x02u
#define DS4_OUTPUT_UPDATE_FLASH  0x04u

typedef void (*DS4OutputCallback)(
    DS4DeviceHandle handle,
    uint8_t updateFlags,
    uint8_t rumbleSmall,
    uint8_t rumbleLarge,
    uint8_t ledRed,
    uint8_t ledGreen,
    uint8_t ledBlue,
    uint8_t flashOn,
    uint8_t flashOff
);
```

- `updateFlags` selects which groups changed (rumble / LED / flash)
- Rumble: 0-255 intensity values
- LED color: 0-255 per channel
- LED flash: units of 2.5 ms per value

Pass `NULL` to `SetDS4OutputCallback` to clear a previously registered callback.

## Isochronous timing

The isochronous audio endpoints are paced to the USB frame clock: one packet
per 1ms frame. This is required for correct playback — without it the host
audio engine consumes the stream as fast as it can submit URBs and anything
slaved to the audio clock runs at high speed. URBs may be pipelined; each
reply is held until its frames have elapsed.

## Speaker audio

The host streams speaker PCM to the `IF1` isochronous endpoint as
2ch S16LE @32kHz (the DS4's native rate, so no resampling is needed
anywhere), up to 132 bytes per transfer. `SetDS4SpeakerCallback`
delivers the exact host-written bytes; the buffer is only valid during the
call, so copy it, and never block (audio thread). There is no haptics
lane: the DS4 has no rear voice-coil pair.

Over TCP, open `bus/{busId}/{deviceid}/audio`: each message is a u16 LE
length followed by that many PCM bytes. Length `0xFFFF` marks a
speaker-reset barrier (stream generation change) with no payload.

## Microphone

The mic (`IF2`) streams mono S16LE @16kHz, exactly 32 bytes (16 samples)
per transfer — the real DS4 v2 ships exactly one frame per SOF, silence
included. `SetDS4MicrophonePCM` queues one feeder frame; anything else is
rejected, as are frames arriving while the host has not opened the mic
interface (dropped, as on DS4Dongle). Closing or reopening the interface
flushes the queue, so a reopen never replays stale PCM. The queue holds
32 frames drop-oldest; underruns serve silence. Over TCP, open
`bus/{busId}/{deviceid}/audio/mic` and write raw 32B frames (frames while
closed are dropped, the stream stays open).

## Speaker reset

`SetDS4SpeakerResetCallback` fires once per streaming generation
change: the host opened, closed, or re-alternated an audio interface.
Flush previous-generation PCM on fire (same barrier as the `0xFFFF` TCP
message). Pass `NULL` to clear.

## Audio control requests (UAC1)

Mute/volume for the speaker feature unit (`0x02`) and mic feature unit
(`0x05`), matching DS4Dongle (`usb.cpp`, ranges captured from a real
DS4 v2):

| Control | Speaker `0x02` | Mic `0x05` |
| --- | --- | --- |
| Mute `GET_*` | last-SET mute byte | last-SET mute byte |
| Volume `GET_CUR` | last-SET (`0xFF00` = -1dB at power-up) | last-SET (`0x0000` = 0dB at power-up) |
| Volume `GET_MIN` | `0xB700` (-73dB) | `0xE8C0` (-23.25dB) |
| Volume `GET_MAX` | `0xFF00` (-1dB) | `0x1800` (+24dB) |
| Volume `GET_RES` | `0x0001` (1dB) | `0x00C0` (0.75dB) |

The channel number is ignored (every request applies to master), and mute
answers every `GET_*` with the mute byte — both matching the dongle.
`GET_CUR` always echoes the last-SET value exactly: the Linux kernel
probes with a SET/GET round-trip and drops the mixer otherwise. The
power-up defaults stand in for the dongle's config seed (the core has no
config store). Volume/mute `SET`s are stored only; with no BT side there
is no controller state to update.

## Feature reports

GETs return the report payload **without** the report ID byte, matching
DS4Dongle (which strips the ID and trailing CRC from controller-forwarded
reports). Synthesized in the core: `0x02` calibration (36B, BT gyro
order pre-swapped to USB order), `0x10` status, `0x11` probe echo,
`0x12` serial-derived 15B, `0x81` 6B controller MAC derived from the
serial, `0xA3` board info (48B), `0xA4` telemetry (serial or
voltage/temperature by subcommand). Auth reports `0xF0`–`0xF3` stall —
there is no PS4 license proxy. Every other ID stalls: without a backing
controller there is nothing to forward to.

## Deliberate deltas from DS4Dongle

- The audio function is always exposed. The dongle hides it while the
  headset jack is empty (`audio_follow_jack` defaults on); the core has
  no jack state and cannot drop interfaces at runtime.
- No USB serial string (`ISerialNumber` 0), matching the reference
  default (`enable_usb_sn` off).
- HID `bInterval` is fixed at the stock `0x05` (no `polling_rate_mode`
  tunable).
- BT-side internals (SBC encode/decode, `0x11`/`0x17`/`0x13` building,
  volume-to-controller mapping, mic keepalive, jack detect, pico
  commands, wake keyboard, CDC) live in the feeder app or are out of
  scope — never in the core.
