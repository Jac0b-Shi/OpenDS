package dualsense

import (
	"context"
	"encoding/binary"
	"encoding/json"
	"errors"
	"fmt"
	"log/slog"
	"math"
	"net"
	"sync"
	"sync/atomic"
	"time"

	"github.com/DualSenseClient/VIIPER/device"
	"github.com/DualSenseClient/VIIPER/usb"
	"github.com/DualSenseClient/VIIPER/usbip"
)

type DualSense struct {
	inputCh    chan *InputState
	inputState *InputState
	metaState  *MetaState

	outputFunc func(OutputState)
	descriptor usb.Descriptor

	subcommand [2]byte

	// Edge handshake store for feature 0x65 (payload without report ID).
	// Served back on GET; unset reads echo the 0x20 firmware body.
	edgeHandshake    [63]byte
	hasEdgeHandshake bool

	seqCounter    uint8
	timestampBase time.Time
	edge          bool

	// Raw input passthrough (see raw.go): exact 64B USB report served
	// verbatim while set; nil means synthesize. Immutable snapshots, so
	// the interrupt path reads without locking.
	rawReport atomic.Pointer[rawSnapshot]
	rawReady  chan struct{}
	paceRaw   bool // opted in before attachment; one completion per published input

	// Synthesized mute LED state echoed in input report b[54] bit 2,
	// derived from the last host output (AllowMuteLight-gated modes
	// 0-2); modes 3-7 leave it unchanged. Guarded by mtx.
	muteLED bool

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

	// Haptics subscribers, guarded by mtx: same discipline, smaller
	// buffer. They receive the rear voice-coil pair only (see below).
	hapticsSubs map[*hapticsSub]struct{}

	// Mic-only jitter buffer, guarded by mtx. Allocated on first ingest,
	// flushed and released when the host changes the mic alt setting.
	micBuffer                *microphoneBuffer
	micBufferMs              uint32
	micUnderruns, micDropped uint64

	// Feeder-driven feature-report overrides (Edge profile proxy, see
	// feature_override.go), guarded by mtx.
	featureOverrides map[byte][]byte

	mtx sync.Mutex
}

// micFrameSize is one 1ms mic frame: 48 samples of 2ch S16LE.
const micFrameSize = 192

// QueueMicrophonePCM enqueues one feeder mic frame for EP2 IN. Only exact
// 192B frames are accepted, and only while the host has opened the mic
// interface (IF2 alt != 0); frames arriving while closed are dropped,
// mirroring DS5Dongle mic_add_queue gating on mic_active. A full buffer
// drops oldest first; EP2 drains it after a mic-only 128ms pre-roll.
func (d *DualSense) QueueMicrophonePCM(frame []byte) bool {
	if len(frame) != micFrameSize {
		return false
	}
	d.mtx.Lock()
	defer d.mtx.Unlock()
	if d.alts[2] == 0 {
		return false
	}
	if d.micBuffer == nil {
		d.micBuffer = &microphoneBuffer{target: int(d.micBufferMs) * micSamplesPerFrame}
	}
	d.micBuffer.push(frame)
	return true
}

// popMicrophoneFrame returns the next queued frame or nil on underrun.
func (d *DualSense) popMicrophoneFrame() []byte {
	d.mtx.Lock()
	defer d.mtx.Unlock()
	if d.micBuffer == nil {
		return nil
	}
	return d.micBuffer.pop()
}

