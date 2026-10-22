package dualsense

import (
	"bytes"
	"testing"

	"github.com/DualSenseClient/VIIPER/usb"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func hidConfig(t *testing.T, edge bool) usb.InterfaceConfig {
	t.Helper()
	d, err := new(nil, edge)
	require.NoError(t, err)
	desc := d.GetDescriptor()
	require.NotNil(t, desc)
	for _, iface := range desc.Interfaces {
		if iface.HID != nil {
			return iface
		}
	}
	t.Fatal("no HID interface in descriptor")
	return usb.InterfaceConfig{}
}

func reportBytes(t *testing.T, edge bool) []byte {
	t.Helper()
	iface := hidConfig(t, edge)
	rb, err := iface.HID.ReportBytes()
	require.NoError(t, err)
	return []byte(rb)
}

// Report descriptors must match DS5Dongle reference lengths:
// DS 321B, Edge 437B.
func TestReportDescriptorLengths(t *testing.T) {
	assert.Len(t, reportBytes(t, false), 321)
	assert.Len(t, reportBytes(t, true), 437)
}

// Report 0x02 output count: DS 47 (0x2F), Edge 63 (0x3F).
func TestReportDescriptorOutputCount(t *testing.T) {
	ds := reportBytes(t, false)
	assert.True(t, bytes.Contains(ds, []byte{0x85, 0x02, 0x09, 0x23, 0x95, 0x2F}))
	dse := reportBytes(t, true)
	assert.True(t, bytes.Contains(dse, []byte{0x85, 0x02, 0x09, 0x23, 0x95, 0x3F}))
}

// Report 0xF2 feature count: DS 15 (0x0F), Edge 52 (0x34).
func TestReportDescriptorF2Count(t *testing.T) {
	ds := reportBytes(t, false)
	assert.True(t, bytes.Contains(ds, []byte{0x85, 0xF2, 0x09, 0x32, 0x95, 0x0F}))
	dse := reportBytes(t, true)
	assert.True(t, bytes.Contains(dse, []byte{0x85, 0xF2, 0x09, 0x32, 0x95, 0x34}))
}

// Edge-only reports must exist on Edge and be absent on DS.
func TestReportDescriptorEdgeOnly(t *testing.T) {
	ds := reportBytes(t, false)
	dse := reportBytes(t, true)
	for _, id := range []byte{0x60, 0x61, 0x68, 0x70, 0x7B} {
		assert.True(t, bytes.Contains(dse, []byte{0x85, id}), "edge missing 0x%02X", id)
		assert.False(t, bytes.Contains(ds, []byte{0x85, id}), "ds unexpectedly has 0x%02X", id)
	}
	// DS-only tail must exist on both (Edge reuses 0xF6-0xF9).
	for _, id := range []byte{0xF6, 0xF9} {
		assert.True(t, bytes.Contains(ds, []byte{0x85, id}))
		assert.True(t, bytes.Contains(dse, []byte{0x85, id}))
	}
}

func TestDeviceDescriptorFields(t *testing.T) {
	d, err := new(nil, false)
	require.NoError(t, err)
	dev := d.GetDescriptor().Device
	assert.Equal(t, uint16(DefaultVID), dev.IDVendor)
	assert.Equal(t, uint16(DefaultPIDDS), dev.IDProduct)
	assert.Equal(t, uint8(0x03), dev.ISerialNumber)
	assert.Equal(t, uint16(0x0100), dev.BcdDevice)

	de, err := new(nil, true)
	require.NoError(t, err)
	assert.Equal(t, uint16(DefaultPIDDSEdge), de.GetDescriptor().Device.IDProduct)

	cfg := d.GetDescriptor().Configuration
	assert.Equal(t, uint8(0xC0), cfg.BMAttributes)
	assert.Equal(t, uint8(0xFA), cfg.BMaxPower)
}

func TestHIDEndpointsInterval1(t *testing.T) {
	iface := hidConfig(t, false)
	assert.Equal(t, uint8(0x03), iface.Descriptor.BInterfaceNumber)
	eps := iface.Endpoints
	require.Len(t, eps, 2)
	for _, ep := range eps {
		assert.Equal(t, uint8(1), ep.BInterval)
	}
}

func TestAudioInterfaces(t *testing.T) {
	d, err := new(nil, false)
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

	// IF1 alt1: EP1 OUT isoc adaptive 392B; IF2 alt1: EP2 IN isoc async 196B.
	outEP := desc.Interfaces[2].Endpoints
	require.Len(t, outEP, 1)
	assert.Equal(t, uint8(0x01), outEP[0].BEndpointAddress)
	assert.Equal(t, uint8(0x09), outEP[0].BMAttributes)
	assert.Equal(t, uint16(392), outEP[0].WMaxPacketSize)
	assert.Equal(t, uint8(1), outEP[0].BInterval)
	inEP := desc.Interfaces[4].Endpoints
	require.Len(t, inEP, 1)
	assert.Equal(t, uint8(0x82), inEP[0].BEndpointAddress)
	assert.Equal(t, uint8(0x05), inEP[0].BMAttributes)
	assert.Equal(t, uint16(196), inEP[0].WMaxPacketSize)
}

// Class-specific audio blobs must total the reference sizes: AC header
// block 73 (0x49) on IF0, 18 per streaming alt (7 AS general + 11 format).
// Together with the fixed interface/endpoint/HID bytes this yields the
// 227-byte configuration.
func TestAudioClassBlobSizes(t *testing.T) {
	d, err := new(nil, false)
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
	assert.Equal(t, 73, classBytes(byAlt[[2]uint8{0, 0}]))
	assert.Equal(t, 18, classBytes(byAlt[[2]uint8{1, 1}]))
	assert.Equal(t, 18, classBytes(byAlt[[2]uint8{2, 1}]))
}

func TestSerialStringPerDevice(t *testing.T) {
	d, err := new(nil, false)
	require.NoError(t, err)
	s := d.GetDescriptor().Strings
	assert.Equal(t, "Sony Interactive Entertainment", s[1])
	assert.Equal(t, "DualSense Wireless Controller", s[2])
	assert.Equal(t, DefaultSerialNumberDS, s[3])

	// Index 0 must encode to the en-US LANGID descriptor (bLength 4,
	// type STRING, wLangID 0x0409). A lookalike rune here breaks string
	// fetching on the wire.
	assert.Equal(t, []byte{4, 3, 0x09, 0x04}, usb.EncodeStringDescriptor(s[0]))

	de, err := new(nil, true)
	require.NoError(t, err)
	se := de.GetDescriptor().Strings
	assert.Equal(t, "DualSense Edge Wireless Controller", se[2])
	assert.Equal(t, DefaultSerialNumberDSEdge, se[3])

	// Per-device maps must not alias: creating an Edge must not rewrite DS strings.
	assert.Equal(t, "DualSense Wireless Controller", d.GetDescriptor().Strings[2])
}

// configDescriptorBytes assembles the configuration descriptor in the same
// order as the USB server (internal/server/usb buildConfigDescriptor), using
// only exported helpers, so the device-side descriptor data can be parity
// checked from here.
func configDescriptorBytes(t *testing.T, d *DualSense) []byte {
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

// The full configuration must stay layout-identical to the DS5Dongle standard
// build (no serial/wake): 227 bytes (0xE3), audio isochronous endpoints in the
// 9-byte audio form (bRefresh/bSynchAddress), HID interrupt endpoints in the
// standard 7-byte form. Regression guard for the config previously being
// 223 bytes with 7-byte audio endpoints.
func TestConfigDescriptorMatchesDS5DongleLayout(t *testing.T) {
	for _, edge := range []bool{false, true} {
		d, err := new(nil, edge)
		require.NoError(t, err)
		got := configDescriptorBytes(t, d)

		require.Len(t, got, 227, "edge=%v", edge)
		assert.Equal(t, []byte{0xE3, 0x00}, got[2:4], "edge=%v", edge)
		assert.True(t, bytes.Contains(got, []byte{0x09, 0x05, 0x01, 0x09, 0x88, 0x01, 0x01, 0x00, 0x00}),
			"speaker EP1 must use the 9-byte audio descriptor (edge=%v)", edge)
		assert.True(t, bytes.Contains(got, []byte{0x09, 0x05, 0x82, 0x05, 0xC4, 0x00, 0x01, 0x00, 0x00}),
			"mic EP2 must use the 9-byte audio descriptor (edge=%v)", edge)
		assert.True(t, bytes.Contains(got, []byte{0x07, 0x05, 0x84, 0x03, 0x40, 0x00, 0x01}),
			"HID IN EP must keep the 7-byte descriptor (edge=%v)", edge)
		assert.True(t, bytes.Contains(got, []byte{0x07, 0x05, 0x03, 0x03, 0x40, 0x00, 0x01}),
			"HID OUT EP must keep the 7-byte descriptor (edge=%v)", edge)
	}
}
