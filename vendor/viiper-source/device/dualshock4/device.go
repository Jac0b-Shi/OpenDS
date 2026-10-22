package dualshock4

import (
	"context"
	"encoding/binary"
	"encoding/json"
	"errors"
	"fmt"
	"log/slog"
	"sync"
	"sync/atomic"
	"time"

	"github.com/DualSenseClient/VIIPER/device"
	"github.com/DualSenseClient/VIIPER/usb"
	"github.com/DualSenseClient/VIIPER/usbip"
)

type DualShock4 struct {
	inputCh    chan *InputState
	inputState *InputState
	metaState  *MetaState

	outputFunc func(OutputState)
	descriptor usb.Descriptor

	probeSelector       [3]byte
	telemetrySubcommand byte

	usbPacketCounter uint32
	timestampBase    time.Time

	// UAC1 streaming state, guarded by mtx: alt-setting per interface
	// number plus feature-unit mute/volume.
	alts    [4]uint8
	spkMute uint8
	micMute uint8
	spkVol  uint16
	micVol  uint16

	// Speaker subscribers, guarded by mtx. Each subscriber drains its own
	// channel; slow subscribers lose oldest frames first so live bridges
	// never stall the USB path.
	speakerSubs map[*speakerSub]struct{}

	// Microphone frame queue (mono S16LE @16kHz, exact 32B frames),
	// guarded by mtx. Fed by the feeder, drained by EP2 IN; silence on
	// underrun mirrors the DS4Dongle underrun guard.
	micQueue [][]byte

	// Raw input passthrough (see raw.go): exact 64B USB report served
	// verbatim while set; nil means synthesize. Immutable snapshots, so
	// the interrupt path reads without locking.
	rawReport atomic.Pointer[rawSnapshot]

	mtx sync.Mutex
}

// micFrameSize is one 1ms mic frame: 16 samples of mono S16LE, matching
// the real DS4 v2's exact-32B-every-frame delivery.
const micFrameSize = 32

const micQueueDepth = 32

// QueueMicrophonePCM enqueues one feeder mic frame for EP2 IN. Only exact
// 32B frames are accepted, and only while the host has opened the mic
// interface (IF2 alt != 0); frames arriving while closed are dropped,
// mirroring DS4Dongle gating on mic_active. A full queue drops oldest
// first.
func (d *DualShock4) QueueMicrophonePCM(frame []byte) bool {
	if len(frame) != micFrameSize {
		return false
	}
	d.mtx.Lock()
	defer d.mtx.Unlock()
	if d.alts[2] == 0 {
		return false
	}
	cp := append([]byte(nil), frame...)
	d.micQueue = append(d.micQueue, cp)
	for len(d.micQueue) > micQueueDepth {
		d.micQueue = d.micQueue[1:]
	}
	return true
}

// popMicrophoneFrame returns the next queued frame or nil on underrun.
func (d *DualShock4) popMicrophoneFrame() []byte {
	d.mtx.Lock()
	defer d.mtx.Unlock()
	if len(d.micQueue) == 0 {
		return nil
	}
	frame := d.micQueue[0]
	d.micQueue = d.micQueue[1:]
	return frame
}

// SpeakerEvent is one speaker-stream item: PCM audio or a generation
// barrier. PCM slices are exact host-written EP1 OUT payloads (2ch S16LE
// @32kHz, up to 132B), read-only and reused after the next event.
type SpeakerEvent struct {
	PCM   []byte
	Reset bool
}

// speakerSub is one speaker-stream subscription (lib callback or TCP audio
// stream).
type speakerSub struct {
	ch      chan SpeakerEvent
	dropped uint64
}

const speakerSubBuffer = 32

// SubscribeSpeaker registers a speaker-stream subscriber. The channel
// receives PCM frames and reset barriers; call unsubscribe to release.
// Unsubscribe is idempotent; core closes the channel on unsubscribe, so
// range loops terminate.
func (d *DualShock4) SubscribeSpeaker() (<-chan SpeakerEvent, func()) {
	d.mtx.Lock()
	defer d.mtx.Unlock()
	if d.speakerSubs == nil {
		d.speakerSubs = map[*speakerSub]struct{}{}
	}
	sub := &speakerSub{ch: make(chan SpeakerEvent, speakerSubBuffer)}
	d.speakerSubs[sub] = struct{}{}
	var once bool
	return sub.ch, func() {
		d.mtx.Lock()
		defer d.mtx.Unlock()
		if once {
			return
		}
		once = true
		delete(d.speakerSubs, sub)
		close(sub.ch)
	}
}

