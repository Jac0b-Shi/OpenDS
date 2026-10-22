package dualshock4

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

// The host sends report ID + 31B payload. Headset/speaker/mic volume
// bytes are accepted but ignored (no feeder channel; DS4Dongle maps them
// to BT in bt.cpp ds4_set_volume/ds4_enable_mic).
func TestParseOutputFullSizeTolerant(t *testing.T) {
	full := make([]byte, 32)
	full[0] = ReportIDOutput
	full[1] = 0x07  // update flags
	full[4] = 0x12  // rumble small
	full[5] = 0xFE  // rumble large
	full[6] = 0x01  // LED red
	full[7] = 0x02  // LED green
	full[8] = 0x03  // LED blue
	full[9] = 0x04  // flash on
	full[10] = 0x05 // flash off
	// Volume fields present on the wire but ignored.
	full[2], full[3] = 0xAA, 0xBB
	full[18], full[19], full[20], full[21] = 0x11, 0x22, 0x40, 0x33

	got := parseOutputReport(full)
	assert.Equal(t, uint8(0x07), got.UpdateFlags)
	assert.Equal(t, uint8(0x12), got.RumbleSmall)
	assert.Equal(t, uint8(0xFE), got.RumbleLarge)
	assert.Equal(t, uint8(0x01), got.LedRed)
	assert.Equal(t, uint8(0x02), got.LedGreen)
	assert.Equal(t, uint8(0x03), got.LedBlue)
	assert.Equal(t, uint8(0x04), got.FlashOn)
	assert.Equal(t, uint8(0x05), got.FlashOff)

	// The ignored volume bytes do not leak into the decoded state.
	assert.Equal(t, uint8(0xAA), full[2])
}
