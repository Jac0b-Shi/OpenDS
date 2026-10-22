package dualsense

import (
	"encoding/binary"
	"encoding/json"
	"io"
	"log/slog"
	"time"
)

// nolint
// viiper:wire dualsense c2s stickLX:i8 stickLY:i8 stickRX:i8 stickRY:i8 buttons:u32 dpad:u8 triggerL2:u8 triggerR2:u8 touch1X:u16 touch1Y:u16 touch1Active:bool touch1Tracking:u8 touch2X:u16 touch2Y:u16 touch2Active:bool touch2Tracking:u8 gyroX:i16 gyroY:i16 gyroZ:i16 accelX:i16 accelY:i16 accelZ:i16
type InputState struct {
	LX, LY  int8
	RX, RY  int8
	Buttons uint32
	DPad    uint8
	L2, R2  uint8

	Touch1X, Touch1Y uint16
	Touch1Active     bool
	Touch1Tracking   uint8
	Touch2X, Touch2Y uint16
	Touch2Active     bool
	Touch2Tracking   uint8

	GyroX, GyroY, GyroZ    int16
	AccelX, AccelY, AccelZ int16
}

// NewInputState returns a DualSense input state in its neutral/resting state.
func NewInputState() *InputState {
	x, y, z := DefaultAccelRaw()
	return &InputState{
		AccelX: x,
		AccelY: y,
		AccelZ: z,
	}
}

func (s *InputState) MarshalBinary() ([]byte, error) {
	b := make([]byte, InputStateSize)
	b[0] = uint8(s.LX)
	b[1] = uint8(s.LY)
	b[2] = uint8(s.RX)
	b[3] = uint8(s.RY)
	binary.LittleEndian.PutUint32(b[4:8], s.Buttons)
	b[8] = s.DPad
	b[9] = s.L2
	b[10] = s.R2
	binary.LittleEndian.PutUint16(b[11:13], s.Touch1X)
	binary.LittleEndian.PutUint16(b[13:15], s.Touch1Y)
	if s.Touch1Active {
		b[15] = 1
	}
	b[16] = s.Touch1Tracking
	binary.LittleEndian.PutUint16(b[17:19], s.Touch2X)
	binary.LittleEndian.PutUint16(b[19:21], s.Touch2Y)
	if s.Touch2Active {
		b[21] = 1
	}
	b[22] = s.Touch2Tracking
	binary.LittleEndian.PutUint16(b[23:25], uint16(s.GyroX))
	binary.LittleEndian.PutUint16(b[25:27], uint16(s.GyroY))
	binary.LittleEndian.PutUint16(b[27:29], uint16(s.GyroZ))
	binary.LittleEndian.PutUint16(b[29:31], uint16(s.AccelX))
	binary.LittleEndian.PutUint16(b[31:33], uint16(s.AccelY))
	binary.LittleEndian.PutUint16(b[33:35], uint16(s.AccelZ))
	return b, nil
}

func (s *InputState) UnmarshalBinary(data []byte) error {
	if len(data) < InputStateSize {
		return io.ErrUnexpectedEOF
	}
	s.LX = int8(data[0])
	s.LY = int8(data[1])
	s.RX = int8(data[2])
	s.RY = int8(data[3])
	s.Buttons = binary.LittleEndian.Uint32(data[4:8])
	s.DPad = data[8]
	s.L2 = data[9]
	s.R2 = data[10]
	s.Touch1X = binary.LittleEndian.Uint16(data[11:13])
	s.Touch1Y = binary.LittleEndian.Uint16(data[13:15])
	s.Touch1Active = data[15] != 0
	s.Touch1Tracking = data[16]
	s.Touch2X = binary.LittleEndian.Uint16(data[17:19])
	s.Touch2Y = binary.LittleEndian.Uint16(data[19:21])
	s.Touch2Active = data[21] != 0
	s.Touch2Tracking = data[22]
	s.GyroX = int16(binary.LittleEndian.Uint16(data[23:25]))
	s.GyroY = int16(binary.LittleEndian.Uint16(data[25:27]))
	s.GyroZ = int16(binary.LittleEndian.Uint16(data[27:29]))
	s.AccelX = int16(binary.LittleEndian.Uint16(data[29:31]))
	s.AccelY = int16(binary.LittleEndian.Uint16(data[31:33]))
	s.AccelZ = int16(binary.LittleEndian.Uint16(data[33:35]))
	return nil
}

