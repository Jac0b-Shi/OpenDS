# Xbox 360 Controller

The Xbox 360 virtual gamepad emulates an XInput-compatible controller that most
operating systems and games understand out of the box.

All functions are part of the [libVIIPER C API](../libviiper/overview.md).

## API

| Function | Description |
| --- | --- |
| `CreateXbox360Device(serverHandle, &handle, busID, autoAttach, vid, pid, subType)` | Create a virtual Xbox 360 controller |
| `SetXbox360DeviceState(handle, state)` | Push an input state to the device |
| `SetXbox360RumbleCallback(handle, cb)` | Register a callback for rumble output |
| `RemoveXbox360Device(handle)` | Remove the device |

You can optionally specify a subtype if you wish to emulate a different type of controller.

### Subtypes

| Subtype | Value |
| --- | --- |
| Gamepad | 1 |
| Wheel | 2 |
| Arcade Stick | 3 |
| Flight Stick | 4 |
| Dance Pad | 5 |
| Guitar | 6 |
| Guitar Alternate | 7 |
| Drums | 8 |
| Rock Band Stage Kit | 9 |
| Guitar Bass | 11 |
| Rock Band Pro Keys | 15 |
| Arcade Pad | 19 |
| Turntable | 23 |
| Rock Band Pro Guitar | 25 |
| Disney Infinity or Lego Dimensions Portal | 33 |
| Skylanders Portal | 36 |

## Input state

```c
typedef struct {
    uint32_t Buttons;
    uint8_t  LT;
    uint8_t  RT;
    int16_t  LX;
    int16_t  LY;
    int16_t  RX;
    int16_t  RY;
    uint8_t  Reserved[6];
} Xbox360DeviceState;
```

- Triggers: 0-255 (0=not pressed, 255=fully pressed)
- Sticks: 0 is center, -32768 is min, 32767 is max
- The reserved bytes are zeroed for most subtypes; a few subtypes do put data here.

### Button flags

| Constant | Value |
| --- | --- |
| `XBOX360_BUTTON_DPAD_UP` | `0x0001` |
| `XBOX360_BUTTON_DPAD_DOWN` | `0x0002` |
| `XBOX360_BUTTON_DPAD_LEFT` | `0x0004` |
| `XBOX360_BUTTON_DPAD_RIGHT` | `0x0008` |
| `XBOX360_BUTTON_START` | `0x0010` |
| `XBOX360_BUTTON_BACK` | `0x0020` |
| `XBOX360_BUTTON_LTHUMB` | `0x0040` |
| `XBOX360_BUTTON_RTHUMB` | `0x0080` |
| `XBOX360_BUTTON_LSHOULDER` | `0x0100` |
| `XBOX360_BUTTON_RSHOULDER` | `0x0200` |
| `XBOX360_BUTTON_GUIDE` | `0x0400` |
| `XBOX360_BUTTON_A` | `0x1000` |
| `XBOX360_BUTTON_B` | `0x2000` |
| `XBOX360_BUTTON_X` | `0x4000` |
| `XBOX360_BUTTON_Y` | `0x8000` |

## Rumble callback

Called when the host sends rumble/motor commands to the device. The callback
replays the latest rumble state when it is first registered.

```c
typedef void (*Xbox360RumbleCallback)(Xbox360DeviceHandle handle, uint8_t leftMotor, uint8_t rightMotor);
```

Pass `NULL` to `SetXbox360RumbleCallback` to clear a previously registered callback.