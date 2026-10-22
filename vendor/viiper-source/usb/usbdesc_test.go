package usb

import (
	"bytes"
	"testing"

	"github.com/stretchr/testify/assert"
)

// Standard endpoints use the 7-byte descriptor; audio-class isochronous
// endpoints use the 9-byte form with bRefresh and bSynchAddress
// (USB Audio Class 1.0), matching the DS5Dongle reference bytes.
func TestEndpointDescriptorWriteForms(t *testing.T) {
	ep := EndpointDescriptor{
		BEndpointAddress: 0x01,
		BMAttributes:     0x09,
		WMaxPacketSize:   392,
		BInterval:        1,
	}

	var b bytes.Buffer
	ep.Write(&b)
	assert.Equal(t, []byte{0x07, 0x05, 0x01, 0x09, 0x88, 0x01, 0x01}, b.Bytes())

	b.Reset()
	ep.AudioIso = true
	ep.Write(&b)
	assert.Equal(t, []byte{0x09, 0x05, 0x01, 0x09, 0x88, 0x01, 0x01, 0x00, 0x00}, b.Bytes())

	b.Reset()
	ep.BRefresh = 0x05
	ep.BSynchAddress = 0x82
	ep.Write(&b)
	assert.Equal(t, []byte{0x09, 0x05, 0x01, 0x09, 0x88, 0x01, 0x01, 0x05, 0x82}, b.Bytes())

	assert.Equal(t, uint8(9), ep.DescLen())
	ep.AudioIso = false
	assert.Equal(t, uint8(7), ep.DescLen())
}
