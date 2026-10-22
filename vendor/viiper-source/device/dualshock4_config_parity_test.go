package device_test

import (
	"bytes"
	"context"
	"encoding/binary"
	"testing"
	"time"

	"github.com/DualSenseClient/VIIPER/internal/server/api"
	"github.com/DualSenseClient/VIIPER/internal/server/api/handler"
	"github.com/DualSenseClient/VIIPER/viiperclient"
	"github.com/DualSenseClient/VIIPER/virtualbus"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	viiperTesting "github.com/DualSenseClient/VIIPER/_testing"

	_ "github.com/DualSenseClient/VIIPER/internal/registry" // Register devices
)

// The configuration descriptor served over USB/IP must match the DS4Dongle
// reference layout (standard build): 225 bytes (0xE1), audio isochronous
// endpoints in the 9-byte audio-class form (bRefresh/bSynchAddress), HID
// interrupt endpoints in the standard 7-byte form.
func TestDualShock4ConfigDescriptorOverUSBIP(t *testing.T) {
	s := viiperTesting.NewTestServer(t)
	defer s.UsbServer.Close() // nolint
	defer s.ApiServer.Close() // nolint

	r := s.ApiServer.Router()
	r.Register("bus/{id}/add", handler.BusDeviceAdd(s.UsbServer, s.ApiServer))
	r.RegisterStream("bus/{busId}/{deviceid}", api.DeviceStreamHandler(s.UsbServer))
	require.NoError(t, s.ApiServer.Start())

	b, err := virtualbus.NewWithBusID(1)
	require.NoError(t, err)
	defer b.Close() // nolint
	require.NoError(t, s.UsbServer.AddBus(b))

	c := viiperclient.New(s.ApiServer.Addr())
	stream, _, err := c.AddDeviceAndConnect(context.Background(), b.BusID(), "dualshock4", nil)
	require.NoError(t, err)
	require.NotNil(t, stream)
	defer stream.Close() //nolint:errcheck

	usbipClient := viiperTesting.NewUsbIpClient(t, s.UsbServer.Addr())
	var devs []viiperTesting.Device
	require.Eventually(t, func() bool {
		list, err := usbipClient.ListDevices()
		if err != nil {
			return false
		}
		devs = list
		return len(devs) == 1
	}, time.Second, 10*time.Millisecond)
	require.Equal(t, uint8(4), devs[0].NumIfaces)

	imp, err := usbipClient.AttachDevice(devs[0].BusID)
	require.NoError(t, err)
	require.NotNil(t, imp.Conn)
	defer imp.Conn.Close() // nolint

	// GET_DESCRIPTOR(CONFIGURATION), full length.
	setup := [8]byte{0x80, 0x06, 0x00, 0x02, 0x00, 0x00, 0xFF, 0x00}
	cfg, err := usbipClient.SubmitIn(imp.Conn, &setup, 255)
	require.NoError(t, err)

	require.GreaterOrEqual(t, len(cfg), 9)

	total := int(binary.LittleEndian.Uint16(cfg[2:4]))
	require.Len(t, cfg, total, "wTotalLength must match the served descriptor")
	require.Equal(t, 225, total, "DS4Dongle standard-build configuration is 225 bytes (0xE1)")

	assert.True(t, bytes.Contains(cfg, []byte{0x09, 0x05, 0x01, 0x09, 0x84, 0x00, 0x01, 0x00, 0x00}),
		"speaker EP1 must use the 9-byte audio descriptor")
	assert.True(t, bytes.Contains(cfg, []byte{0x09, 0x05, 0x82, 0x05, 0x22, 0x00, 0x01, 0x00, 0x00}),
		"mic EP2 must use the 9-byte audio descriptor")
	assert.True(t, bytes.Contains(cfg, []byte{0x07, 0x05, 0x84, 0x03, 0x40, 0x00, 0x05}),
		"HID IN EP must keep the 7-byte descriptor")
	assert.True(t, bytes.Contains(cfg, []byte{0x07, 0x05, 0x03, 0x03, 0x40, 0x00, 0x05}),
		"HID OUT EP must keep the 7-byte descriptor")

	// The HID report descriptor length in the HID class descriptor must match
	// the DS4v2 reference (507, 0x01FB).
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