// fireSpeakerEvent copies the payload (caller scratch) and fans out.
// No subscribers means absorbed without copying.
func (d *DualShock4) fireSpeakerEvent(ev SpeakerEvent) {
	d.mtx.Lock()
	defer d.mtx.Unlock()
	if len(d.speakerSubs) == 0 {
		return
	}
	if ev.PCM != nil {
		ev.PCM = append([]byte(nil), ev.PCM...)
	}
	for sub := range d.speakerSubs {
		select {
		case sub.ch <- ev:
		default:
			// Slow subscriber: drop oldest, keep the freshest.
			select {
			case <-sub.ch:
			default:
			}
			select {
			case sub.ch <- ev:
			default:
				sub.dropped++
			}
		}
	}
}

func New(o *device.CreateOptions) (*DualShock4, error) {
	metaState := &MetaState{
		SerialNumber:       DefaultSerialString,
		Board:              DefaultBoardString,
		BuildTime:          DefaultBuildTime,
		BatteryStatus:      DefaultBatteryStatus,
		TemperatureCelsius: DefaultTemperature,
		BatteryVoltage:     DefaultVoltage,
	}
	if o != nil && o.DeviceSpecific != "" {
		var newMeta MetaState
		err := json.Unmarshal([]byte(o.DeviceSpecific), &newMeta)
		if err != nil {
			return nil, fmt.Errorf("invalid JSON payload: %w", err)
		}
		if newMeta.SerialNumber != "" {
			metaState.SerialNumber = newMeta.SerialNumber
		}
		if newMeta.Board != "" {
			metaState.Board = newMeta.Board
		}
		if !newMeta.BuildTime.IsZero() {
			metaState.BuildTime = newMeta.BuildTime
		}
		if newMeta.BatteryStatus != 0 {
			metaState.BatteryStatus = newMeta.BatteryStatus
		}
		if newMeta.TemperatureCelsius != 0 {
			metaState.TemperatureCelsius = newMeta.TemperatureCelsius
		}
		if newMeta.BatteryVoltage != 0 {
			metaState.BatteryVoltage = newMeta.BatteryVoltage
		}
	}

	d := &DualShock4{
		descriptor: defaultDescriptor,
		metaState:  metaState,
	}
	if o != nil {
		if o.IDVendor != nil {
			d.descriptor.Device.IDVendor = *o.IDVendor
		}
		if o.IDProduct != nil {
			d.descriptor.Device.IDProduct = *o.IDProduct
		}
		if len(d.metaState.SerialNumber) > 0 && len(d.metaState.SerialNumber) <= 16 {
			// Serials are hex; short values zero-pad left so they still
			// decode (Sprintf %016s pads strings with spaces, which would
			// fail hex decode to zeros).
			d.metaState.SerialNumber = serialsNormalize(d.metaState.SerialNumber)
		}
	}

	slog.Info("DS4 device instantiated",
		"vid", d.descriptor.Device.IDVendor,
		"pid", d.descriptor.Device.IDProduct,
		"interfaces", len(d.descriptor.Interfaces))

	d.inputState = NewInputState()
	d.inputCh = make(chan *InputState, 1)
	d.inputCh <- d.inputState
	d.timestampBase = time.Now()

	// Speaker volume powers up at -1dB (0xFF00), matching DS4Dongle's
	// config-seeded GET_CUR (speaker_volume 100); mic powers up at 0dB
	// (zero value, in the advertised -23.25..+24dB range).
	d.spkVol = uacSpkVolumeDefault

	return d, nil
}

func (d *DualShock4) SetMetaState(meta MetaState) {
	d.mtx.Lock()
	defer d.mtx.Unlock()
	d.metaState = &meta
}

