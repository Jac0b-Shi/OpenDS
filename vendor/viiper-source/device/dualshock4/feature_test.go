package dualshock4

import (
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func featureGet(t *testing.T, d *DualShock4, id byte) ([]byte, bool) {
	t.Helper()
	b, handled := d.HandleControl(hidClassIN, hidGetReport,
		uint16(reportTypeFeature)<<8|uint16(id), 0, 64, nil)
	return b, handled
}

// Feature GETs return the report payload WITHOUT the report ID byte,
// matching DS4Dongle (main.cpp strips the ID and trailing CRC). Lengths
// are the descriptor ReportCounts.
func TestFeaturePayloadLengths(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)
	for id, n := range map[byte]int{
		featureIDCalibration:   36,
		featureIDStatus:        4,
		featureIDProbeResponse: 2,
		featureIDSerial:        15,
		featureIDIdentity:      6,
		featureIDBoardInfo:     48,
		featureIDTelemetry:     13,
	} {
		b, handled := featureGet(t, d, id)
		require.True(t, handled, "id=0x%02X", id)
		assert.Len(t, b, n, "id=0x%02X", id)
	}
}

// Auth reports stall, as on the dongle (no PS4 license proxy).
// Unlisted IDs stall too (the dongle has no cached controller data).
func TestFeatureUnknownStalls(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)
	for _, id := range []byte{0xF0, 0xF1, 0xF2, 0xF3, 0x03, 0x05, 0x13, 0xFF} {
		_, handled := featureGet(t, d, id)
		assert.False(t, handled, "id=0x%02X", id)
	}
}

func TestFeatureControllerMAC(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)
	s := serialStringToBytes(DefaultSerialString)
	b, handled := featureGet(t, d, featureIDIdentity)
	require.True(t, handled)
	assert.Equal(t,
		[]byte{s[7], s[6], s[5], s[4], s[3], s[2]}, b)
}

func TestFeatureStatus(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)
	b, handled := featureGet(t, d, featureIDStatus)
	require.True(t, handled)
	// Battery level nibble, 12, 664 LE.
	assert.Equal(t,
		[]byte{DefaultBatteryStatus & BatteryLevelMask, 12, 0x98, 0x02}, b)
}

func TestFeatureProbeRoundTrip(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)

	// Untouched probe answers zeros.
	b, handled := featureGet(t, d, featureIDProbeResponse)
	require.True(t, handled)
	assert.Equal(t, []byte{0x00, 0x00}, b)

	// SET 0x08 stores the selector; GET 0x11 echoes it, with the
	// 0xFF/0x00/0x0C selector answered as 0x01.
	_, handled = d.HandleControl(hidClassOUT, hidSetReport,
		uint16(reportTypeFeature)<<8|uint16(featureIDProbe), 0, 4,
		[]byte{featureIDProbe, 0xFF, 0x00, 0x0C})
	require.True(t, handled)
	b, handled = featureGet(t, d, featureIDProbeResponse)
	require.True(t, handled)
	assert.Equal(t, []byte{0x01, 0x00}, b)
}

func TestFeatureTelemetryVariants(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)
	s := serialStringToBytes(DefaultSerialString)

	// Default subcommand 0x00: voltage/temperature form.
	b, handled := featureGet(t, d, featureIDTelemetry)
	require.True(t, handled)
	require.Len(t, b, 13)
	assert.Equal(t, []byte{0x00, 0x03, 0x01, 0x00, 0x04}, b[:5])

	// Subcommand 0x02: serial halves.
	_, handled = d.HandleControl(hidClassOUT, hidSetReport,
		uint16(reportTypeFeature)<<8|uint16(featureIDSubcommand), 0, 2,
		[]byte{featureIDSubcommand, 0x02})
	require.True(t, handled)
	b, handled = featureGet(t, d, featureIDTelemetry)
	require.True(t, handled)
	assert.Equal(t,
		[]byte{s[3], s[2], s[1], s[0], s[7], s[6], s[5], s[4],
			0x00, 0x00, 0x00, 0x00, 0x00}, b)
}

func TestFeatureBoardInfo(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)
	b, handled := featureGet(t, d, featureIDBoardInfo)
	require.True(t, handled)
	require.Len(t, b, 48)
	assert.Contains(t, string(b), DefaultBuildTime.Format("Jan 02 2006"))
	assert.Equal(t, []byte{0x01, 0x00}, b[32:34]) // HardwareVersionMajor
	assert.Equal(t, []byte{0x00, 0xB4}, b[34:36]) // HardwareVersionMinor
	assert.Equal(t, uint8(1), b[46])
}

// Conformance sweep: every feature ID in the HID descriptor either
// resolves to its exact payload length or stalls by policy (auth range,
// or IDs the dongle forwards to the controller — with no backing
// controller there is nothing to serve).
func TestFeatureDescriptorConformance(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)
	resolves := map[byte]int{
		0x02: 36, 0x10: 4, 0x11: 2, 0x12: 15, 0x81: 6, 0xA3: 48, 0xA4: 13,
	}
	// All 48 descriptor feature IDs (see descriptor_test.go).
	for _, id := range []byte{
		0x04, 0x02, 0x08, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15,
		0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
		0x90, 0x91, 0x92, 0x93,
		0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6,
		0xF0, 0xF1, 0xF2,
		0xA7, 0xA8, 0xA9, 0xAA, 0xAB, 0xAC, 0xAD, 0xAE, 0xAF,
		0xB0, 0xB1, 0xB2, 0xE0, 0xB3, 0xB4,
	} {
		b, handled := featureGet(t, d, id)
		if n, ok := resolves[id]; ok {
			require.True(t, handled, "id=0x%02X should resolve", id)
			assert.Len(t, b, n, "id=0x%02X", id)
			continue
		}
		assert.False(t, handled, "id=0x%02X should stall", id)
	}
}
