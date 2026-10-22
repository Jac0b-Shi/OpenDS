package usb

import (
	"encoding/binary"
	"testing"

	usbdesc "github.com/DualSenseClient/VIIPER/usb"
	"github.com/DualSenseClient/VIIPER/usbip"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func isoReq(lengths ...uint32) []byte {
	var offset uint32
	out := make([]byte, len(lengths)*isoDescSize)
	for i, l := range lengths {
		o := i * isoDescSize
		binary.BigEndian.PutUint32(out[o:o+4], offset)
		binary.BigEndian.PutUint32(out[o+4:o+8], l)
		binary.BigEndian.PutUint32(out[o+8:o+12], 0xDEAD)
		binary.BigEndian.PutUint32(out[o+12:o+16], 0xBEEF)
		offset += l
	}
	return out
}

func TestBuildIsoReplySinglePacket(t *testing.T) {
	got := buildIsoReply(isoReq(192), 1, 192, nil)
	require.Len(t, got, isoDescSize)
	assert.Equal(t, uint32(0), binary.BigEndian.Uint32(got[0:4]))
	assert.Equal(t, uint32(192), binary.BigEndian.Uint32(got[4:8]))
	assert.Equal(t, uint32(192), binary.BigEndian.Uint32(got[8:12]))
	assert.Equal(t, uint32(0), binary.BigEndian.Uint32(got[12:16]))
}

func TestBuildIsoReplyShortActual(t *testing.T) {
	got := buildIsoReply(isoReq(196), 1, 192, nil)
	require.Len(t, got, isoDescSize)
	assert.Equal(t, uint32(196), binary.BigEndian.Uint32(got[4:8]))
	assert.Equal(t, uint32(192), binary.BigEndian.Uint32(got[8:12]))
	assert.Equal(t, uint32(0), binary.BigEndian.Uint32(got[12:16]))
}

func TestBuildIsoReplyDistributesAcrossPackets(t *testing.T) {
	got := buildIsoReply(isoReq(192, 192), 2, 192, nil)
	require.Len(t, got, 2*isoDescSize)
	assert.Equal(t, uint32(192), binary.BigEndian.Uint32(got[8:12]))
	assert.Equal(t, uint32(0), binary.BigEndian.Uint32(got[24:28]))
	assert.Equal(t, uint32(0), binary.BigEndian.Uint32(got[12:16]))
	assert.Equal(t, uint32(0), binary.BigEndian.Uint32(got[28:32]))
}

func TestBuildIsoReplyLinuxWireLayout(t *testing.T) {
	// Linux usbip_iso_packet_descriptor: offset, length, actual_length, status.
	// Literal fixture deliberately does not use the server's field helpers.
	want := []byte{0, 0, 0, 0, 0, 0, 0, 196, 0, 0, 0, 192, 0, 0, 0, 0}
	assert.Equal(t, want, buildIsoReply(isoReq(196), 1, 192, []uint32{192}))
}

func TestBuildIsoReplyKeepsEachShortPacket(t *testing.T) {
	got := buildIsoReply(isoReq(196, 196, 196, 196), 4, 3*192, []uint32{192, 0, 192, 192})
	for i, actual := range []uint32{192, 0, 192, 192} {
		o := i * isoDescSize
		assert.Equal(t, uint32(i*196), binary.BigEndian.Uint32(got[o:o+4]))
		assert.Equal(t, actual, binary.BigEndian.Uint32(got[o+8:o+12]))
		assert.Equal(t, uint32(0), binary.BigEndian.Uint32(got[o+12:o+16]))
	}
}

func mixedEndpointDescriptor() *usbdesc.Descriptor {
	return &usbdesc.Descriptor{
		Interfaces: []usbdesc.InterfaceConfig{
			{
				Descriptor: usbdesc.InterfaceDescriptor{BInterfaceNumber: 0x01},
				Endpoints: []usbdesc.EndpointDescriptor{
					{BEndpointAddress: 0x01, BMAttributes: 0x09}, // isoc OUT
				},
			},
			{
				Descriptor: usbdesc.InterfaceDescriptor{BInterfaceNumber: 0x03},
				Endpoints: []usbdesc.EndpointDescriptor{
					{BEndpointAddress: 0x84, BMAttributes: 0x03}, // interrupt IN
				},
			},
		},
	}
}

func TestIsIsochronousEndpoint(t *testing.T) {
	desc := mixedEndpointDescriptor()
	assert.True(t, isIsochronousEndpoint(desc, 1, usbip.DirOut))
	assert.False(t, isIsochronousEndpoint(desc, 4, usbip.DirIn))
	assert.False(t, isIsochronousEndpoint(desc, 1, usbip.DirIn)) // wrong direction
	assert.False(t, isIsochronousEndpoint(desc, 0, usbip.DirIn)) // EP0 never
	assert.False(t, isIsochronousEndpoint(desc, 9, usbip.DirIn)) // unknown
}
