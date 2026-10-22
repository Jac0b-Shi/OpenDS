package dualshock4

import (
	"bytes"
	"testing"

	"github.com/DualSenseClient/VIIPER/usb"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// configDescriptorBytes assembles the configuration descriptor in the same
// order as the USB server (internal/server/usb buildConfigDescriptor), using
// only exported helpers, so the device-side descriptor data can be parity
// checked from here.
func configDescriptorBytes(t *testing.T, d *DualShock4) []byte {
	t.Helper()
	desc := d.GetDescriptor()
	var b bytes.Buffer
	h := usb.ConfigHeader{
		BNumInterfaces:      desc.NumInterfaces(),
		BConfigurationValue: desc.Configuration.BConfigurationValue,
		IConfiguration:      desc.Configuration.IConfiguration,
		BMAttributes:        desc.Configuration.BMAttributes,
		BMaxPower:           desc.Configuration.BMaxPower,
	}
	h.Write(&b)
	for _, iface := range desc.Interfaces {
		iface.Descriptor.Write(&b)
		if iface.HID != nil {
			hd, err := iface.HID.DescriptorBytes()
			require.NoError(t, err)
			b.Write([]byte(hd))
		}
		for _, cd := range iface.ClassDescriptors {
			b.Write([]byte(cd.Bytes()))
		}
		for _, ep := range iface.Endpoints {
			ep.Write(&b)
			for _, cd := range ep.ClassDescriptors {
				b.Write([]byte(cd.Bytes()))
			}
		}
	}
	out := b.Bytes()
	require.GreaterOrEqual(t, len(out), 4)
	out[2] = byte(len(out))
	out[3] = byte(len(out) >> 8)
	return out
}

// The full configuration must stay layout-identical to the DS4Dongle
// standard build: 225 bytes (0xE1), 4 interfaces, audio isochronous
// endpoints in the 9-byte audio form (bRefresh/bSynchAddress), HID
// interrupt endpoints in the standard 7-byte form.
func TestConfigDescriptorMatchesDS4Layout(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)
	desc := d.GetDescriptor()
	assert.Equal(t, uint8(4), desc.NumInterfaces())
	require.Len(t, desc.Interfaces, 6)

	type ifID struct {
		num, alt uint8
	}
	var got []ifID
	for _, iface := range desc.Interfaces {
		got = append(got, ifID{iface.Descriptor.BInterfaceNumber, iface.Descriptor.BAlternateSetting})
	}
	assert.Equal(t, []ifID{{0, 0}, {1, 0}, {1, 1}, {2, 0}, {2, 1}, {3, 0}}, got)

	cfg := configDescriptorBytes(t, d)
	require.Len(t, cfg, 225)
	assert.Equal(t, []byte{0xE1, 0x00}, cfg[2:4])
	assert.True(t, bytes.Contains(cfg, []byte{0x09, 0x05, 0x01, 0x09, 0x84, 0x00, 0x01, 0x00, 0x00}),
		"speaker EP1 must use the 9-byte audio descriptor")
	assert.True(t, bytes.Contains(cfg, []byte{0x09, 0x05, 0x82, 0x05, 0x22, 0x00, 0x01, 0x00, 0x00}),
		"mic EP2 must use the 9-byte audio descriptor")
	assert.True(t, bytes.Contains(cfg, []byte{0x07, 0x05, 0x84, 0x03, 0x40, 0x00, 0x05}),
		"HID IN EP must keep the 7-byte descriptor")
	assert.True(t, bytes.Contains(cfg, []byte{0x07, 0x05, 0x03, 0x03, 0x40, 0x00, 0x05}),
		"HID OUT EP must keep the 7-byte descriptor")
	assert.True(t, bytes.Contains(cfg, []byte{0x09, 0x21, 0x11, 0x01, 0x00, 0x01, 0x22, 0xFB, 0x01}),
		"HID class descriptor must advertise the 507-byte DS4 report descriptor")

	// Descriptor walk: every bLength chain must land exactly on the end.
	for off := 0; off < len(cfg); {
		l := int(cfg[off])
		require.NotZero(t, l, "zero bLength at offset %d", off)
		require.LessOrEqual(t, off+l, len(cfg), "descriptor overruns at offset %d", off)
		off += l
	}
}

// Class-specific audio blobs must total the reference sizes: AC header
// block 71 (0x47) on IF0, 18 per streaming alt (7 AS general + 11 format).
// Together with the fixed interface/endpoint/HID bytes this yields the
// 225-byte configuration.
func TestAudioClassBlobSizes(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)
	byAlt := map[[2]uint8]usb.InterfaceConfig{}
	for _, iface := range d.GetDescriptor().Interfaces {
		byAlt[[2]uint8{iface.Descriptor.BInterfaceNumber, iface.Descriptor.BAlternateSetting}] = iface
	}
	classBytes := func(iface usb.InterfaceConfig) int {
		n := 0
		for _, cd := range iface.ClassDescriptors {
			n += len(cd.Bytes())
		}
		return n
	}
	assert.Equal(t, 71, classBytes(byAlt[[2]uint8{0, 0}]))
	assert.Equal(t, 18, classBytes(byAlt[[2]uint8{1, 1}]))
	assert.Equal(t, 18, classBytes(byAlt[[2]uint8{2, 1}]))
}