// nolint
// viiper:wire dualsense s2c flags0:u8 flags1:u8 rumbleSmall:u8 rumbleLarge:u8 volumeHeadphones:u8 volumeSpeaker:u8 volumeMic:u8 audioControl:u8 muteLightMode:u8 muteControl:u8 triggerRight:u8*11 triggerLeft:u8*11 hostTimestamp:u32 motorPower:u8 audioControl2:u8 flags3:u8 hapticFilter:u8 unkByte:u8 lightFade:u8 lightBrightness:u8 playerLeds:u8 ledRed:u8 ledGreen:u8 ledBlue:u8
type OutputState struct {
	Flags0           uint8
	Flags1           uint8
	RumbleSmall      uint8
	RumbleLarge      uint8
	VolumeHeadphones uint8
	VolumeSpeaker    uint8
	VolumeMic        uint8
	AudioControl     uint8
	MuteLightMode    uint8
	MuteControl      uint8
	TriggerRight     [11]byte
	TriggerLeft      [11]byte
	HostTimestamp    uint32
	MotorPower       uint8
	AudioControl2    uint8
	Flags3           uint8
	HapticFilter     uint8
	UnkByte          uint8
	LightFade        uint8
	LightBrightness  uint8
	PlayerLeds       uint8
	LedRed           uint8
	LedGreen         uint8
	LedBlue          uint8
}

// Output flag bits (see DS5Dongle SetStateData).
const (
	Flag1AllowMuteLight      uint8 = 0x01
	Flag0UseRumbleNotHaptics uint8 = 0x02
	Flag0AllowRightTrigger   uint8 = 0x04
	Flag0AllowLeftTrigger    uint8 = 0x08
	Flag1AllowLedColor       uint8 = 0x04
	Flag1AllowPlayerLEDs     uint8 = 0x10
)

// Mute-light modes (see DS5Dongle MuteLight enum in utils.h).
const (
	MuteLightOff       uint8 = 0
	MuteLightOn        uint8 = 1
	MuteLightBreathing uint8 = 2
	MuteLightDoNothing uint8 = 3
)

// TriggerEffect is the decoded 11-byte adaptive-trigger block at
// OutputState.TriggerRight/TriggerLeft: block[0] is the effect mode,
// block[1:11] are mode parameters. DS5Dongle carries these blocks opaquely
// (SetStateData RightTriggerFFB/LeftTriggerFFB); mode semantics follow the
// community reverse-engineered 0x02 layout, so only offsets are decoded
// here — the feeder maps modes as before.
type TriggerEffect struct {
	Mode   uint8
	Params [10]byte
}

// RightTriggerEffect decodes the R2 adaptive-trigger block.
func (f *OutputState) RightTriggerEffect() TriggerEffect {
	var p [10]byte
	copy(p[:], f.TriggerRight[1:11])
	return TriggerEffect{Mode: f.TriggerRight[0], Params: p}
}

// LeftTriggerEffect decodes the L2 adaptive-trigger block.
func (f *OutputState) LeftTriggerEffect() TriggerEffect {
	var p [10]byte
	copy(p[:], f.TriggerLeft[1:11])
	return TriggerEffect{Mode: f.TriggerLeft[0], Params: p}
}

// MicLED reports the mute-light mode (MuteLightMode byte).
func (f *OutputState) MicLED() uint8 {
	return f.MuteLightMode
}

// LightbarCustom reports whether the host takes over the lightbar
// (Flags3 AllowColorLightFadeAnimation bit).
func (f *OutputState) LightbarCustom() bool {
	return f.Flags3&0x02 != 0
}

