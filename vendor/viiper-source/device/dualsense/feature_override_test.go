package dualsense

import (
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// Feeder-driven Edge profile proxy: physical 0x70-0x7B snapshots pushed
// by the feeder must serve verbatim instead of zero stubs.
func TestFeatureOverrideServesFeederPayload(t *testing.T) {
	de, err := new(nil, true)
	require.NoError(t, err)

	custom := make([]byte, 64)
	custom[0] = 0x70
	for i := 1; i < 64; i++ {
		custom[i] = byte(0xC0 + i)
	}
	require.True(t, de.SetFeatureReport(custom))
	got := de.getFeatureReport(0x70)
	require.Len(t, got, 64)
	assert.Equal(t, custom, got)

	// Clearing restores the zero stub.
	require.True(t, de.ClearFeatureReport(0x70))
	got = de.getFeatureReport(0x70)
	require.Len(t, got, 64)
	assert.Equal(t, uint8(0x70), got[0])
	assert.Zero(t, got[1])
}

func TestFeatureOverrideRejectsBadShapes(t *testing.T) {
	de, err := new(nil, true)
	require.NoError(t, err)

	assert.False(t, de.SetFeatureReport(nil))
	assert.False(t, de.SetFeatureReport(make([]byte, 10)))

	badID := make([]byte, 64)
	badID[0] = 0xFF
	assert.False(t, de.SetFeatureReport(badID))

	// Edge-only IDs are rejected on the standard device.
	d, err := new(nil, false)
	require.NoError(t, err)
	custom := make([]byte, 64)
	custom[0] = 0x70
	assert.False(t, d.SetFeatureReport(custom))
	assert.False(t, d.ClearFeatureReport(0x70))
}
