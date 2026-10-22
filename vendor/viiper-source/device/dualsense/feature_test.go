package dualsense

import (
	"testing"

	"github.com/DualSenseClient/VIIPER/usb"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// descriptorFeatureCounts extracts reportID -> payload count for every
// Feature main item in a HID report descriptor. IDs without an explicit
// ReportCount inherit the running count, mirroring HID semantics.
func descriptorFeatureCounts(t *testing.T, edge bool) map[byte]int {
	t.Helper()
	d, err := new(nil, edge)
	require.NoError(t, err)
	var hidIF *usb.HIDFunction
	for i := range d.GetDescriptor().Interfaces {
		if d.GetDescriptor().Interfaces[i].HID != nil {
			hidIF = d.GetDescriptor().Interfaces[i].HID
		}
	}
	require.NotNil(t, hidIF)
	rb, err := hidIF.ReportBytes()
	require.NoError(t, err)
	b := []byte(rb)

	out := map[byte]int{}
	var pending *byte
	lastCount := 0
	for i := 0; i < len(b); {
		switch b[i] {
		case 0x85: // Report ID
			require.Less(t, i+1, len(b))
			id := b[i+1]
			pending = &id
			i += 2
		case 0x95: // Report Count
			require.Less(t, i+1, len(b))
			lastCount = int(b[i+1])
			i += 2
		case 0xB1: // Feature main
			require.NotNil(t, pending, "feature main without report ID")
			out[*pending] = lastCount
			pending = nil
			i += 2
		case 0x81, 0x91: // Input/Output main: not a feature
			pending = nil
			i += 2
		default:
			i++
		}
	}
	return out
}

// Every feature ID in the descriptor must resolve to a GET response of
// exactly count+1 bytes on its variant.
func TestFeatureTableMatchesDescriptor(t *testing.T) {
	for _, edge := range []bool{false, true} {
		d, err := new(nil, edge)
		require.NoError(t, err)
		for id, count := range descriptorFeatureCounts(t, edge) {
			b := d.getFeatureReport(id)
			require.NotNil(t, b, "edge=%v id=0x%02X", edge, id)
			assert.Len(t, b, count+1, "edge=%v id=0x%02X", edge, id)
			assert.Equal(t, id, b[0])
		}
	}
}

// Unknown IDs must not resolve (host sees a stall, as before).
func TestFeatureUnknownStalls(t *testing.T) {
	d, err := new(nil, false)
	require.NoError(t, err)
	assert.Nil(t, d.getFeatureReport(0xFF))
	assert.Nil(t, d.getFeatureReport(0x04))
	de, err := new(nil, true)
	require.NoError(t, err)
	assert.Nil(t, de.getFeatureReport(0xFF))
	// Edge-only IDs stall on DS.
	assert.Nil(t, d.getFeatureReport(0x70))
	assert.NotNil(t, de.getFeatureReport(0x70))
}

// Edge 0x65 defaults to an echo of the 0x20 firmware body.
func TestEdgeHandshakeDefaultsToFirmwareEcho(t *testing.T) {
	de, err := new(nil, true)
	require.NoError(t, err)
	got := de.getFeatureReport(0x65)
	require.Len(t, got, 64)
	assert.Equal(t, uint8(0x65), got[0])
	fw := de.featureReportFirmware()
	assert.Equal(t, fw[1:], got[1:])

	// Absent from the DS descriptor: stalls there.
	d, err := new(nil, false)
	require.NoError(t, err)
	assert.Nil(t, d.getFeatureReport(0x65))
}

// SET 0x65 stores the payload; GET serves it back verbatim.
func TestEdgeHandshakeSetEcho(t *testing.T) {
	de, err := new(nil, true)
	require.NoError(t, err)

	payload := make([]byte, 64)
	payload[0] = 0x65
	for i := 1; i < 64; i++ {
		payload[i] = byte(i)
	}
	_, handled := de.HandleControl(0x21, 0x09, 0x0365, 0x0003, 64, payload)
	require.True(t, handled)

	got := de.getFeatureReport(0x65)
	require.Len(t, got, 64)
	assert.Equal(t, payload, got)

	// Short SETs are accepted but not stored.
	_, handled = de.HandleControl(0x21, 0x09, 0x0365, 0x0003, 3, []byte{0x65, 0x01, 0x02})
	require.True(t, handled)
	assert.Equal(t, payload, de.getFeatureReport(0x65))

	// DS accepts the SET but never serves it.
	d, err := new(nil, false)
	require.NoError(t, err)
	_, handled = d.HandleControl(0x21, 0x09, 0x0365, 0x0003, 64, payload)
	require.True(t, handled)
	assert.Nil(t, d.getFeatureReport(0x65))
}
