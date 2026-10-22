package main

/*
#include <stdint.h>
#include <stdlib.h>

typedef uintptr_t USBServerHandle;

typedef uintptr_t DS4DeviceHandle;

#define DS4_BUTTON_SQUARE    0x0010u
#define DS4_BUTTON_CROSS     0x0020u
#define DS4_BUTTON_CIRCLE    0x0040u
#define DS4_BUTTON_TRIANGLE  0x0080u
#define DS4_BUTTON_L1        0x0100u
#define DS4_BUTTON_R1        0x0200u
#define DS4_BUTTON_L2        0x0400u
#define DS4_BUTTON_R2        0x0800u
#define DS4_BUTTON_SHARE     0x1000u
#define DS4_BUTTON_OPTIONS   0x2000u
#define DS4_BUTTON_L3        0x4000u
#define DS4_BUTTON_R3        0x8000u
#define DS4_BUTTON_PS        0x0001u
#define DS4_BUTTON_TOUCHPAD  0x0002u

#define DS4_DPAD_UP        0x00u
#define DS4_DPAD_UP_RIGHT  0x01u
#define DS4_DPAD_RIGHT     0x02u
#define DS4_DPAD_DOWN_RIGHT 0x03u
#define DS4_DPAD_DOWN      0x04u
#define DS4_DPAD_DOWN_LEFT 0x05u
#define DS4_DPAD_LEFT      0x06u
#define DS4_DPAD_UP_LEFT   0x07u
#define DS4_DPAD_NEUTRAL   0x08u

// Exact size of a USB input report 0x01 (report ID + 63B payload).
#define DS4_RAW_REPORT_SIZE 64

#define DS4_OUTPUT_UPDATE_RUMBLE 0x01u
#define DS4_OUTPUT_UPDATE_LED    0x02u
#define DS4_OUTPUT_UPDATE_FLASH    0x04u

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

typedef struct {
	const char* SerialNumber;       // NULL = use default
	const char* Board;              // NULL = use default
	uint8_t     BatteryStatus;      // 0 = use default
	double      TemperatureCelsius; // 0 = use default
	double      BatteryVoltage;     // 0 = use default
} DS4MetaState;

typedef void (*DS4OutputCallback)(DS4DeviceHandle handle,uint8_t updateFlags, uint8_t rumbleSmall, uint8_t rumbleLarge, uint8_t ledRed, uint8_t ledGreen, uint8_t ledBlue, uint8_t flashOn, uint8_t flashOff);

typedef void (*DS4AudioCallback)(DS4DeviceHandle handle, const uint8_t* pcm, size_t length);

typedef void (*DS4SpeakerResetCallback)(DS4DeviceHandle handle);

static void viiper_call_ds4_output(DS4OutputCallback fn, DS4DeviceHandle handle,uint8_t updateFlags, uint8_t rumbleSmall, uint8_t rumbleLarge, uint8_t ledRed, uint8_t ledGreen, uint8_t ledBlue, uint8_t flashOn, uint8_t flashOff) {
	fn(handle, updateFlags, rumbleSmall, rumbleLarge, ledRed, ledGreen, ledBlue, flashOn, flashOff);
}

static void viiper_call_ds4_audio(DS4AudioCallback fn, DS4DeviceHandle handle, const uint8_t* pcm, size_t length) {
	fn(handle, pcm, length);
}

static void viiper_call_ds4_speaker_reset(DS4SpeakerResetCallback fn, DS4DeviceHandle handle) {
	fn(handle);
}

*/
import "C"
import (
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"runtime/cgo"
	"slices"
	"sync"
	"unsafe"

	"github.com/DualSenseClient/VIIPER/device"
	"github.com/DualSenseClient/VIIPER/device/dualshock4"
	"github.com/DualSenseClient/VIIPER/internal/server/api"
)