// MergeMetaState applies non-zero metadata fields (create-time merge rules:
// empty strings and zero numerics keep current values).
func (d *DualShock4) MergeMetaState(delta MetaState) {
	d.mtx.Lock()
	defer d.mtx.Unlock()
	m := *d.metaState
	if delta.SerialNumber != "" {
		m.SerialNumber = delta.SerialNumber
	}
	if delta.Board != "" {
		m.Board = delta.Board
	}
	if !delta.BuildTime.IsZero() {
		m.BuildTime = delta.BuildTime
	}
	if delta.BatteryStatus != 0 {
		m.BatteryStatus = delta.BatteryStatus
	}
	if delta.TemperatureCelsius != 0 {
		m.TemperatureCelsius = delta.TemperatureCelsius
	}
	if delta.BatteryVoltage != 0 {
		m.BatteryVoltage = delta.BatteryVoltage
	}
	d.metaState = &m
}

func (d *DualShock4) SetOutputCallback(f func(OutputState)) {
	d.outputFunc = f
}

func (d *DualShock4) UpdateInputState(state *InputState) {
	d.mtx.Lock()
	d.inputState = state
	d.mtx.Unlock()
	select {
	case <-d.inputCh:
	default:
	}
	d.inputCh <- state
}

func (d *DualShock4) GetDescriptor() *usb.Descriptor {
	return &d.descriptor
}

func (d *DualShock4) GetDeviceSpecificArgs() map[string]any {
	var res map[string]any
	d.mtx.Lock()
	defer d.mtx.Unlock()

	bytes, err := json.Marshal(d.metaState)
	if err != nil {
		return map[string]any{}
	}
	err = json.Unmarshal(bytes, &res)
	if err != nil {
		return map[string]any{}
	}
	return res
}

func (d *DualShock4) HandleTransfer(ctx context.Context, ep uint32, dir uint32, out []byte) []byte {
	if dir == usbip.DirIn {
		switch ep {
		case 4:
			// Raw passthrough wins when a feeder pipes real reports.
			if raw := d.rawInputReport(); raw != nil {
				return raw
			}
			select {
			case <-ctx.Done():
				if errors.Is(ctx.Err(), context.DeadlineExceeded) {
					d.mtx.Lock()
					is := d.inputState
					ms := *d.metaState
					d.mtx.Unlock()
					return d.buildUSBInputReport(is, &ms)
				}
				return nil
			case is := <-d.inputCh:
				d.mtx.Lock()
				ms := *d.metaState
				d.mtx.Unlock()
				return d.buildUSBInputReport(is, &ms)
			}
		case 2:
			// Microphone IN: queued feeder frames, silence on underrun.
			if frame := d.popMicrophoneFrame(); frame != nil {
				return frame
			}
			return micSilence
		default:
			return nil
		}
	}

	if dir == usbip.DirOut {
		switch ep {
		case 3:
			if len(out) >= 11 && out[0] == ReportIDOutput {
				if d.outputFunc != nil {
					d.outputFunc(parseOutputReport(out))
				}
			}
		case 1:
			// Speaker OUT: fan PCM out to subscribers (lib callback,
			// TCP audio streams). No subscribers means absorbed.
			if len(out) > 0 {
				d.fireSpeakerEvent(SpeakerEvent{PCM: out})
			}
		}
	}

	return nil
}