// SpeakerEvent is one speaker-stream item: PCM audio or a generation
// barrier. PCM slices are exact host-written EP1 OUT payloads (4ch S16LE
// @48kHz, up to 392B), read-only and reused after the next event.
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
func (d *DualSense) SubscribeSpeaker() (<-chan SpeakerEvent, func()) {
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
func (d *DualSense) fireSpeakerEvent(ev SpeakerEvent) {
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

// HapticsEvent is one rear-haptics item: the deinterleaved rear voice-coil
// pair (2ch S16LE @48kHz) for minimal-latency forwarding, or a generation
// barrier. Mirrors the DS5Dongle rear-channel derivation (audio.cpp);
// resampling to the 3kHz haptics rate is feeder-side.
type HapticsEvent struct {
	PCM   []byte
	Reset bool
}

type hapticsSub struct {
	ch      chan HapticsEvent
	dropped uint64
}

const hapticsSubBuffer = 8

// SubscribeHaptics registers a rear-haptics subscriber. Same release
// discipline as SubscribeSpeaker (channel closed on unsubscribe).
func (d *DualSense) SubscribeHaptics() (<-chan HapticsEvent, func()) {
	d.mtx.Lock()
	defer d.mtx.Unlock()
	if d.hapticsSubs == nil {
		d.hapticsSubs = map[*hapticsSub]struct{}{}
	}
	sub := &hapticsSub{ch: make(chan HapticsEvent, hapticsSubBuffer)}
	d.hapticsSubs[sub] = struct{}{}
	var once bool
	return sub.ch, func() {
		d.mtx.Lock()
		defer d.mtx.Unlock()
		if once {
			return
		}
		once = true
		delete(d.hapticsSubs, sub)
		close(sub.ch)
	}
}

// fireHapticsEvent fans out to haptics subscribers (payload already owned).
func (d *DualSense) fireHapticsEvent(ev HapticsEvent) {
	d.mtx.Lock()
	defer d.mtx.Unlock()
	for sub := range d.hapticsSubs {
		select {
		case sub.ch <- ev:
		default:
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

// hasHapticsSubs reports whether any haptics subscriber is registered.
func (d *DualSense) hasHapticsSubs() bool {
	d.mtx.Lock()
	defer d.mtx.Unlock()
	return len(d.hapticsSubs) > 0
}

// splitRearPair deinterleaves the rear voice-coil pair (channels 2,3) from
// one 4ch S16LE frame block. Returns nil when len is not whole frames.
func splitRearPair(frame []byte) []byte {
	const block = 4 * 2 // 4 channels of S16LE
	if len(frame) == 0 || len(frame)%block != 0 {
		return nil
	}
	frames := len(frame) / block
	out := make([]byte, frames*2*2)
	for i := 0; i < frames; i++ {
		copy(out[i*4:i*4+4], frame[i*8+4:i*8+8])
	}
	return out
}

func New(o *device.CreateOptions) (*DualSense, error) {
	return new(o, false)
}
func NewEdge(o *device.CreateOptions) (*DualSense, error) {
	return new(o, true)
}

func new(o *device.CreateOptions, edge bool) (*DualSense, error) {
	metaState := &MetaState{
		SerialNumber:       DefaultSerialNumberDS,
		MACAddress:         DefaultMACAddressDS,
		Board:              DefaultBoardStringDS,
		BuildTime:          DefaultBuildTime,
		BatteryStatus:      DefaultBatteryStatus,
		TemperatureCelsius: DefaultTemperature,
		BatteryVoltage:     DefaultVoltage,
		ShellColor:         DefaultShellColor,
	}
	if edge {
		metaState.SerialNumber = DefaultSerialNumberDSEdge
		metaState.MACAddress = DefaultMACAddressDSEdge
		metaState.Board = DefaultBoardStringEdge
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
		if newMeta.MACAddress != "" {
			metaState.MACAddress = newMeta.MACAddress
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
		metaState.ShellColor = newMeta.ShellColor
	}

	devDesc := baseDeviceDescriptor()
	devDesc.IDProduct = DefaultPIDDS
	product := "DualSense Wireless Controller"
	if edge {
		devDesc.IDProduct = DefaultPIDDSEdge
		product = "DualSense Edge Wireless Controller"
	}
	ifaces := hidInterfaceFor(edge)

	if o != nil {
		if o.IDVendor != nil {
			devDesc.IDVendor = *o.IDVendor
		}
		if o.IDProduct != nil {
			devDesc.IDProduct = *o.IDProduct
		}
	}

	// Strings are per-device: the map must be cloned so one device never
	// mutates the shared template, and index 3 carries this unit's serial.
	strings := map[uint8]string{
		0: "\u0409", // LangID: en-US (0x0409)
		1: "Sony Interactive Entertainment",
		2: product,
		3: metaState.SerialNumber,
	}

	d := &DualSense{
		descriptor: usb.Descriptor{
			Device:        devDesc,
			Configuration: baseConfiguration(),
			Interfaces:    ifaces,
			Strings:       strings,
		},
		metaState: metaState,
		edge:      edge,
	}

	slog.Info("DualSense device instantiated",
		"edge", edge,
		"vid", d.descriptor.Device.IDVendor,
		"pid", d.descriptor.Device.IDProduct,
		"interfaces", d.descriptor.NumInterfaces())

	d.inputState = NewInputState()
	d.inputCh = make(chan *InputState, 1)
	d.rawReady = make(chan struct{}, 1)
	d.inputCh <- d.inputState
	d.timestampBase = time.Now()

	// Mic volume powers up at +48dB (0x3000), matching DS5Dongle
	// volume[1] init (usb.cpp); speaker powers up at 0dB (zero value,
	// matching the dongle default speaker_volume of 100).
	d.micVol = uacMicVolumeDefault

	return d, nil
}

func (d *DualSense) SetMetaState(meta MetaState) {
	d.mtx.Lock()
	defer d.mtx.Unlock()
	d.metaState = &meta
	if d.descriptor.Strings != nil {
		d.descriptor.Strings[3] = meta.SerialNumber
	}
}

// MergeMetaState applies non-zero metadata fields (create-time merge rules:
// empty strings and zero numerics keep current values) and refreshes the
// USB serial string.
func (d *DualSense) MergeMetaState(delta MetaState) {
	d.mtx.Lock()
	defer d.mtx.Unlock()
	m := *d.metaState
	if delta.SerialNumber != "" {
		m.SerialNumber = delta.SerialNumber
	}
	if delta.MACAddress != "" {
		m.MACAddress = delta.MACAddress
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
	if delta.ShellColor != "" {
		m.ShellColor = delta.ShellColor
	}
	d.metaState = &m
	if d.descriptor.Strings != nil {
		d.descriptor.Strings[3] = m.SerialNumber
	}
}

func (d *DualSense) SetOutputCallback(f func(OutputState)) {
	d.outputFunc = f
}

func (d *DualSense) UpdateInputState(state *InputState) {
	d.mtx.Lock()
	d.inputState = state
	d.mtx.Unlock()
	select {
	case <-d.inputCh:
	default:
	}
	d.inputCh <- state
}

func (d *DualSense) GetDescriptor() *usb.Descriptor {
	return &d.descriptor
}

func (d *DualSense) GetDeviceSpecificArgs() map[string]any {
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

func (d *DualSense) HandleTransfer(ctx context.Context, ep uint32, dir uint32, out []byte) []byte {
	if dir == usbip.DirIn {
		switch ep {
		case 4:
			// Raw passthrough wins when a feeder pipes real reports.
			if raw := d.rawInputReport(); raw != nil {
				if d.paceRaw {
					select {
					case <-ctx.Done():
						return nil
					case <-d.rawReady:
						return d.rawInputReport()
					}
				}
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
			if len(out) >= 48 && out[0] == ReportIDOutput {
				fb := parseOutputReport(out)
				d.noteOutputMuteLight(fb)
				if d.outputFunc != nil {
					d.outputFunc(fb)
				}
			}
		case 1:
			// Speaker OUT: fan PCM out to subscribers (lib callback,
			// TCP audio streams) and the rear pair to haptics
			// subscribers. No subscribers means absorbed.
			if len(out) > 0 {
				d.fireSpeakerEvent(SpeakerEvent{PCM: out})
				if d.hasHapticsSubs() {
					if rear := splitRearPair(out); rear != nil {
						d.fireHapticsEvent(HapticsEvent{PCM: rear})
					}
				}
			}
		}
	}

	return nil
}

func (d *DualSense) HandleControl(bmRequestType, bRequest uint8, wValue, wIndex, wLength uint16, data []byte) ([]byte, bool) {
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
				if b := d.getFeatureReport(reportID); b != nil {
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
			case reportType == reportTypeFeature && reportID == featureIDCommand && len(data) >= 3:
				d.subcommand[0] = data[1]
				d.subcommand[1] = data[2]
				return nil, true
			case reportType == reportTypeFeature && reportID == featureIDEdgeHandshake && len(data) >= 64:
				// Edge profile handshake: verbatim echo of the 0x20
				// firmware body (DS5Dongle dse.cpp). data[0] is the
				// report ID; the 63 payload bytes are stored.
				if d.edge {
					d.mtx.Lock()
					copy(d.edgeHandshake[:], data[1:64])
					d.hasEdgeHandshake = true
					d.mtx.Unlock()
				}
				return nil, true
			case reportType == reportTypeFeature:
				return nil, true
			case reportType == reportTypeOutput && reportID == ReportIDOutput && len(data) >= 48:
				fb := parseOutputReport(data)
				d.noteOutputMuteLight(fb)
				if d.outputFunc != nil {
					d.outputFunc(fb)
				}
				return nil, true
			}
		}
	}

	slog.Warn("DualSense control request unhandled",
		"bmRequestType", bmRequestType,
		"bRequest", bRequest,
		"reportType", reportType,
		"reportID", reportID,
		"wIndex", wIndex,
		"wLength", wLength,
		"dataLen", len(data))

	return nil, false
}

var featureGetHandlers = map[byte]func(*DualSense) []byte{
	featureIDCalibration:     (*DualSense).featureReportCalibration,
	featureIDPairing:         (*DualSense).featureReportPairing,
	featureIDFirmware:        (*DualSense).featureReportFirmware,
	featureIDCommandResponse: (*DualSense).featureReportCommandResponse,
	featureIDEdgeHandshake:   (*DualSense).featureReportEdgeHandshake,
}

// featureReportEdgeHandshake serves GET 0x65: the stored handshake payload,
// defaulting to an echo of the 0x20 firmware body. Edge-only (absent from
// the DS descriptor); nil on DS so the host stalls as before.
func (d *DualSense) featureReportEdgeHandshake() []byte {
	if !d.edge {
		return nil
	}
	report := make([]byte, 64)
	report[0] = featureIDEdgeHandshake
	d.mtx.Lock()
	defer d.mtx.Unlock()
	if d.hasEdgeHandshake {
		copy(report[1:], d.edgeHandshake[:])
		return report
	}
	fw := d.featureReportFirmwareLocked()
	copy(report[1:], fw[1:])
	return report
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

// uacMicVolumeDefault is the mic power-up volume (+48dB in 1/256dB units),
// matching DS5Dongle volume[1] init (usb.cpp).
const uacMicVolumeDefault uint16 = 0x3000

// micSilence is one 1ms idle frame: 48 samples of 2ch S16LE zeros.
var micSilence = make([]byte, 192)

func (d *DualSense) setAltSetting(iface, alt uint8) {
	changed := false
	d.mtx.Lock()
	if int(iface) < len(d.alts) {
		changed = d.alts[iface] != alt
		d.alts[iface] = alt
		if changed && iface == 2 {
			// Mic generation change: drop queued feeder frames so a
			// reopen never replays stale PCM. DS5Dongle gates ingest
			// while closed but leaves queued Opus in place (up to
			// ~80ms stale on reopen); the virtual device flushes
			// instead.
			d.flushMicrophoneBuffer()
		}
	}
	d.mtx.Unlock()
	// Streaming generation change on either audio interface: subscribers
	// flush previous-generation PCM.
	if changed && (iface == 1 || iface == 2) {
		d.fireSpeakerEvent(SpeakerEvent{Reset: true})
		d.fireHapticsEvent(HapticsEvent{Reset: true})
	}
}

// handleAudioControl serves UAC1 feature-unit mute/volume for the speaker
// (entity 0x02) and mic (entity 0x05), mirroring DS5Dongle usb.cpp ranges.
// The channel number is ignored: the dongle applies every request to the
// master channel. Anything else (unknown entity or request) is left
// unhandled so HID class traffic on the same bmRequestType bytes still
// falls through.
func (d *DualSense) handleAudioControl(bm, bRequest uint8, wValue, wIndex uint16, data []byte) ([]byte, bool) {
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
				return []byte{0x00, 0x9C}, true
			}
			return []byte{0x00, 0x00}, true
		case uacGetMax:
			if speaker {
				return []byte{0x00, 0x00}, true
			}
			return []byte{0x00, 0x30}, true
		case uacGetRes:
			if speaker {
				return []byte{0x00, 0x01}, true
			}
			return []byte{0x7A, 0x00}, true
		}
	}
	return nil, false
}

// dsFeatureLengths maps every DS feature report ID to its total GET length
// (report ID + payload), derived from the HID report descriptor counts.
var dsFeatureLengths = map[byte]int{
	0x05: 41, 0x08: 48, 0x09: 20, 0x0A: 27, 0x0B: 42, 0x0C: 42,
	0x20: 64, 0x21: 5, 0x22: 64,
	0x80: 64, 0x81: 64, 0x82: 10, 0x83: 64, 0x84: 64, 0x85: 3,
	0xA0: 2, 0xE0: 64,
	0xF0: 64, 0xF1: 64, 0xF2: 16, 0xF4: 64, 0xF5: 4,
	0xF6: 64, 0xF7: 64, 0xF8: 64, 0xF9: 64,
}

// edgeFeatureLengths overrides/additions for the Edge descriptor:
// 0xF2 is wider and the profile block is Edge-only.
var edgeFeatureLengths = map[byte]int{
	0xF2: 53,
	0x60: 64, 0x61: 64, 0x62: 64, 0x63: 64, 0x64: 64, 0x65: 64,
	0x68: 64,
	0x70: 64, 0x71: 64, 0x72: 64, 0x73: 64, 0x74: 64, 0x75: 64,
	0x76: 64, 0x77: 64, 0x78: 64, 0x79: 64, 0x7A: 64, 0x7B: 64,
}

// getFeatureReport serves a GET_REPORT for any descriptor-listed feature ID.
// Feeder overrides (Edge profile proxy) win; IDs with real content use
// their handler; the rest return a zero stub of exact length with the
// report ID set. Edge profile reports (0x60-0x7B) are static stubs until
// a feeder pushes physical snapshots.
func (d *DualSense) getFeatureReport(id byte) []byte {
	if b := d.featureOverride(id); b != nil {
		return b
	}
	if fn, ok := featureGetHandlers[id]; ok {
		return fn(d)
	}
	n, ok := dsFeatureLengths[id]
	if d.edge {
		if en, found := edgeFeatureLengths[id]; found {
			n, ok = en, true
		}
	}
	if !ok {
		return nil
	}
	b := make([]byte, n)
	b[0] = id
	return b
}

// parseOutputReport decodes the USB output report 0x02 (report ID + 47B
// payload on DS, ID + 63B on Edge) into feeder semantics. Edge appends
// 16 reserved bytes after the LED (DS5Dongle SetStateData reserved[16]);
// they carry no feeder state and are ignored.
func parseOutputReport(out []byte) OutputState {
	var feedback OutputState
	if len(out) < 48 {
		return feedback
	}
	feedback.Flags0 = out[1]
	feedback.Flags1 = out[2]
	feedback.RumbleSmall = out[3]
	feedback.RumbleLarge = out[4]
	feedback.VolumeHeadphones = out[5]
	feedback.VolumeSpeaker = out[6]
	feedback.VolumeMic = out[7]
	feedback.AudioControl = out[8]
	feedback.MuteLightMode = out[9]
	feedback.MuteControl = out[10]
	copy(feedback.TriggerRight[:], out[11:22])
	copy(feedback.TriggerLeft[:], out[22:33])
	feedback.HostTimestamp = binary.LittleEndian.Uint32(out[33:37])
	feedback.MotorPower = out[37]
	feedback.AudioControl2 = out[38]
	feedback.Flags3 = out[39]
	feedback.HapticFilter = out[40]
	feedback.UnkByte = out[41]
	feedback.LightFade = out[42]
	feedback.LightBrightness = out[43]
	feedback.PlayerLeds = out[44]
	feedback.LedRed = out[45]
	feedback.LedGreen = out[46]
	feedback.LedBlue = out[47]
	return feedback
}

func (d *DualSense) featureReportCalibration() []byte {
	report := make([]byte, 41)
	report[0] = featureIDCalibration

	for i, v := range [17]int16{
		0, 0, 0,
		8192, -8192, 8192, -8192, 8192, -8192,
		500, 500,
		8192, -8192, 8192, -8192, 8192, -8192,
	} {
		binary.LittleEndian.PutUint16(report[1+i*2:], uint16(v))
	}

	report[35] = 0x0B // TODO:
	return report
}

func (d *DualSense) featureReportPairing() []byte {
	report := make([]byte, 20)
	report[0] = featureIDPairing

	d.mtx.Lock()
	mac := d.metaState.MACAddress
	d.mtx.Unlock()

	if hw, err := net.ParseMAC(mac); err == nil && len(hw) == 6 {
		for i := range 6 {
			report[1+i] = hw[5-i]
		}
	}

	// TODO:
	report[7] = 0x08
	report[8] = 0x25
	report[10] = 0x1E
	report[12] = 0xEE
	report[13] = 0x74
	report[14] = 0xD0
	report[15] = 0xBC
	return report
}

func (d *DualSense) featureReportFirmware() []byte {
	d.mtx.Lock()
	defer d.mtx.Unlock()
	return d.featureReportFirmwareLocked()
}

// featureReportFirmwareLocked builds the 0x20 report; caller holds mtx.
func (d *DualSense) featureReportFirmwareLocked() []byte {
	report := make([]byte, 64)
	report[0] = featureIDFirmware

	bt := d.metaState.BuildTime

	copy(report[1:12], bt.Format("Jan 02 2006"))
	copy(report[12:20], bt.Format("15:04:05"))

	report[20] = HardwareType
	report[21] = 0x01 // TODO: unknown
	report[22] = 0x44 // TODO: put in CONST!!! // build revision from real device

	binary.LittleEndian.PutUint32(report[24:28], HwInfo)

	// TODO: unknown
	report[28] = 0x36
	report[31] = 0x01
	report[32] = 0xC1
	report[33] = 0xC8

	binary.LittleEndian.PutUint16(report[44:46], FirmwareVersion)

	// TODO: unknown
	report[48] = 0x14
	report[52] = 0x0B
	report[54] = 0x01
	report[56] = 0x06
	return report
}

func (d *DualSense) featureReportCommandResponse() []byte {
	report := make([]byte, 64)
	report[0] = featureIDCommandResponse

	d.mtx.Lock()
	sub := d.subcommand
	serial := d.metaState.SerialNumber
	voltage := d.metaState.BatteryVoltage
	temp := d.metaState.TemperatureCelsius
	d.mtx.Unlock()

	switch sub[0] {
	case subcmdSerial:
		copy(report[3:21], serial)
	case subcmdStatus:
		// nvs locked
		report[1] = 0x01
		report[4] = 0x01
	case subcmdSensors:
		vRaw := uint16(math.Round(voltage * 1000))
		report[4] = byte(vRaw)
		report[5] = byte(vRaw >> 8)
		tRaw := uint16(math.Max(0, math.Min(4095, math.Round((2470.0-temp*26.0)/0.78125))))
		report[6] = byte(tRaw)
		report[7] = byte(tRaw >> 8)
	default:
		slog.Warn("DualSense: unknown sub-command for featureIDCommandResponse",
			"sub0", sub[0], "sub1", sub[1])
		report[1] = 0x01
	}
	return report
}

// noteOutputMuteLight folds a host output report into the synthesized mute
// LED state echoed in input report b[54] bit 2. Only AllowMuteLight-gated
// Off/On/Breathing modes update it; DoNothing/NoAction leave the previous
// state (the real controller ignores those values too).
func (d *DualSense) noteOutputMuteLight(fb OutputState) {
	if fb.Flags1&Flag1AllowMuteLight == 0 {
		return
	}
	switch fb.MuteLightMode {
	case MuteLightOff:
		d.mtx.Lock()
		d.muteLED = false
		d.mtx.Unlock()
	case MuteLightOn, MuteLightBreathing:
		d.mtx.Lock()
		d.muteLED = true
		d.mtx.Unlock()
	}
}

// buildUSBInputReport encodes the 64B USB input report 0x01.
//
// Populated from feeder state: sticks, triggers, seq, buttons, gyro/accel,
// sensor timestamp, touch contacts, temperature, battery status.
//
// Deliberately left zero (need a real-hardware capture to fill correctly,
// not sample constants): b[11:16] touch timestamps, b[41:48] trigger
// effect state echo, b[49] reserved marker (kept at the historical 0x10),
// b[50:52], b[54] except the mute-light bit (bit 0 headset-plugged and
// bits 1,3-7 stay zero: controller-side knowledge with no feeder
// channel), b[55:63] audio/headset flags and reserved tail.
func (d *DualSense) buildUSBInputReport(s *InputState, m *MetaState) []byte {
	b := make([]byte, InputReportSize)

	b[0] = ReportIDInput

	b[1] = uint8(int16(s.LX) + 128)
	b[2] = uint8(int16(s.LY) + 128)
	b[3] = uint8(int16(s.RX) + 128)
	b[4] = uint8(int16(s.RY) + 128)

	b[5] = s.L2
	b[6] = s.R2

	d.seqCounter++
	b[7] = d.seqCounter

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
	b[8] = (usbDPad & DPadMask) | (uint8(s.Buttons) & 0xF0)
	b[9] = uint8(s.Buttons >> 8)
	b[10] = uint8(s.Buttons >> 16)

	binary.LittleEndian.PutUint16(b[16:18], uint16(s.GyroX))
	binary.LittleEndian.PutUint16(b[18:20], uint16(s.GyroY))
	binary.LittleEndian.PutUint16(b[20:22], uint16(s.GyroZ))

	binary.LittleEndian.PutUint16(b[22:24], uint16(s.AccelX))
	binary.LittleEndian.PutUint16(b[24:26], uint16(s.AccelY))
	binary.LittleEndian.PutUint16(b[26:28], uint16(s.AccelZ))

	ts := uint32(time.Since(d.timestampBase).Microseconds() * 3)
	binary.LittleEndian.PutUint32(b[28:32], ts)

	// b[32] is the int8 temperature byte: the controller populates it (the
	// DS5Dongle neutral report at main.cpp:48 carries payload[31] = 0xfc,
	// i.e. -4C), and hid-playstation reads the same offset as Celsius.
	b[32] = byte(int8(min(max(math.Round(m.TemperatureCelsius), -128), 127)))

	// Contact bytes carry the feeder-supplied tracking IDs verbatim (real
	// controller IDs preserved end-to-end); feeders that do not track
	// send zero.
	touch1 := s.Touch1Tracking & 0x7F
	if !s.Touch1Active {
		touch1 |= TouchInactiveMask
	}
	b[33] = touch1
	encodeTouchCoords(b[34:37], s.Touch1X, s.Touch1Y)

	touch2 := s.Touch2Tracking & 0x7F
	if !s.Touch2Active {
		touch2 |= TouchInactiveMask
	}
	b[37] = touch2
	encodeTouchCoords(b[38:41], s.Touch2X, s.Touch2Y)

	b[49] = 0x10
	b[53] = m.BatteryStatus

	// b[54] bit 2 echoes the host-driven mute LED (synthesized from the
	// last output report; the dongle forwards the controller bit).
	d.mtx.Lock()
	led := d.muteLED
	d.mtx.Unlock()
	if led {
		b[54] |= 0x04
	}

	return b
}
