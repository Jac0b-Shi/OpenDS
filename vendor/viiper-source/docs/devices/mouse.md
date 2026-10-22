# HID Mouse

A standard 5-button mouse with vertical and horizontal scroll wheels.
Reports relative motion deltas.

All functions are part of the [libVIIPER C API](../libviiper/overview.md).

## API

| Function | Description |
| --- | --- |
| `CreateMouseDevice(serverHandle, &handle, busID, autoAttach, vid, pid)` | Create a virtual HID mouse |
| `SetMouseDeviceState(handle, state)` | Push an input state to the device |
| `RemoveMouseDevice(handle)` | Remove the device |

## Input state

```c
typedef struct {
    uint8_t Buttons;
    int16_t DX;
    int16_t DY;
    int16_t Wheel;
    int16_t Pan;
} MouseDeviceState;
```

- Buttons: bitfield — bits 0..4 for buttons 1..5
- `DX`/`DY`: relative motion deltas, -32768 to +32767
- `Wheel`: vertical wheel, positive = up
- `Pan`: horizontal wheel, positive = right

Motion and wheel deltas are consumed after each report and reset;
buttons persist until changed.

### Button flags

| Constant | Value | Button |
| --- | --- | --- |
| `MOUSE_BTN_LEFT` | `0x01` | Left |
| `MOUSE_BTN_RIGHT` | `0x02` | Right |
| `MOUSE_BTN_MIDDLE` | `0x04` | Middle |
| `MOUSE_BTN_BACK` | `0x08` | Back / Button 4 |
| `MOUSE_BTN_FORWARD` | `0x10` | Forward / Button 5 |