func (d *DualShock4) HandleControl(bmRequestType, bRequest uint8, wValue, wIndex, wLength uint16, data []byte) ([]byte, bool) {
	reportType := uint8(wValue >> 8)
	reportID := uint8(wValue & 0xFF)

	// Standard SET_INTERFACE selects the audio streaming alt-setting.
	if bmRequestType == usbStdOutIface && bRequest == usbSetInterface {
		d.setAltSetting(uint8(wIndex), uint8(wValue))
		return nil, true
	}

	// UAC1 feature-unit mute/volume (speaker 0x02, mic 0x05).
	if bmRequestType == usbClassOutIface || bmRequestType == usbClassInIface {
		if resp, ok := d.handleAudioControl(bmRequestType, bRequest, wValue, wIndex, data); ok {
			if resp != nil && wLength > 0 && int(wLength) < len(resp) {
				resp = resp[:wLength]
			}
			return resp, true
		}
	}

	switch bmRequestType {
	case hidClassIN:
		switch bRequest {
		case hidGetReport:
			if reportType == reportTypeInput && reportID == ReportIDInput {
				if raw := d.rawInputReport(); raw != nil {
					b := raw
					if wLength > 0 && int(wLength) < len(b) {
						b = b[:wLength]
					}
					return b, true
				}
				d.mtx.Lock()
				is := *d.inputState
				ms := *d.metaState
				d.mtx.Unlock()
				b := d.buildUSBInputReport(&is, &ms)
				if wLength > 0 && int(wLength) < len(b) {
					b = b[:wLength]
				}
				return b, true
			}
			if reportType == reportTypeFeature {
				if fn, ok := featureGetHandlers[reportID]; ok {
					b := fn(d)
					if wLength > 0 && int(wLength) < len(b) {
						b = b[:wLength]
					}
					return b, true
				}
			}
		case hidGetIdle:
			return []byte{0x00}, true
		case hidGetProtocol:
			return []byte{0x01}, true
		}
	case hidClassOUT:
		if bRequest == hidSetReport {
			switch {
			case reportType == reportTypeFeature && reportID == featureIDSubcommand:
				if len(data) >= 2 {
					d.telemetrySubcommand = data[1]
				}
				return nil, true
			case reportType == reportTypeFeature && reportID == featureIDProbe:
				if len(data) >= 4 {
					d.probeSelector[0] = data[1]
					d.probeSelector[1] = data[2]
					d.probeSelector[2] = data[3]
				}
				return nil, true
			case reportType == reportTypeOutput && reportID == ReportIDOutput && len(data) >= 11:
				if d.outputFunc != nil {
					d.outputFunc(parseOutputReport(data))
				}
				return nil, true
			}
		}
	}

	slog.Warn("DS4 control request unhandled",
		"bmRequestType", bmRequestType,
		"bRequest", bRequest,
		"reportType", reportType,
		"reportID", reportID,
		"wIndex", wIndex,
		"wLength", wLength,
		"dataLen", len(data))

	return nil, false
}

// USB audio plumbing constants.
const (
	usbStdOutIface   uint8 = 0x01
	usbSetInterface  uint8 = 0x0B
	usbClassOutIface uint8 = 0x21
	usbClassInIface  uint8 = 0xA1

	uacSetCur uint8 = 0x01
	uacGetCur uint8 = 0x81
	uacGetMin uint8 = 0x82
	uacGetMax uint8 = 0x83
	uacGetRes uint8 = 0x84

	uacEntitySpeakerFU uint8 = 0x02
	uacEntityMicFU     uint8 = 0x05
	uacFUControlMute   uint8 = 0x01
	uacFUControlVolume uint8 = 0x02
)

// uacSpkVolumeDefault is the speaker power-up volume (-1dB in 1/256dB
// units), matching DS4Dongle's config-seeded GET_CUR (speaker_volume 100
// maps to the top of the advertised -73..-1dB range).
const uacSpkVolumeDefault uint16 = 0xFF00

// micSilence is one 1ms idle frame: 16 samples of mono S16LE zeros,
// matching the real DS4 v2's exact-32B-every-frame delivery.
var micSilence = make([]byte, 32)

func (d *DualShock4) setAltSetting(iface, alt uint8) {
	changed := false
	d.mtx.Lock()
	if int(iface) < len(d.alts) {
		changed = d.alts[iface] != alt
		d.alts[iface] = alt
		if changed && iface == 2 {
			// Mic generation change: drop queued feeder frames so a
			// reopen never replays stale PCM. DS4Dongle flushes its
			// SBC/PCM queues and resets the decoder on open for the
			// same reason.
			d.micQueue = nil
		}
	}
	d.mtx.Unlock()
	// Streaming generation change on either audio interface: subscribers
	// flush previous-generation PCM.
	if changed && (iface == 1 || iface == 2) {
		d.fireSpeakerEvent(SpeakerEvent{Reset: true})
	}
}