// CreateDS4Device creates a new DualShock 4 device on the bus with the given ID on the server associated with the given handle.
// @param serverHandle Handle to the USB server.
// @param outDeviceHandle Output parameter for the created device handle.
// @param busID ID of the bus to add the device to.
// @param autoAttachLocalhost If true, the device will be automatically attached to a USBIP-Client/Driver running on THIS machine.
// @param idVendor Optional USB vendor ID (0 = default).
// @param idProduct Optional USB product ID (0 = default).
// @param meta Optional pointer to initial device metadata. Pass NULL to use defaults.
//
//export CreateDS4Device
func CreateDS4Device(
	serverHandle C.USBServerHandle,
	outDeviceHandle *C.DS4DeviceHandle,
	busID uint32,
	autoAttachLocalhost bool,
	idVendor uint16,
	idProduct uint16,
	meta *C.DS4MetaState,
) bool {
	sh := cgo.Handle(serverHandle)
	shw, ok := sh.Value().(*usbServerHandleWrapper)
	if !ok {
		return false
	}
	bus := shw.s.GetBus(busID)
	if bus == nil {
		return false
	}

	opts := &device.CreateOptions{}
	if idVendor != 0 {
		opts.IDVendor = &idVendor
	}
	if idProduct != 0 {
		opts.IDProduct = &idProduct
	}
	if meta != nil {
		goMeta := dualshock4.MetaState{
			SerialNumber:       goStringOrEmpty(meta.SerialNumber),
			Board:              goStringOrEmpty(meta.Board),
			BatteryStatus:      uint8(meta.BatteryStatus),
			TemperatureCelsius: float64(meta.TemperatureCelsius),
			BatteryVoltage:     float64(meta.BatteryVoltage),
		}
		b, err := json.Marshal(goMeta)
		if err != nil {
			return false
		}
		opts.DeviceSpecific = string(b)
	}

	d, err := dualshock4.New(opts)
	if err != nil {
		return false
	}
	devCtx, err := bus.Add(d)
	if err != nil {
		return false
	}
	exportMeta := device.GetDeviceMeta(devCtx)
	if exportMeta == nil {
		return false
	}

	if autoAttachLocalhost {
		err := api.AttachLocalhostClient(
			context.Background(),
			exportMeta,
			shw.s.GetListenPort(),
			true,
			slog.Default(),
		)
		if err != nil {
			slog.Error("failed to auto-attach localhost client", "error", err)
			return false
		}
	}

	handleWrapper := &deviceHandleWrapper{
		device:     d,
		exportMeta: exportMeta,
		usbServer:  shw,
	}
	*outDeviceHandle = C.DS4DeviceHandle(cgo.NewHandle(handleWrapper))

	shw.mtx.Lock()
	defer shw.mtx.Unlock()
	shw.deviceHandles[busID] = append(shw.deviceHandles[busID], deviceHandle(*outDeviceHandle))
	return true
}

// SetDS4DeviceState updates the input state of the DualShock 4 device associated with the given handle.
// @param handle Handle to the DS4 device.
// @param state New input state to set on the device.
//
//export SetDS4DeviceState
func SetDS4DeviceState(handle C.DS4DeviceHandle, state C.DS4DeviceState) bool {
	dh := cgo.Handle(handle)
	dhw, ok := dh.Value().(*deviceHandleWrapper)
	if !ok {
		return false
	}
	ds4device, ok := dhw.device.(*dualshock4.DualShock4)
	if !ok {
		return false
	}
	s := &dualshock4.InputState{
		LX:           int8(state.LX),
		LY:           int8(state.LY),
		RX:           int8(state.RX),
		RY:           int8(state.RY),
		Buttons:      uint16(state.Buttons),
		DPad:         uint8(state.DPad),
		L2:           uint8(state.L2),
		R2:           uint8(state.R2),
		Touch1X:      uint16(state.Touch1X),
		Touch1Y:      uint16(state.Touch1Y),
		Touch1Active: state.Touch1Active != 0,
		Touch2X:      uint16(state.Touch2X),
		Touch2Y:      uint16(state.Touch2Y),
		Touch2Active: state.Touch2Active != 0,
		GyroX:        int16(state.GyroX),
		GyroY:        int16(state.GyroY),
		GyroZ:        int16(state.GyroZ),
		AccelX:       int16(state.AccelX),
		AccelY:       int16(state.AccelY),
		AccelZ:       int16(state.AccelZ),
	}
	ds4device.UpdateInputState(s)
	return true
}

// SetDS4OutputCallback sets a callback to be invoked when the host sends output (rumble/LED) commands to the device.
// @param handle Handle to the DS4 device.
// @param callback Callback receiving updateFlags, rumbleSmall, rumbleLarge, ledRed, ledGreen, ledBlue, flashOn, flashOff. Pass NULL to clear.
//
//export SetDS4OutputCallback
func SetDS4OutputCallback(handle C.DS4DeviceHandle, cb C.DS4OutputCallback) bool {
	dh := cgo.Handle(handle)
	dhw, ok := dh.Value().(*deviceHandleWrapper)
	if !ok {
		return false
	}
	ds4device, ok := dhw.device.(*dualshock4.DualShock4)
	if !ok {
		return false
	}
	if cb == nil {
		ds4device.SetOutputCallback(nil)
		return true
	}
	ds4device.SetOutputCallback(func(out dualshock4.OutputState) {
		C.viiper_call_ds4_output(cb, handle,
			C.uint8_t(out.UpdateFlags),
			C.uint8_t(out.RumbleSmall),
			C.uint8_t(out.RumbleLarge),
			C.uint8_t(out.LedRed),
			C.uint8_t(out.LedGreen),
			C.uint8_t(out.LedBlue),
			C.uint8_t(out.FlashOn),
			C.uint8_t(out.FlashOff),
		)
	})
	return true
}