func (f *OutputState) MarshalBinary() ([]byte, error) {
	b := make([]byte, OutputStateSize)
	b[0] = f.Flags0
	b[1] = f.Flags1
	b[2] = f.RumbleSmall
	b[3] = f.RumbleLarge
	b[4] = f.VolumeHeadphones
	b[5] = f.VolumeSpeaker
	b[6] = f.VolumeMic
	b[7] = f.AudioControl
	b[8] = f.MuteLightMode
	b[9] = f.MuteControl
	copy(b[10:21], f.TriggerRight[:])
	copy(b[21:32], f.TriggerLeft[:])
	binary.LittleEndian.PutUint32(b[32:36], f.HostTimestamp)
	b[36] = f.MotorPower
	b[37] = f.AudioControl2
	b[38] = f.Flags3
	b[39] = f.HapticFilter
	b[40] = f.UnkByte
	b[41] = f.LightFade
	b[42] = f.LightBrightness
	b[43] = f.PlayerLeds
	b[44] = f.LedRed
	b[45] = f.LedGreen
	b[46] = f.LedBlue
	return b, nil
}

func (f *OutputState) UnmarshalBinary(data []byte) error {
	if len(data) < OutputStateSize {
		return io.ErrUnexpectedEOF
	}
	f.Flags0 = data[0]
	f.Flags1 = data[1]
	f.RumbleSmall = data[2]
	f.RumbleLarge = data[3]
	f.VolumeHeadphones = data[4]
	f.VolumeSpeaker = data[5]
	f.VolumeMic = data[6]
	f.AudioControl = data[7]
	f.MuteLightMode = data[8]
	f.MuteControl = data[9]
	copy(f.TriggerRight[:], data[10:21])
	copy(f.TriggerLeft[:], data[21:32])
	f.HostTimestamp = binary.LittleEndian.Uint32(data[32:36])
	f.MotorPower = data[36]
	f.AudioControl2 = data[37]
	f.Flags3 = data[38]
	f.HapticFilter = data[39]
	f.UnkByte = data[40]
	f.LightFade = data[41]
	f.LightBrightness = data[42]
	f.PlayerLeds = data[43]
	f.LedRed = data[44]
	f.LedGreen = data[45]
	f.LedBlue = data[46]
	return nil
}

type MetaState struct {
	SerialNumber string    `json:"serial_number"`
	MACAddress   string    `json:"mac_address"` // "XX:XX:XX:XX:XX:XX"
	Board        string    `json:"board"`
	BuildTime    time.Time `json:"build_time"`

	BatteryStatus      uint8   `json:"battery_status"`
	TemperatureCelsius float64 `json:"temperature_celsius"`
	BatteryVoltage     float64 `json:"battery_voltage"`

	ShellColor string `json:"shell_color"` // hardware variant / controller color code, e.g. "00", "Z1"
}

func (m *MetaState) ToMap() map[string]any {
	bytes, err := json.Marshal(m)
	if err != nil {
		slog.Error("marshal meta state for map", "error", err)
		return map[string]any{}
	}
	var res map[string]any
	err = json.Unmarshal(bytes, &res)
	if err != nil {
		slog.Error("unmarshal meta state for map", "error", err)
		return map[string]any{}
	}
	return res
}

func (m *MetaState) UpdateFromMap(data map[string]any) {
	bytes, err := json.Marshal(data)
	if err != nil {
		slog.Error("marshal meta state for update", "error", err)
		return
	}
	var newMeta MetaState
	err = json.Unmarshal(bytes, &newMeta)
	if err != nil {
		slog.Error("unmarshal meta state for update", "error", err)
		return
	}
	if newMeta.SerialNumber != "" {
		m.SerialNumber = newMeta.SerialNumber
	}
	if newMeta.MACAddress != "" {
		m.MACAddress = newMeta.MACAddress
	}
	if newMeta.Board != "" {
		m.Board = newMeta.Board
	}
	if !newMeta.BuildTime.IsZero() {
		m.BuildTime = newMeta.BuildTime
	}
	if newMeta.BatteryStatus != 0 {
		m.BatteryStatus = newMeta.BatteryStatus
	}
	if newMeta.TemperatureCelsius != 0 {
		m.TemperatureCelsius = newMeta.TemperatureCelsius
	}
	if newMeta.BatteryVoltage != 0 {
		m.BatteryVoltage = newMeta.BatteryVoltage
	}
	m.ShellColor = newMeta.ShellColor
}
