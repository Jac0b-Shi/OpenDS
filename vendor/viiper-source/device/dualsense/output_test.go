package dualsense

import (
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestParseOutputReportFull(t *testing.T) {
	out := make([]byte, 48)
	out[0] = ReportIDOutput
	out[1] = 0x03  // flags0
	out[2] = 0x14  // flags1: LED color + player LEDs
	out[3] = 0x11  // rumble small
	out[4] = 0x22  // rumble large
	out[5] = 0x7F  // headphones
	out[6] = 0x64  // speaker
	out[7] = 0x40  // mic
	out[8] = 0x55  // audio control
	out[9] = 0x02  // mute light
	out[10] = 0x1F // mute control
	for i := 0; i < 11; i++ {
		out[11+i] = byte(0xA0 + i) // right trigger FFB
		out[22+i] = byte(0xB0 + i) // left trigger FFB
	}
	out[33] = 0x78
	out[34] = 0x56
	out[35] = 0x34
	out[36] = 0x12 // host timestamp 0x12345678
	out[37] = 0x73 // motor power
	out[38] = 0x04 // audio control 2
	out[39] = 0x03 // flags3
	out[40] = 0x01 // haptic filter
	out[41] = 0x00 // unk
	out[42] = 0x02 // light fade
	out[43] = 0x01 // brightness
	out[44] = 0x1B // player LEDs
	out[45] = 0xFF // red
	out[46] = 0x80 // green
	out[47] = 0x10 // blue

	fb := parseOutputReport(out)
	assert.Equal(t, uint8(0x03), fb.Flags0)
	assert.Equal(t, uint8(0x14), fb.Flags1)
	assert.Equal(t, uint8(0x11), fb.RumbleSmall)
	assert.Equal(t, uint8(0x22), fb.RumbleLarge)
	assert.Equal(t, uint8(0x7F), fb.VolumeHeadphones)
	assert.Equal(t, uint8(0x64), fb.VolumeSpeaker)
	assert.Equal(t, uint8(0x40), fb.VolumeMic)
	assert.Equal(t, uint8(0x55), fb.AudioControl)
	assert.Equal(t, uint8(0x02), fb.MuteLightMode)
	assert.Equal(t, uint8(0x1F), fb.MuteControl)
	assert.Equal(t, uint8(0xA0), fb.TriggerRight[0])
	assert.Equal(t, uint8(0xAA), fb.TriggerRight[10])
	assert.Equal(t, uint8(0xB0), fb.TriggerLeft[0])
	assert.Equal(t, uint32(0x12345678), fb.HostTimestamp)
	assert.Equal(t, uint8(0x73), fb.MotorPower)
	assert.Equal(t, uint8(0x1B), fb.PlayerLeds)
	assert.Equal(t, uint8(0xFF), fb.LedRed)
	assert.Equal(t, uint8(0x80), fb.LedGreen)
	assert.Equal(t, uint8(0x10), fb.LedBlue)

	// Wire round-trip: marshal drops the report ID, unmarshal restores fields.
	wire, err := fb.MarshalBinary()
	require.NoError(t, err)
	require.Len(t, wire, OutputStateSize)
	assert.Equal(t, out[1:], wire)
	var back OutputState
	require.NoError(t, back.UnmarshalBinary(wire))
	assert.Equal(t, fb, back)
}

func TestParseOutputReportShort(t *testing.T) {
	fb := parseOutputReport([]byte{ReportIDOutput, 0x01})
	assert.Equal(t, OutputState{}, fb)
	fb = parseOutputReport(nil)
	assert.Equal(t, OutputState{}, fb)
}

func TestParseOutputReportEdgeReservedIgnored(t *testing.T) {
	ds := make([]byte, 48)
	ds[0] = ReportIDOutput
	for i := 1; i < 48; i++ {
		ds[i] = byte(i)
	}
	edge := make([]byte, 64)
	copy(edge, ds)
	for i := 48; i < 64; i++ {
		edge[i] = byte(0xA0 + i)
	}
	assert.Equal(t, parseOutputReport(ds), parseOutputReport(edge))
}

func TestTriggerEffectDecode(t *testing.T) {
	out := make([]byte, 48)
	out[0] = ReportIDOutput
	out[11] = 0x01 // R2 mode
	for i := 0; i < 10; i++ {
		out[12+i] = byte(0x10 + i)
	}
	out[22] = 0x02 // L2 mode
	out[39] = 0x02 // flags3: custom lightbar
	out[9] = 0x02  // mic LED pulse

	fb := parseOutputReport(out)
	r := fb.RightTriggerEffect()
	assert.Equal(t, uint8(0x01), r.Mode)
	assert.Equal(t, uint8(0x10), r.Params[0])
	assert.Equal(t, uint8(0x19), r.Params[9])
	l := fb.LeftTriggerEffect()
	assert.Equal(t, uint8(0x02), l.Mode)
	assert.Equal(t, uint8(0x02), fb.MicLED())
	assert.True(t, fb.LightbarCustom())

	plain := parseOutputReport(make([]byte, 48))
	assert.Equal(t, uint8(0), plain.RightTriggerEffect().Mode)
	assert.False(t, plain.LightbarCustom())
}
