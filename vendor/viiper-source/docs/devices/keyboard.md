# HID Keyboard

A full-featured HID keyboard with N-key rollover using a 256-bit key bitmap,
plus LED status feedback (NumLock, CapsLock, ScrollLock).

All functions are part of the [libVIIPER C API](../libviiper/overview.md).

## API

| Function | Description |
| --- | --- |
| `CreateKeyboardDevice(serverHandle, &handle, busID, autoAttach, vid, pid)` | Create a virtual HID keyboard |
| `SetKeyboardDeviceState(handle, state)` | Push an input state to the device |
| `SetKeyboardLEDCallback(handle, cb)` | Register a callback for LED state changes |
| `RemoveKeyboardDevice(handle)` | Remove the device |

## Input state

```c
typedef struct {
    uint8_t Modifiers;
    uint8_t KeyBitmap[32]; /* 256-bit bitmap, one bit per HID key code */
} KeyboardDeviceState;
```

### Modifier flags

| Constant | Value | Key |
| --- | --- | --- |
| `KB_MOD_LEFT_CTRL` | `0x01` | Left Control |
| `KB_MOD_LEFT_SHIFT` | `0x02` | Left Shift |
| `KB_MOD_LEFT_ALT` | `0x04` | Left Alt |
| `KB_MOD_LEFT_GUI` | `0x08` | Left GUI (Win/Cmd) |
| `KB_MOD_RIGHT_CTRL` | `0x10` | Right Control |
| `KB_MOD_RIGHT_SHIFT` | `0x20` | Right Shift |
| `KB_MOD_RIGHT_ALT` | `0x40` | Right Alt |
| `KB_MOD_RIGHT_GUI` | `0x80` | Right GUI (Win/Cmd) |

Key codes in `KeyBitmap` follow the [USB HID Usage Tables](https://usb.org/sites/default/files/hut1_5.pdf) (page 83, Keyboard/Keypad page).

## LED callback

Called when the host changes keyboard LED state.

```c
typedef void (*KeyboardLEDCallback)(KeyboardDeviceHandle handle, uint8_t leds);
```

### LED flags

| Constant | Value |
| --- | --- |
| `KB_LED_NUM_LOCK` | `0x01` |
| `KB_LED_CAPS_LOCK` | `0x02` |
| `KB_LED_SCROLL_LOCK` | `0x04` |
| `KB_LED_COMPOSE` | `0x08` |
| `KB_LED_KANA` | `0x10` |

Pass `NULL` to `SetKeyboardLEDCallback` to clear a previously registered callback.