// ds4AudioUnsubs tracks lib speaker-stream subscriptions per device handle
// so re-registering replaces the old pumps and removal stops them.
var (
	ds4AudioUnsubs   = map[deviceHandle]func(){}
	ds4AudioUnsubsMu sync.Mutex
)

func clearDS4AudioSub(h deviceHandle) {
	ds4AudioUnsubsMu.Lock()
	defer ds4AudioUnsubsMu.Unlock()
	if unsub, ok := ds4AudioUnsubs[h]; ok {
		unsub()
		delete(ds4AudioUnsubs, h)
	}
}

func trackDS4AudioSub(h deviceHandle, unsub func()) {
	ds4AudioUnsubsMu.Lock()
	defer ds4AudioUnsubsMu.Unlock()
	ds4AudioUnsubs[h] = unsub
}

// SetDS4SpeakerCallback sets a callback invoked with the exact bytes the
// host writes to the speaker endpoint (2ch S16LE @32kHz, up to 132B).
// The buffer is only valid during the call; copy it. Invoked on the audio
// thread: never block. Pass NULL to clear.
//
//export SetDS4SpeakerCallback
func SetDS4SpeakerCallback(handle C.DS4DeviceHandle, cb C.DS4AudioCallback) bool {
	dh := cgo.Handle(handle)
	dhw, ok := dh.Value().(*deviceHandleWrapper)
	if !ok {
		return false
	}
	ds4device, ok := dhw.device.(*dualshock4.DualShock4)
	if !ok {
		return false
	}
	clearDS4AudioSub(deviceHandle(handle))
	if cb == nil {
		return true
	}
	ch, unsub := ds4device.SubscribeSpeaker()
	trackDS4AudioSub(deviceHandle(handle), unsub)
	go func() {
		for ev := range ch {
			if ev.Reset || len(ev.PCM) == 0 {
				continue
			}
			pcm := ev.PCM
			C.viiper_call_ds4_audio(cb, handle,
				(*C.uint8_t)(unsafe.Pointer(&pcm[0])),
				C.size_t(len(pcm)),
			)
		}
	}()
	return true
}

// RemoveDS4Device removes the DualShock 4 device associated with the given handle from the server.
// @param handle Handle to the DS4 device to remove.
//
//export RemoveDS4Device
func RemoveDS4Device(handle C.DS4DeviceHandle) bool {
	dh := cgo.Handle(handle)
	dhw, ok := dh.Value().(*deviceHandleWrapper)
	if !ok {
		return false
	}
	if err := dhw.usbServer.s.RemoveDeviceByID(dhw.exportMeta.BusID, fmt.Sprintf("%d", dhw.exportMeta.DevID)); err != nil {
		return false
	}

	shw := dhw.usbServer
	busID := dhw.exportMeta.BusID

	shw.mtx.Lock()
	defer shw.mtx.Unlock()
	shw.deviceHandles[busID] = slices.DeleteFunc(shw.deviceHandles[busID], func(h deviceHandle) bool {
		return h == deviceHandle(handle)
	})
	clearDS4AudioSub(deviceHandle(handle))
	clearDS4ResetSub(deviceHandle(handle))
	dh.Delete()

	return true
}

// SetDS4MicrophonePCM queues one feeder mic frame (exactly 32B: 16 frames
// of mono S16LE @16kHz) for EP2 IN. Returns false for any other length,
// or while the host has not opened the mic interface (frames arriving
// while closed are dropped, as on DS4Dongle). Full queues drop oldest
// first; underruns serve silence.
//
//export SetDS4MicrophonePCM
func SetDS4MicrophonePCM(handle C.DS4DeviceHandle, data *C.uint8_t, length C.size_t) bool {
	dh := cgo.Handle(handle)
	dhw, ok := dh.Value().(*deviceHandleWrapper)
	if !ok {
		return false
	}
	ds4device, ok := dhw.device.(*dualshock4.DualShock4)
	if !ok {
		return false
	}
	if data == nil || length != 32 {
		return false
	}
	frame := C.GoBytes(unsafe.Pointer(data), C.int(length))
	return ds4device.QueueMicrophonePCM(frame)
}

