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
	got := buildIsoReply(isoReq(192), 1, 192)
	require.Len(t, got, isoDescSize)
	assert.Equal(t, uint32(0), binary.BigEndian.Uint32(got[0:4]))
	assert.Equal(t, uint32(192), binary.BigEndian.Uint32(got[4:8]))
	assert.Equal(t, uint32(0), binary.BigEndian.Uint32(got[8:12]))
	assert.Equal(t, uint32(192), binary.BigEndian.Uint32(got[12:16]))
}

func TestBuildIsoReplyShortActual(t *testing.T) {
	got := buildIsoReply(isoReq(196), 1, 192)
	require.Len(t, got, isoDescSize)
	assert.Equal(t, uint32(196), binary.BigEndian.Uint32(got[4:8]))
	assert.Equal(t, uint32(0), binary.BigEndian.Uint32(got[8:12]))
	assert.Equal(t, uint32(192), binary.BigEndian.Uint32(got[12:16]))
}

func TestBuildIsoReplyDistributesAcrossPackets(t *testing.T) {
	got := buildIsoReply(isoReq(192, 192), 2, 192)
	require.Len(t, got, 2*isoDescSize)
	assert.Equal(t, uint32(192), binary.BigEndian.Uint32(got[12:16]))
	assert.Equal(t, uint32(0), binary.BigEndian.Uint32(got[28:32]))
	assert.Equal(t, uint32(0), binary.BigEndian.Uint32(got[8:12]))
	assert.Equal(t, uint32(0), binary.BigEndian.Uint32(got[24:28]))
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