// handleAudioControl serves UAC1 feature-unit mute/volume for the speaker
// (entity 0x02) and mic (entity 0x05), mirroring DS4Dongle usb.cpp ranges.
// The channel number is ignored: the dongle applies every request to the
// master channel. Anything else (unknown entity or request) is left
// unhandled so HID class traffic on the same bmRequestType bytes still
// falls through.
func (d *DualShock4) handleAudioControl(bm, bRequest uint8, wValue, wIndex uint16, data []byte) ([]byte, bool) {
	entity := uint8(wIndex >> 8)
	if entity != uacEntitySpeakerFU && entity != uacEntityMicFU {
		return nil, false
	}
	cs := uint8(wValue >> 8)
	speaker := entity == uacEntitySpeakerFU

	d.mtx.Lock()
	defer d.mtx.Unlock()
	switch {
	case bm == usbClassOutIface && bRequest == uacSetCur && cs == uacFUControlMute && len(data) >= 1:
		if speaker {
			d.spkMute = data[0]
		} else {
			d.micMute = data[0]
		}
		return nil, true
	case bm == usbClassOutIface && bRequest == uacSetCur && cs == uacFUControlVolume && len(data) >= 2:
		v := binary.LittleEndian.Uint16(data[:2])
		if speaker {
			d.spkVol = v
		} else {
			d.micVol = v
		}
		return nil, true
	case bm == usbClassInIface && cs == uacFUControlMute:
		// The dongle answers every mute GET with the mute byte,
		// regardless of bRequest (no CUR/MIN/MAX/RES switch).
		if speaker {
			return []byte{d.spkMute}, true
		}
		return []byte{d.micMute}, true
	case bm == usbClassInIface && cs == uacFUControlVolume:
		var cur uint16
		if speaker {
			cur = d.spkVol
		} else {
			cur = d.micVol
		}
		switch bRequest {
		case uacGetCur:
			b := make([]byte, 2)
			binary.LittleEndian.PutUint16(b, cur)
			return b, true
		case uacGetMin:
			if speaker {
				return []byte{0x00, 0xB7}, true // -73dB
			}
			return []byte{0xC0, 0xE8}, true // -23.25dB
		case uacGetMax:
			if speaker {
				return []byte{0x00, 0xFF}, true // -1dB
			}
			return []byte{0x00, 0x18}, true // +24dB
		case uacGetRes:
			if speaker {
				return []byte{0x00, 0x01}, true // 1dB
			}
			return []byte{0xC0, 0x00}, true // 0.75dB
		}
	}
	return nil, false
}

// featureGetHandlers maps feature report IDs to their builder functions.
// Builders return the report payload WITHOUT the report ID byte, matching
// DS4Dongle (main.cpp strips the ID and trailing CRC from BT-forwarded
// reports; synthesized reports are built ID-less the same way).
var featureGetHandlers = map[byte]func(*DualShock4) []byte{
	featureIDStatus:        (*DualShock4).featureReportStatus,
	featureIDProbeResponse: (*DualShock4).featureReportProbeResponse,
	featureIDCalibration:   (*DualShock4).featureReportCalibration,
	featureIDSerial:        (*DualShock4).featureReportSerial,
	featureIDTelemetry:     (*DualShock4).featureReportTelemetry,
	featureIDIdentity:      (*DualShock4).featureReportControllerMAC,
	featureIDBoardInfo:     (*DualShock4).featureReportBoardInfo,
}

// parseOutputReport decodes USB output report 0x05 (report ID + 31B
// payload). The feeder surface is rumble/LED/flash; bytes data[2:4] and
// data[11:32] carry headset/speaker/mic volumes on real hardware
// (DS4Dongle bt.cpp ds4_set_volume/ds4_enable_mic) with no feeder channel
// here, so they are accepted but ignored.
func parseOutputReport(data []byte) OutputState {
	return OutputState{
		UpdateFlags: data[1],
		RumbleSmall: data[4],
		RumbleLarge: data[5],
		LedRed:      data[6],
		LedGreen:    data[7],
		LedBlue:     data[8],
		FlashOn:     data[9],
		FlashOff:    data[10],
	}
}