// SetDS4RawInputReport pipes one exact 64B USB input report 0x01
// (real controller bytes). While set, interrupt IN and GET_REPORT serve it
// verbatim; ClearDS4RawInputReport restores synthetic reports.
// Returns false for any other length or wrong report ID.
//
//export SetDS4RawInputReport
func SetDS4RawInputReport(handle C.DS4DeviceHandle, data *C.uint8_t, length C.size_t) bool {
	dh := cgo.Handle(handle)
	dhw, ok := dh.Value().(*deviceHandleWrapper)
	if !ok {
		return false
	}
	ds4device, ok := dhw.device.(*dualshock4.DualShock4)
	if !ok {
		return false
	}
	if data == nil || length != C.DS4_RAW_REPORT_SIZE {
		return false
	}
	raw := C.GoBytes(unsafe.Pointer(data), C.int(length))
	return ds4device.SetRawInputReport(raw)
}

// ClearDS4RawInputReport drops the raw passthrough and restores
// synthetic input reports.
//
//export ClearDS4RawInputReport
func ClearDS4RawInputReport(handle C.DS4DeviceHandle) bool {
	dh := cgo.Handle(handle)
	dhw, ok := dh.Value().(*deviceHandleWrapper)
	if !ok {
		return false
	}
	ds4device, ok := dhw.device.(*dualshock4.DualShock4)
	if !ok {
		return false
	}
	ds4device.ClearRawInputReport()
	return true
}

// SetDS4MetaState merge-updates identity/battery metadata at runtime.
// Fields left at zero value keep their current values; a nil meta fails.
//
//export SetDS4MetaState
func SetDS4MetaState(handle C.DS4DeviceHandle, meta *C.DS4MetaState) bool {
	dh := cgo.Handle(handle)
	dhw, ok := dh.Value().(*deviceHandleWrapper)
	if !ok {
		return false
	}
	ds4device, ok := dhw.device.(*dualshock4.DualShock4)
	if !ok {
		return false
	}
	if meta == nil {
		return false
	}
	ds4device.MergeMetaState(dualshock4.MetaState{
		SerialNumber:       goStringOrEmpty(meta.SerialNumber),
		Board:              goStringOrEmpty(meta.Board),
		BatteryStatus:      byte(meta.BatteryStatus),
		TemperatureCelsius: float64(meta.TemperatureCelsius),
		BatteryVoltage:     float64(meta.BatteryVoltage),
	})
	return true
}

// ds4ResetUnsubs tracks lib speaker-reset subscriptions per device handle.
var (
	ds4ResetUnsubs   = map[deviceHandle]func(){}
	ds4ResetUnsubsMu sync.Mutex
)

func clearDS4ResetSub(h deviceHandle) {
	ds4ResetUnsubsMu.Lock()
	defer ds4ResetUnsubsMu.Unlock()
	if unsub, ok := ds4ResetUnsubs[h]; ok {
		unsub()
		delete(ds4ResetUnsubs, h)
	}
}

func trackDS4ResetSub(h deviceHandle, unsub func()) {
	ds4ResetUnsubsMu.Lock()
	defer ds4ResetUnsubsMu.Unlock()
	ds4ResetUnsubs[h] = unsub
}

// SetDS4SpeakerResetCallback sets a callback invoked once per speaker
// streaming generation change (host opened, closed, or re-alternated an
// audio interface). Flush previous-generation PCM on fire. Pass NULL to
// clear.
//
//export SetDS4SpeakerResetCallback
func SetDS4SpeakerResetCallback(handle C.DS4DeviceHandle, cb C.DS4SpeakerResetCallback) bool {
	dh := cgo.Handle(handle)
	dhw, ok := dh.Value().(*deviceHandleWrapper)
	if !ok {
		return false
	}
	ds4device, ok := dhw.device.(*dualshock4.DualShock4)
	if !ok {
		return false
	}
	clearDS4ResetSub(deviceHandle(handle))
	if cb == nil {
		return true
	}
	ch, unsub := ds4device.SubscribeSpeaker()
	trackDS4ResetSub(deviceHandle(handle), unsub)
	go func() {
		for ev := range ch {
			if !ev.Reset {
				continue
			}
			C.viiper_call_ds4_speaker_reset(cb, handle)
		}
	}()
	return true
}
