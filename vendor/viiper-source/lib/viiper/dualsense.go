package main

/*
#include <stdint.h>
#include <stdlib.h>

typedef uintptr_t USBServerHandle;

typedef uintptr_t DSDeviceHandle;

#define DS_BUTTON_SQUARE    0x00000010u
#define DS_BUTTON_CROSS     0x00000020u
#define DS_BUTTON_CIRCLE    0x00000040u
#define DS_BUTTON_TRIANGLE  0x00000080u
#define DS_BUTTON_L1        0x00000100u
#define DS_BUTTON_R1        0x00000200u
#define DS_BUTTON_L2        0x00000400u
#define DS_BUTTON_R2        0x00000800u
#define DS_BUTTON_CREATE    0x00001000u
#define DS_BUTTON_OPTIONS   0x00002000u
#define DS_BUTTON_L3        0x00004000u
#define DS_BUTTON_R3        0x00008000u
#define DS_BUTTON_PS        0x00010000u
#define DS_BUTTON_TOUCHPAD  0x00020000u
#define DS_BUTTON_MIC_MUTE  0x00040000u
#define DS_BUTTON_RFN       0x00200000u
#define DS_BUTTON_LFN       0x00100000u
#define DS_BUTTON_R4        0x00800000u
#define DS_BUTTON_L4        0x00400000u

#define DS_DPAD_UP     0x01u
#define DS_DPAD_DOWN   0x02u
#define DS_DPAD_LEFT   0x04u
#define DS_DPAD_RIGHT  0x08u

// Exact size of a USB input report 0x01 (report ID + 63B payload).
#define DS_RAW_REPORT_SIZE 64

#define DS_SHELL_COLOR_WHITE                    "00"
#define DS_SHELL_COLOR_BLACK                    "01"
#define DS_SHELL_COLOR_COSMIC_RED               "02"
#define DS_SHELL_COLOR_NOVA_PINK                "03"
#define DS_SHELL_COLOR_GALACTIC_PURPLE          "04"
#define DS_SHELL_COLOR_STARLIGHT_BLUE           "05"
#define DS_SHELL_COLOR_GREY_CAMOUFLAGE          "06"
#define DS_SHELL_COLOR_VOLCANIC_RED             "07"
#define DS_SHELL_COLOR_STERLING_SILVER          "08"
#define DS_SHELL_COLOR_COBALT_BLUE              "09"
#define DS_SHELL_COLOR_CHROMA_TEAL              "10"
#define DS_SHELL_COLOR_CHROMA_INDIGO            "11"
#define DS_SHELL_COLOR_CHROMA_PEARL             "12"
#define DS_SHELL_COLOR_ANNIVERSARY_30TH         "30"
#define DS_SHELL_COLOR_GOD_OF_WAR_RAGNAROK      "Z1"
#define DS_SHELL_COLOR_SPIDER_MAN_2             "Z2"
#define DS_SHELL_COLOR_ASTRO_BOT                "Z3"
#define DS_SHELL_COLOR_FORTNITE                 "Z4"
#define DS_SHELL_COLOR_MONSTER_HUNTER_WILDS     "Z5"
#define DS_SHELL_COLOR_THE_LAST_OF_US           "Z6"
#define DS_SHELL_COLOR_GHOST_OF_YOTEI           "Z7"
#define DS_SHELL_COLOR_ICON_BLUE_LIMITED_EDITION "ZB"
#define DS_SHELL_COLOR_ASTRO_BOT_JOYFUL_EDITION "ZC"
#define DS_SHELL_COLOR_GENSHIN_IMPACT           "ZE"

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
	uint8_t  Touch1Tracking;
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

typedef struct {
	const char* SerialNumber;       // NULL = use default
	const char* MACAddress;         // NULL = use default
	const char* Board;              // NULL = use default
	uint8_t     BatteryStatus;      // 0 = use default
	double      TemperatureCelsius; // 0 = use default
	double      BatteryVoltage;     // 0 = use default
	const char* ShellColor;     // NULL = use default (2-char code, e.g. "00", "Z1")
} DSMetaState;

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

static void viiper_call_ds_output(DSOutputCallback fn, DSDeviceHandle handle, const DSOutputState* output) {
	fn(handle, output);
}

typedef void (*DSAudioCallback)(DSDeviceHandle handle, const uint8_t* pcm, size_t length);

static void viiper_call_ds_audio(DSAudioCallback fn, DSDeviceHandle handle, const uint8_t* pcm, size_t length) {
	fn(handle, pcm, length);
}

typedef void (*DSSpeakerResetCallback)(DSDeviceHandle handle);

static void viiper_call_ds_reset(DSSpeakerResetCallback fn, DSDeviceHandle handle) {
	fn(handle);
}

typedef void (*DSRealtimeHapticsCallback)(DSDeviceHandle handle, const uint8_t* pcm, size_t length);

static void viiper_call_ds_haptics(DSRealtimeHapticsCallback fn, DSDeviceHandle handle, const uint8_t* pcm, size_t length) {
	fn(handle, pcm, length);
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
	"github.com/DualSenseClient/VIIPER/device/dualsense"
	"github.com/DualSenseClient/VIIPER/internal/server/api"
)

// CreateDualSenseDevice creates a new DualSense (non-edge) device on the bus with the given ID on the server associated with the given handle.
// @param serverHandle Handle to the USB server.
// @param outDeviceHandle Output parameter for the created device handle.
// @param busID ID of the bus to add the device to.
// @param autoAttachLocalhost If true, the device will be automatically attached to a USBIP-Client/Driver running on THIS machine.
// @param idVendor Optional USB vendor ID (0 = default).
// @param idProduct Optional USB product ID (0 = default).
// @param meta Optional pointer to initial device metadata. Pass NULL to use defaults.
//
//export CreateDualSenseDevice
func CreateDualSenseDevice(
	serverHandle C.USBServerHandle,
	outDeviceHandle *C.DSDeviceHandle,
	busID uint32,
	autoAttachLocalhost bool,
	idVendor uint16,
	idProduct uint16,
	meta *C.DSMetaState,
) bool {
	return createDualSenseDevice(serverHandle, outDeviceHandle, busID, autoAttachLocalhost, idVendor, idProduct, meta, false)
}

// CreateDualSenseEdgeDevice creates a new DualSense Edge device on the bus with the given ID on the server associated with the given handle.
// @param serverHandle Handle to the USB server.
// @param outDeviceHandle Output parameter for the created device handle.
// @param busID ID of the bus to add the device to.
// @param autoAttachLocalhost If true, the device will be automatically attached to a USBIP-Client/Driver running on THIS machine.
// @param idVendor Optional USB vendor ID (0 = default).
// @param idProduct Optional USB product ID (0 = default).
// @param meta Optional pointer to initial device metadata. Pass NULL to use defaults.
//
//export CreateDualSenseEdgeDevice
func CreateDualSenseEdgeDevice(
	serverHandle C.USBServerHandle,
	outDeviceHandle *C.DSDeviceHandle,
	busID uint32,
	autoAttachLocalhost bool,
	idVendor uint16,
	idProduct uint16,
	meta *C.DSMetaState,
) bool {
	return createDualSenseDevice(serverHandle, outDeviceHandle, busID, autoAttachLocalhost, idVendor, idProduct, meta, true)
}

func createDualSenseDevice(
	serverHandle C.USBServerHandle,
	outDeviceHandle *C.DSDeviceHandle,
	busID uint32,
	autoAttachLocalhost bool,
	idVendor uint16,
	idProduct uint16,
	meta *C.DSMetaState,
	edge bool,
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
		goMeta := dualsense.MetaState{
			SerialNumber:       goStringOrEmpty(meta.SerialNumber),
			MACAddress:         goStringOrEmpty(meta.MACAddress),
			Board:              goStringOrEmpty(meta.Board),
			BatteryStatus:      uint8(meta.BatteryStatus),
			TemperatureCelsius: float64(meta.TemperatureCelsius),
			BatteryVoltage:     float64(meta.BatteryVoltage),
			ShellColor:         goStringOrEmpty(meta.ShellColor),
		}
		b, err := json.Marshal(goMeta)
		if err != nil {
			return false
		}
		opts.DeviceSpecific = string(b)
	}

	ctor := dualsense.New
	if edge {
		ctor = dualsense.NewEdge
	}
	d, err := ctor(opts)
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
	*outDeviceHandle = C.DSDeviceHandle(cgo.NewHandle(handleWrapper))

	shw.mtx.Lock()
	defer shw.mtx.Unlock()
	shw.deviceHandles[busID] = append(shw.deviceHandles[busID], deviceHandle(*outDeviceHandle))
	return true
}

// SetDualSenseDeviceState updates the input state of the DualSense device associated with the given handle.
// @param handle Handle to the DualSense device.
// @param state New input state to set on the device.
//
//export SetDualSenseDeviceState
func SetDualSenseDeviceState(handle C.DSDeviceHandle, state C.DSDeviceState) bool {
	dh := cgo.Handle(handle)
	dhw, ok := dh.Value().(*deviceHandleWrapper)
	if !ok {
		return false
	}
	dsDevice, ok := dhw.device.(*dualsense.DualSense)
	if !ok {
		return false
	}
	s := &dualsense.InputState{
		LX:             int8(state.LX),
		LY:             int8(state.LY),
		RX:             int8(state.RX),
		RY:             int8(state.RY),
		Buttons:        uint32(state.Buttons),
		DPad:           uint8(state.DPad),
		L2:             uint8(state.L2),
		R2:             uint8(state.R2),
		Touch1X:        uint16(state.Touch1X),
		Touch1Y:        uint16(state.Touch1Y),
		Touch1Active:   state.Touch1Active != 0,
		Touch1Tracking: uint8(state.Touch1Tracking),
		Touch2X:        uint16(state.Touch2X),
		Touch2Y:        uint16(state.Touch2Y),
		Touch2Active:   state.Touch2Active != 0,
		Touch2Tracking: uint8(state.Touch2Tracking),
		GyroX:          int16(state.GyroX),
		GyroY:          int16(state.GyroY),
		GyroZ:          int16(state.GyroZ),
		AccelX:         int16(state.AccelX),
		AccelY:         int16(state.AccelY),
		AccelZ:         int16(state.AccelZ),
	}
	dsDevice.UpdateInputState(s)
	return true
}

// SetDualSenseOutputCallback sets a callback to be invoked when the host sends output (rumble/triggers/LED) commands to the device.
// @param handle Handle to the DualSense device.
// @param callback Callback receiving the full output state. Pass NULL to clear.
//
//export SetDualSenseOutputCallback
func SetDualSenseOutputCallback(handle C.DSDeviceHandle, cb C.DSOutputCallback) bool {
	dh := cgo.Handle(handle)
	dhw, ok := dh.Value().(*deviceHandleWrapper)
	if !ok {
		return false
	}
	dsDevice, ok := dhw.device.(*dualsense.DualSense)
	if !ok {
		return false
	}
	if cb == nil {
		dsDevice.SetOutputCallback(nil)
		return true
	}
	dsDevice.SetOutputCallback(func(out dualsense.OutputState) {
		var cOut C.DSOutputState
		cOut.Flags0 = C.uint8_t(out.Flags0)
		cOut.Flags1 = C.uint8_t(out.Flags1)
		cOut.RumbleSmall = C.uint8_t(out.RumbleSmall)
		cOut.RumbleLarge = C.uint8_t(out.RumbleLarge)
		cOut.VolumeHeadphones = C.uint8_t(out.VolumeHeadphones)
		cOut.VolumeSpeaker = C.uint8_t(out.VolumeSpeaker)
		cOut.VolumeMic = C.uint8_t(out.VolumeMic)
		cOut.AudioControl = C.uint8_t(out.AudioControl)
		cOut.MuteLightMode = C.uint8_t(out.MuteLightMode)
		cOut.MuteControl = C.uint8_t(out.MuteControl)
		for i := 0; i < 11; i++ {
			cOut.TriggerRight[i] = C.uint8_t(out.TriggerRight[i])
			cOut.TriggerLeft[i] = C.uint8_t(out.TriggerLeft[i])
		}
		cOut.HostTimestamp = C.uint32_t(out.HostTimestamp)
		cOut.MotorPower = C.uint8_t(out.MotorPower)
		cOut.AudioControl2 = C.uint8_t(out.AudioControl2)
		cOut.Flags3 = C.uint8_t(out.Flags3)
		cOut.HapticFilter = C.uint8_t(out.HapticFilter)
		cOut.UnkByte = C.uint8_t(out.UnkByte)
		cOut.LightFade = C.uint8_t(out.LightFade)
		cOut.LightBrightness = C.uint8_t(out.LightBrightness)
		cOut.PlayerLeds = C.uint8_t(out.PlayerLeds)
		cOut.LedRed = C.uint8_t(out.LedRed)
		cOut.LedGreen = C.uint8_t(out.LedGreen)
		cOut.LedBlue = C.uint8_t(out.LedBlue)
		C.viiper_call_ds_output(cb, handle, &cOut)
	})
	return true
}

// dsAudioUnsubs tracks lib speaker-stream subscriptions per device handle
// so re-registering replaces the old pumps and removal stops them.
var (
	dsAudioUnsubs   = map[deviceHandle]func(){}
	dsAudioUnsubsMu sync.Mutex
)

func clearDSAudioSub(h deviceHandle) {
	dsAudioUnsubsMu.Lock()
	defer dsAudioUnsubsMu.Unlock()
	if unsub, ok := dsAudioUnsubs[h]; ok {
		unsub()
		delete(dsAudioUnsubs, h)
	}
}

func trackDSAudioSub(h deviceHandle, unsub func()) {
	dsAudioUnsubsMu.Lock()
	defer dsAudioUnsubsMu.Unlock()
	dsAudioUnsubs[h] = unsub
}

// SetDualSenseAudioOutCallback sets a callback invoked with the exact bytes
// the host writes to the speaker endpoint (4ch S16LE @48kHz, up to 392B).
// The buffer is only valid during the call; copy it. Invoked on the audio
// thread: never block. Pass NULL to clear.
//
//export SetDualSenseAudioOutCallback
func SetDualSenseAudioOutCallback(handle C.DSDeviceHandle, cb C.DSAudioCallback) bool {
	dh := cgo.Handle(handle)
	dhw, ok := dh.Value().(*deviceHandleWrapper)
	if !ok {
		return false
	}
	dsDevice, ok := dhw.device.(*dualsense.DualSense)
	if !ok {
		return false
	}
	clearDSAudioSub(deviceHandle(handle))
	if cb == nil {
		return true
	}
	ch, unsub := dsDevice.SubscribeSpeaker()
	trackDSAudioSub(deviceHandle(handle), unsub)
	go func() {
		for ev := range ch {
			if ev.Reset || len(ev.PCM) == 0 {
				continue
			}
			pcm := ev.PCM
			C.viiper_call_ds_audio(cb, handle,
				(*C.uint8_t)(unsafe.Pointer(&pcm[0])),
				C.size_t(len(pcm)),
			)
		}
	}()
	return true
}

// SetDualSenseSpeakerResetCallback sets a callback invoked once per speaker
// streaming generation change (host opened, closed, or re-alternated the
// audio interface). Flush previous-generation PCM on fire. Pass NULL to
// clear.
//
//export SetDualSenseSpeakerResetCallback
func SetDualSenseSpeakerResetCallback(handle C.DSDeviceHandle, cb C.DSSpeakerResetCallback) bool {
	dh := cgo.Handle(handle)
	dhw, ok := dh.Value().(*deviceHandleWrapper)
	if !ok {
		return false
	}
	dsDevice, ok := dhw.device.(*dualsense.DualSense)
	if !ok {
		return false
	}
	clearDSResetSub(deviceHandle(handle))
	if cb == nil {
		return true
	}
	ch, unsub := dsDevice.SubscribeSpeaker()
	trackDSResetSub(deviceHandle(handle), unsub)
	go func() {
		for ev := range ch {
			if !ev.Reset {
				continue
			}
			C.viiper_call_ds_reset(cb, handle)
		}
	}()
	return true
}

// dsResetUnsubs tracks lib reset-callback subscriptions per device handle.
var (
	dsResetUnsubs   = map[deviceHandle]func(){}
	dsResetUnsubsMu sync.Mutex
)

func clearDSResetSub(h deviceHandle) {
	dsResetUnsubsMu.Lock()
	defer dsResetUnsubsMu.Unlock()
	if unsub, ok := dsResetUnsubs[h]; ok {
		unsub()
		delete(dsResetUnsubs, h)
	}
}

func trackDSResetSub(h deviceHandle, unsub func()) {
	dsResetUnsubsMu.Lock()
	defer dsResetUnsubsMu.Unlock()
	dsResetUnsubs[h] = unsub
}

// SetDualSenseRealtimeHapticsCallback sets a callback invoked with the
// rear voice-coil pair (2ch S16LE @48kHz) for minimal-latency haptics
// forwarding; resampling to the 3kHz haptics rate is feeder-side. Same
// buffer/thread rules as the speaker callback. Pass NULL to clear.
//
//export SetDualSenseRealtimeHapticsCallback
func SetDualSenseRealtimeHapticsCallback(handle C.DSDeviceHandle, cb C.DSRealtimeHapticsCallback) bool {
	dh := cgo.Handle(handle)
	dhw, ok := dh.Value().(*deviceHandleWrapper)
	if !ok {
		return false
	}
	dsDevice, ok := dhw.device.(*dualsense.DualSense)
	if !ok {
		return false
	}
	clearDSHapticsSub(deviceHandle(handle))
	if cb == nil {
		return true
	}
	ch, unsub := dsDevice.SubscribeHaptics()
	trackDSHapticsSub(deviceHandle(handle), unsub)
	go func() {
		for ev := range ch {
			if ev.Reset || len(ev.PCM) == 0 {
				continue
			}
			pcm := ev.PCM
			C.viiper_call_ds_haptics(cb, handle,
				(*C.uint8_t)(unsafe.Pointer(&pcm[0])),
				C.size_t(len(pcm)),
			)
		}
	}()
	return true
}

// dsHapticsUnsubs tracks lib haptics-callback subscriptions per handle.
var (
	dsHapticsUnsubs   = map[deviceHandle]func(){}
	dsHapticsUnsubsMu sync.Mutex
)

func clearDSHapticsSub(h deviceHandle) {
	dsHapticsUnsubsMu.Lock()
	defer dsHapticsUnsubsMu.Unlock()
	if unsub, ok := dsHapticsUnsubs[h]; ok {
		unsub()
		delete(dsHapticsUnsubs, h)
	}
}

func trackDSHapticsSub(h deviceHandle, unsub func()) {
	dsHapticsUnsubsMu.Lock()
	defer dsHapticsUnsubsMu.Unlock()
	dsHapticsUnsubs[h] = unsub
}

// SetDualSenseMetaState merge-updates identity/battery metadata at runtime.
// Empty strings and zero numerics keep current values (same rules as
// creation). The USB serial string refreshes when the serial changes.
//
//export SetDualSenseMetaState
func SetDualSenseMetaState(handle C.DSDeviceHandle, meta *C.DSMetaState) bool {
	dh := cgo.Handle(handle)
	dhw, ok := dh.Value().(*deviceHandleWrapper)
	if !ok {
		return false
	}
	dsDevice, ok := dhw.device.(*dualsense.DualSense)
	if !ok {
		return false
	}
	if meta == nil {
		return false
	}
	dsDevice.MergeMetaState(dualsense.MetaState{
		SerialNumber:       goStringOrEmpty(meta.SerialNumber),
		MACAddress:         goStringOrEmpty(meta.MACAddress),
		Board:              goStringOrEmpty(meta.Board),
		BatteryStatus:      uint8(meta.BatteryStatus),
		TemperatureCelsius: float64(meta.TemperatureCelsius),
		BatteryVoltage:     float64(meta.BatteryVoltage),
		ShellColor:         goStringOrEmpty(meta.ShellColor),
	})
	return true
}

// SetDualSenseMicrophonePCM queues one feeder mic frame (exactly 192B:
// 48 frames of 2ch S16LE @48kHz) for EP2 IN. Returns false for any other
// length, or while the host has not opened the mic interface (frames
// arriving while closed are dropped, as on DS5Dongle). Full queues drop
// oldest first; underruns serve silence.
//
//export SetDualSenseMicrophonePCM
func SetDualSenseMicrophonePCM(handle C.DSDeviceHandle, data *C.uint8_t, length C.size_t) bool {
	dh := cgo.Handle(handle)
	dhw, ok := dh.Value().(*deviceHandleWrapper)
	if !ok {
		return false
	}
	dsDevice, ok := dhw.device.(*dualsense.DualSense)
	if !ok {
		return false
	}
	if data == nil || length != 192 {
		return false
	}
	frame := C.GoBytes(unsafe.Pointer(data), C.int(length))
	return dsDevice.QueueMicrophonePCM(frame)
}

//export SetDualSenseMicrophoneBufferMs
func SetDualSenseMicrophoneBufferMs(handle C.DSDeviceHandle, ms C.uint32_t) bool {
	wrapper, ok := cgo.Handle(handle).Value().(*deviceHandleWrapper)
	if !ok {
		return false
	}
	ds, ok := wrapper.device.(*dualsense.DualSense)
	return ok && ds.SetMicrophoneBufferMs(uint32(ms))
}

//export GetDualSenseMicrophoneBufferStats
func GetDualSenseMicrophoneBufferStats(handle C.DSDeviceHandle, data *C.uint64_t, length C.size_t) bool {
	if data == nil || length != 4 {
		return false
	}
	wrapper, ok := cgo.Handle(handle).Value().(*deviceHandleWrapper)
	if !ok {
		return false
	}
	ds, ok := wrapper.device.(*dualsense.DualSense)
	if !ok {
		return false
	}
	stats := ds.MicrophoneBufferStats()
	copy(unsafe.Slice(data, 4), []C.uint64_t{C.uint64_t(stats[0]), C.uint64_t(stats[1]), C.uint64_t(stats[2]), C.uint64_t(stats[3])})
	return true
}

// SetDualSenseRawInputReport pipes one exact 64B USB input report 0x01
// (real controller bytes). While set, interrupt IN and GET_REPORT serve it
// verbatim; ClearDualSenseRawInputReport restores synthetic reports.
// Returns false for any other length or wrong report ID.
//
//export SetDualSenseRawInputReport
func SetDualSenseRawInputReport(handle C.DSDeviceHandle, data *C.uint8_t, length C.size_t) bool {
	dh := cgo.Handle(handle)
	dhw, ok := dh.Value().(*deviceHandleWrapper)
	if !ok {
		return false
	}
	dsDevice, ok := dhw.device.(*dualsense.DualSense)
	if !ok {
		return false
	}
	if data == nil || length != C.DS_RAW_REPORT_SIZE {
		return false
	}
	raw := C.GoBytes(unsafe.Pointer(data), C.int(length))
	return dsDevice.SetRawInputReport(raw)
}

// ClearDualSenseRawInputReport drops the raw passthrough and restores
// synthetic input reports.
//
//export ClearDualSenseRawInputReport
func ClearDualSenseRawInputReport(handle C.DSDeviceHandle) bool {
	dh := cgo.Handle(handle)
	dhw, ok := dh.Value().(*deviceHandleWrapper)
	if !ok {
		return false
	}
	dsDevice, ok := dhw.device.(*dualsense.DualSense)
	if !ok {
		return false
	}
	dsDevice.ClearRawInputReport()
	return true
}

// UseDualSensePhysicalUSBProfile selects the standard wired descriptor profile.
// Call before USB attachment. Edge is unchanged and returns false.
//
//export UseDualSensePhysicalUSBProfile
func UseDualSensePhysicalUSBProfile(handle C.DSDeviceHandle) bool {
	dhw, ok := cgo.Handle(handle).Value().(*deviceHandleWrapper)
	if !ok {
		return false
	}
	ds, ok := dhw.device.(*dualsense.DualSense)
	return ok && ds.UsePhysicalUSBProfile()
}

// SetDualSenseFeatureReport stores a verbatim GET_REPORT response for one
// feature ID (report ID at data[0], exact descriptor length), e.g. Edge
// profile snapshots 0x70-0x7B pushed from a physical controller.
// Returns false for unknown IDs, Edge-only IDs on standard devices, or
// wrong lengths.
//
//export SetDualSenseFeatureReport
func SetDualSenseFeatureReport(handle C.DSDeviceHandle, data *C.uint8_t, length C.size_t) bool {
	dh := cgo.Handle(handle)
	dhw, ok := dh.Value().(*deviceHandleWrapper)
	if !ok {
		return false
	}
	dsDevice, ok := dhw.device.(*dualsense.DualSense)
	if !ok {
		return false
	}
	if data == nil || length == 0 {
		return false
	}
	report := C.GoBytes(unsafe.Pointer(data), C.int(length))
	return dsDevice.SetFeatureReport(report)
}

// ClearDualSenseFeatureReport drops a feeder feature override, restoring
// the built-in handler or zero stub.
//
//export ClearDualSenseFeatureReport
func ClearDualSenseFeatureReport(handle C.DSDeviceHandle, reportID C.uint8_t) bool {
	dh := cgo.Handle(handle)
	dhw, ok := dh.Value().(*deviceHandleWrapper)
	if !ok {
		return false
	}
	dsDevice, ok := dhw.device.(*dualsense.DualSense)
	if !ok {
		return false
	}
	return dsDevice.ClearFeatureReport(byte(reportID))
}

// RemoveDualSenseDevice removes the DualSense device associated with the given handle from the server.
// @param handle Handle to the DualSense device to remove.
//
//export RemoveDualSenseDevice
func RemoveDualSenseDevice(handle C.DSDeviceHandle) bool {
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
	clearDSAudioSub(deviceHandle(handle))
	clearDSResetSub(deviceHandle(handle))
	clearDSHapticsSub(deviceHandle(handle))
	dh.Delete()

	return true
}