func (d *DualShock4) featureReportTelemetry() []byte {
	d.mtx.Lock()
	defer d.mtx.Unlock()

	s := serialStringToBytes(d.metaState.SerialNumber)
	switch d.telemetrySubcommand {
	case 0x02:
		return []byte{
			s[3], s[2], s[1], s[0], s[7], s[6], s[5], s[4],
			0x00, 0x00, 0x00, 0x00, 0x00,
		}
	case 0x0B:
		return []byte{
			s[3], s[2], s[1], s[0], s[7], s[6], s[5], s[4],
			0xAC, 0xA8, 0x1B,
			0x00, 0x00,
		}
	default:
		volts := telemetryVoltageU16(d.metaState.BatteryVoltage)
		temp := telemetryTemperatureU16(d.metaState.TemperatureCelsius)
		return []byte{
			d.telemetrySubcommand, 0x03, 0x01, 0x00, 0x04,
			byte(volts), byte(volts >> 8),
			byte(temp), byte(temp >> 8),
			0x00, 0x00, 0x00, 0x00,
		}
	}
}

// featureReportControllerMAC serves USB GET 0x81: the 6-byte controller
// MAC, LSB first (DS4Dongle main.cpp synthesizes it from the BT layer;
// the virtual device derives the bytes from the serial).
func (d *DualShock4) featureReportControllerMAC() []byte {
	d.mtx.Lock()
	serial := serialStringToBytes(d.metaState.SerialNumber)
	d.mtx.Unlock()

	return []byte{serial[7], serial[6], serial[5], serial[4], serial[3], serial[2]}
}

func (d *DualShock4) featureReportBoardInfo() []byte {
	report := make([]byte, 48)

	d.mtx.Lock()
	buildDateStr := d.metaState.BuildTime.Format("Jan 02 2006")
	buildTimeStr := d.metaState.BuildTime.Format("15:04:05")
	d.mtx.Unlock()

	copy(report[0:15], buildDateStr)
	copy(report[15:31], buildTimeStr)
	binary.LittleEndian.PutUint16(report[32:34], HardwareVersionMajor)
	binary.LittleEndian.PutUint16(report[34:36], HardwareVersionMinor)
	binary.LittleEndian.PutUint32(report[36:40], SoftwareVersionMajor)
	binary.LittleEndian.PutUint16(report[40:42], SoftwareVersionMinor)

	report[46] = 1

	return report
}

func (d *DualShock4) featureReportSerial() []byte {
	d.mtx.Lock()
	serial := serialStringToBytes(d.metaState.SerialNumber)
	d.mtx.Unlock()

	report := make([]byte, 15)
	report[0] = serial[7]
	report[1] = serial[6]
	report[2] = serial[5]
	report[3] = serial[4]
	report[4] = serial[3]
	report[5] = serial[2]
	report[6] = serial[1]
	copy(report[7:15], serial[:])

	return report
}

func (d *DualShock4) featureReportStatus() []byte {
	d.mtx.Lock()
	defer d.mtx.Unlock()
	report := make([]byte, 4)
	report[0] = d.metaState.BatteryStatus & BatteryLevelMask
	report[1] = 12
	binary.LittleEndian.PutUint16(report[2:4], 664)
	return report
}

func (d *DualShock4) featureReportProbeResponse() []byte {
	b1 := d.probeSelector[0]
	b2 := d.probeSelector[1]

	report := [2]byte{b1, b2}

	if d.probeSelector[0] == 0xFF && d.probeSelector[1] == 0x00 && d.probeSelector[2] == 0x0C {
		report[0] = 0x01
	}

	return report[:]
}

func (d *DualShock4) featureReportCalibration() []byte {
	report := make([]byte, 36)

	// 17 LE int16 fields packed sequentially: bias(pitch,yaw,roll) |
	// gyro±(x,y,z) interleaved per axis (USB order; DS4Dongle reorders
	// the BT all-plus-then-all-minus words into this layout) |
	// speed(x,y) | accel±(x,y,z).
	for i, v := range [17]int16{
		0, 0, 0,
		1024, -1024, 1024, -1024, 1024, -1024,
		64, 64,
		8192, -8192, 8192, -8192, 8192, -8192,
	} {
		binary.LittleEndian.PutUint16(report[i*2:], uint16(v))
	}

	return report
}

