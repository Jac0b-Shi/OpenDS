package dualsense

import (
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestMergeMetaStateRules(t *testing.T) {
	d, err := new(nil, false)
	require.NoError(t, err)

	before := *d.metaState
	d.MergeMetaState(MetaState{SerialNumber: "NEW-SERIAL-0001"})
	assert.Equal(t, "NEW-SERIAL-0001", d.metaState.SerialNumber)
	assert.Equal(t, "NEW-SERIAL-0001", d.GetDescriptor().Strings[3])
	// Untouched fields keep current values.
	assert.Equal(t, before.MACAddress, d.metaState.MACAddress)
	assert.Equal(t, before.BatteryStatus, d.metaState.BatteryStatus)

	// Zero numerics and empty strings keep values.
	d.MergeMetaState(MetaState{BatteryStatus: 0x11})
	assert.Equal(t, uint8(0x11), d.metaState.BatteryStatus)
	assert.Equal(t, "NEW-SERIAL-0001", d.metaState.SerialNumber)
	d.MergeMetaState(MetaState{})
	assert.Equal(t, uint8(0x11), d.metaState.BatteryStatus)

	// Pairing report reflects the live MAC.
	d.MergeMetaState(MetaState{MACAddress: "01:02:03:04:05:06"})
	rep := d.featureReportPairing()
	assert.Equal(t, []byte{0x09, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01}, rep[:7])
}
