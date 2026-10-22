package dualshock4

import (
	"testing"

	"github.com/DualSenseClient/VIIPER/device"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// Short serials must stay hex-decodable: the 0x12/0x81 feature reports and
// the telemetry MAC halves are derived from the serial bytes.
func TestShortSerialZeroPadsForHexDecode(t *testing.T) {
	d, err := New(&device.CreateOptions{DeviceSpecific: `{"serial_number":"ABC"}`})
	require.NoError(t, err)
	assert.Equal(t, "0000000000000ABC", d.metaState.SerialNumber)
	assert.Equal(t, [8]byte{0, 0, 0, 0, 0, 0, 0x0A, 0xBC}, serialStringToBytes(d.metaState.SerialNumber))
}

// A full-length serial passes through untouched.
func TestFullSerialUntouched(t *testing.T) {
	const serial = DefaultSerialString
	d, err := New(&device.CreateOptions{DeviceSpecific: `{"serial_number":"` + serial + `"}`})
	require.NoError(t, err)
	assert.Equal(t, serial, d.metaState.SerialNumber)
}