func (d *DualShock4) buildUSBInputReport(s *InputState, m *MetaState) []byte {
	b := make([]byte, InputReportSize)

	b[0] = ReportIDInput

	b[1] = uint8(int16(s.LX) + 128)
	b[2] = uint8(int16(s.LY) + 128)
	b[3] = uint8(int16(s.RX) + 128)
	b[4] = uint8(int16(s.RY) + 128)

	usbDPad := uint8(DPadUSBNeutral)
	switch {
	case s.DPad&DPadUp != 0 && s.DPad&DPadRight != 0:
		usbDPad = DPadUSBUpRight
	case s.DPad&DPadUp != 0 && s.DPad&DPadLeft != 0:
		usbDPad = DPadUSBUpLeft
	case s.DPad&DPadDown != 0 && s.DPad&DPadRight != 0:
		usbDPad = DPadUSBDownRight
	case s.DPad&DPadDown != 0 && s.DPad&DPadLeft != 0:
		usbDPad = DPadUSBDownLeft
	case s.DPad&DPadUp != 0:
		usbDPad = DPadUSBUp
	case s.DPad&DPadDown != 0:
		usbDPad = DPadUSBDown
	case s.DPad&DPadLeft != 0:
		usbDPad = DPadUSBLeft
	case s.DPad&DPadRight != 0:
		usbDPad = DPadUSBRight
	}

	b[5] = (usbDPad & DPadMask) | (uint8(s.Buttons) & 0xF0)
	b[6] = uint8(s.Buttons >> 8)

	counterRaw := atomic.AddUint32(&d.usbPacketCounter, 1)
	counter := counterRaw & 0x3F

	psTouch := uint8(0)
	if s.Buttons&ButtonPS != 0 {
		psTouch |= ButtonPSUSB
	}
	if s.Buttons&ButtonTouchpadClick != 0 {
		psTouch |= ButtonTouchpadClickUSB
	}
	b[7] = psTouch | uint8(counter<<CounterShift)

	b[8] = s.L2
	b[9] = s.R2

	ts := d.nextReportTimestamp()
	binary.LittleEndian.PutUint16(b[10:12], uint16(ts))

	binary.LittleEndian.PutUint16(b[13:15], uint16(s.GyroX))
	binary.LittleEndian.PutUint16(b[15:17], uint16(s.GyroY))
	binary.LittleEndian.PutUint16(b[17:19], uint16(s.GyroZ))

	binary.LittleEndian.PutUint16(b[19:21], uint16(s.AccelX))
	binary.LittleEndian.PutUint16(b[21:23], uint16(s.AccelY))
	binary.LittleEndian.PutUint16(b[23:25], uint16(s.AccelZ))

	b[12] = 0x09            // status: touchpad connected, no extension
	b[30] = m.BatteryStatus // low nibble = level, bit4 = cable
	b[33] = 0x01            // nvslocked
	b[34] = 0x01

	// Touch packet counters increment per report while active; inactive
	// stays 0x80. The increment follows community DS4 captures, not
	// DS4Dongle: that firmware forwards BT bytes verbatim and its neutral
	// init (main.cpp:41) leaves these bytes zero, so nothing in-repo
	// describes the counter semantics. Derived from the report counter so
	// no extra state is needed.
	touchCnt := uint8(counterRaw & 0x7F)
	touch1Counter := touchCnt
	if !s.Touch1Active {
		touch1Counter = TouchInactiveMask
	}
	b[35] = touch1Counter
	encodeTouchCoords(b[36:39], s.Touch1X, s.Touch1Y)

	touch2Counter := touchCnt
	if !s.Touch2Active {
		touch2Counter = TouchInactiveMask
	}
	b[39] = touch2Counter
	encodeTouchCoords(b[40:43], s.Touch2X, s.Touch2Y)

	return b
}

func (d *DualShock4) nextReportTimestamp() uint32 {
	return uint32(time.Since(d.timestampBase).Nanoseconds() * 3 / 16000)
}
