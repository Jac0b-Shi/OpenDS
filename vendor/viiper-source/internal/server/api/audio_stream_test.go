package api_test

import (
	"context"
	"log/slog"
	"net"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	viiperTesting "github.com/DualSenseClient/VIIPER/_testing"
	"github.com/DualSenseClient/VIIPER/device/dualsense"
	"github.com/DualSenseClient/VIIPER/device/dualshock4"
	"github.com/DualSenseClient/VIIPER/internal/server/api"
	"github.com/DualSenseClient/VIIPER/internal/server/api/handler"
	"github.com/DualSenseClient/VIIPER/usbip"
	"github.com/DualSenseClient/VIIPER/viiperclient"
	"github.com/DualSenseClient/VIIPER/virtualbus"
)

// End to end: host writes speaker PCM via USBIP OUT EP1, the TCP audio
// feeder stream delivers the exact bytes framed.
func TestAudioStream_SpeakerPassthrough(t *testing.T) {
	s := viiperTesting.NewTestServer(t)
	defer s.ApiServer.Close() //nolint:errcheck
	defer s.UsbServer.Close() //nolint:errcheck

	r := s.ApiServer.Router()
	r.Register("bus/{id}/add", handler.BusDeviceAdd(s.UsbServer, s.ApiServer))
	r.RegisterStream("bus/{busId}/{deviceid}", api.DeviceStreamHandler(s.UsbServer))
	r.RegisterStream("bus/{busId}/{deviceid}/audio", api.DeviceAudioStreamHandler(s.UsbServer))

	require.NoError(t, s.ApiServer.Start())
	time.Sleep(50 * time.Millisecond)

	b, err := virtualbus.NewWithBusID(1)
	require.NoError(t, err)
	defer b.Close() //nolint:errcheck
	require.NoError(t, s.UsbServer.AddBus(b))
	time.Sleep(50 * time.Millisecond)

	client := viiperclient.New(s.ApiServer.Addr())
	addResp, err := client.DeviceAdd(b.BusID(), "dualsense", nil)
	require.NoError(t, err)

	audio, err := client.OpenAudioStream(context.Background(), b.BusID(), addResp.DevID)
	require.NoError(t, err)
	defer audio.Close() //nolint:errcheck
	require.NoError(t, audio.SetReadDeadline(time.Now().Add(2*time.Second)))

	usbipClient := viiperTesting.NewUsbIpClient(t, s.UsbServer.Addr())
	devs, err := usbipClient.ListDevices()
	require.NoError(t, err)
	require.Len(t, devs, 1)
	imp, err := usbipClient.AttachDevice(devs[0].BusID)
	require.NoError(t, err)
	if imp != nil && imp.Conn != nil {
		defer imp.Conn.Close() //nolint:errcheck
	}
	time.Sleep(50 * time.Millisecond)

	frame := make([]byte, 384)
	for i := range frame {
		frame[i] = byte(i * 7)
	}
	require.NoError(t, usbipClient.Submit(imp.Conn, usbip.DirOut, 1, frame, nil))

	pcm, reset, err := audio.ReadFrame()
	require.NoError(t, err)
	assert.False(t, reset)
	assert.Equal(t, frame, pcm)
}

// End to end: SET_INTERFACE on the mic interface surfaces a reset barrier
// on the TCP audio stream.
func TestAudioStream_ResetBarrier(t *testing.T) {
	s := viiperTesting.NewTestServer(t)
	defer s.ApiServer.Close() //nolint:errcheck
	defer s.UsbServer.Close() //nolint:errcheck

	r := s.ApiServer.Router()
	r.Register("bus/{id}/add", handler.BusDeviceAdd(s.UsbServer, s.ApiServer))
	r.RegisterStream("bus/{busId}/{deviceid}", api.DeviceStreamHandler(s.UsbServer))
	r.RegisterStream("bus/{busId}/{deviceid}/audio", api.DeviceAudioStreamHandler(s.UsbServer))

	require.NoError(t, s.ApiServer.Start())
	time.Sleep(50 * time.Millisecond)

	b, err := virtualbus.NewWithBusID(1)
	require.NoError(t, err)
	defer b.Close() //nolint:errcheck
	require.NoError(t, s.UsbServer.AddBus(b))
	time.Sleep(50 * time.Millisecond)

	client := viiperclient.New(s.ApiServer.Addr())
	addResp, err := client.DeviceAdd(b.BusID(), "dualsense", nil)
	require.NoError(t, err)

	audio, err := client.OpenAudioStream(context.Background(), b.BusID(), addResp.DevID)
	require.NoError(t, err)
	defer audio.Close() //nolint:errcheck
	require.NoError(t, audio.SetReadDeadline(time.Now().Add(2*time.Second)))

	usbipClient := viiperTesting.NewUsbIpClient(t, s.UsbServer.Addr())
	devs, err := usbipClient.ListDevices()
	require.NoError(t, err)
	require.Len(t, devs, 1)
	imp, err := usbipClient.AttachDevice(devs[0].BusID)
	require.NoError(t, err)
	if imp != nil && imp.Conn != nil {
		defer imp.Conn.Close() //nolint:errcheck
	}
	time.Sleep(50 * time.Millisecond)

	// SET_INTERFACE mic alt 1: bm 0x01, bRequest 0x0B, wValue 1, wIndex 2.
	setup := [8]byte{0x01, 0x0B, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00}
	require.NoError(t, usbipClient.Submit(imp.Conn, usbip.DirOut, 0, nil, &setup))

	_, reset, err := audio.ReadFrame()
	require.NoError(t, err)
	assert.True(t, reset)
}

// Devices without speaker PCM fail the audio route.
func TestAudioStream_UnsupportedDevice(t *testing.T) {
	s := viiperTesting.NewTestServer(t)
	defer s.ApiServer.Close() //nolint:errcheck
	defer s.UsbServer.Close() //nolint:errcheck

	r := s.ApiServer.Router()
	r.Register("bus/{id}/add", handler.BusDeviceAdd(s.UsbServer, s.ApiServer))
	r.RegisterStream("bus/{busId}/{deviceid}", api.DeviceStreamHandler(s.UsbServer))
	r.RegisterStream("bus/{busId}/{deviceid}/audio", api.DeviceAudioStreamHandler(s.UsbServer))

	require.NoError(t, s.ApiServer.Start())
	time.Sleep(50 * time.Millisecond)

	b, err := virtualbus.NewWithBusID(1)
	require.NoError(t, err)
	defer b.Close() //nolint:errcheck
	require.NoError(t, s.UsbServer.AddBus(b))

	client := viiperclient.New(s.ApiServer.Addr())
	addResp, err := client.DeviceAdd(b.BusID(), "xbox360", nil)
	require.NoError(t, err)

	audio, err := client.OpenAudioStream(context.Background(), b.BusID(), addResp.DevID)
	require.NoError(t, err)
	defer audio.Close() //nolint:errcheck
	require.NoError(t, audio.SetReadDeadline(time.Now().Add(2*time.Second)))

	// Server closes with "not supported"; the framed read surfaces EOF.
	_, _, err = audio.ReadFrame()
	require.Error(t, err)
	assert.NotEqual(t, context.DeadlineExceeded, err)
}

// Pipe-level: TCP mic ingest lands in the queue EP2 IN serves.
func TestMicStream_IngestToEndpoint(t *testing.T) {
	dev, err := dualsense.New(nil)
	require.NoError(t, err)

	// The queue gates on the mic interface: host opens capture first.
	_, handled := dev.HandleControl(0x01, 0x0B, 1, 2, 0, nil)
	require.True(t, handled)

	server, client := net.Pipe()
	done := make(chan error, 1)
	go func() {
		done <- dualsense.MicStreamHandler(dev, slog.Default())(server)
	}()

	frame := make([]byte, 192)
	for i := range frame {
		frame[i] = byte(i + 1)
	}
	_, err = client.Write(frame)
	require.NoError(t, err)

	// Poll until the queue delivers (handler runs async).
	var got []byte
	for i := 0; i < 50; i++ {
		got = dev.HandleTransfer(context.Background(), 2, usbip.DirIn, nil)
		if len(got) == 192 && got[0] == 1 {
			break
		}
		time.Sleep(10 * time.Millisecond)
	}
	require.Len(t, got, 192)
	assert.Equal(t, frame, got)

	require.NoError(t, client.Close())
	select {
	case err := <-done:
		assert.Error(t, err) // EOF after close
	case <-time.After(2 * time.Second):
		t.Fatal("mic handler did not end after close")
	}
}

// Pipe-level: TCP DS4 mic ingest lands in the queue EP2 IN serves.
func TestDS4MicStream_IngestToEndpoint(t *testing.T) {
	dev, err := dualshock4.New(nil)
	require.NoError(t, err)

	// The queue gates on the mic interface: host opens capture first.
	_, handled := dev.HandleControl(0x01, 0x0B, 1, 2, 0, nil)
	require.True(t, handled)

	server, client := net.Pipe()
	done := make(chan error, 1)
	go func() {
		done <- dualshock4.MicStreamHandler(dev, slog.Default())(server)
	}()

	frame := make([]byte, 32)
	for i := range frame {
		frame[i] = byte(i + 1)
	}
	_, err = client.Write(frame)
	require.NoError(t, err)

	// Poll until the queue delivers (handler runs async).
	var got []byte
	for i := 0; i < 50; i++ {
		got = dev.HandleTransfer(context.Background(), 2, usbip.DirIn, nil)
		if len(got) == 32 && got[0] == 1 {
			break
		}
		time.Sleep(10 * time.Millisecond)
	}
	require.Len(t, got, 32)
	assert.Equal(t, frame, got)

	require.NoError(t, client.Close())
	select {
	case err := <-done:
		assert.Error(t, err) // EOF after close
	case <-time.After(2 * time.Second):
		t.Fatal("mic handler did not end after close")
	}
}

// End to end: runtime meta merge over TCP refreshes identity everywhere
// the host reads it (device list + USB serial string).
func TestDeviceMeta_RoundTrip(t *testing.T) {
	s := viiperTesting.NewTestServer(t)
	defer s.ApiServer.Close() //nolint:errcheck
	defer s.UsbServer.Close() //nolint:errcheck

	r := s.ApiServer.Router()
	r.Register("bus/{id}/add", handler.BusDeviceAdd(s.UsbServer, s.ApiServer))
	r.Register("bus/{id}/meta", handler.BusDeviceMeta(s.UsbServer))
	r.Register("bus/{id}/list", handler.BusDevicesList(s.UsbServer))

	require.NoError(t, s.ApiServer.Start())
	time.Sleep(50 * time.Millisecond)

	b, err := virtualbus.NewWithBusID(1)
	require.NoError(t, err)
	defer b.Close() //nolint:errcheck
	require.NoError(t, s.UsbServer.AddBus(b))

	client := viiperclient.New(s.ApiServer.Addr())
	addResp, err := client.DeviceAdd(b.BusID(), "dualsense", nil)
	require.NoError(t, err)

	updated, err := client.DeviceMeta(b.BusID(), addResp.DevID, `{"serial_number":"META-UPDATED-01"}`)
	require.NoError(t, err)
	assert.Equal(t, addResp.DevID, updated.DevID)

	list, err := client.DevicesList(b.BusID())
	require.NoError(t, err)
	require.Len(t, list.Devices, 1)
	assert.Equal(t, "META-UPDATED-01", list.Devices[0].DeviceSpecific["serial_number"])

	// Unknown device and bad JSON fail cleanly.
	_, err = client.DeviceMeta(b.BusID(), "999", `{"serial_number":"x"}`)
	require.Error(t, err)
	_, err = client.DeviceMeta(b.BusID(), addResp.DevID, `not-json`)
	require.Error(t, err)
}

// End to end: host speaker PCM surfaces on the haptics stream as the rear
// pair only.
func TestHapticsStream_RearPair(t *testing.T) {
	s := viiperTesting.NewTestServer(t)
	defer s.ApiServer.Close() //nolint:errcheck
	defer s.UsbServer.Close() //nolint:errcheck

	r := s.ApiServer.Router()
	r.Register("bus/{id}/add", handler.BusDeviceAdd(s.UsbServer, s.ApiServer))
	r.RegisterStream("bus/{busId}/{deviceid}", api.DeviceStreamHandler(s.UsbServer))
	r.RegisterStream("bus/{busId}/{deviceid}/audio", api.DeviceAudioStreamHandler(s.UsbServer))
	r.RegisterStream("bus/{busId}/{deviceid}/audio/haptics", api.DeviceHapticsStreamHandler(s.UsbServer))

	require.NoError(t, s.ApiServer.Start())
	time.Sleep(50 * time.Millisecond)

	b, err := virtualbus.NewWithBusID(1)
	require.NoError(t, err)
	defer b.Close() //nolint:errcheck
	require.NoError(t, s.UsbServer.AddBus(b))
	time.Sleep(50 * time.Millisecond)

	client := viiperclient.New(s.ApiServer.Addr())
	addResp, err := client.DeviceAdd(b.BusID(), "dualsense", nil)
	require.NoError(t, err)

	haptics, err := client.OpenHapticsStream(context.Background(), b.BusID(), addResp.DevID)
	require.NoError(t, err)
	defer haptics.Close() //nolint:errcheck
	require.NoError(t, haptics.SetReadDeadline(time.Now().Add(2*time.Second)))

	usbipClient := viiperTesting.NewUsbIpClient(t, s.UsbServer.Addr())
	devs, err := usbipClient.ListDevices()
	require.NoError(t, err)
	require.Len(t, devs, 1)
	imp, err := usbipClient.AttachDevice(devs[0].BusID)
	require.NoError(t, err)
	if imp != nil && imp.Conn != nil {
		defer imp.Conn.Close() //nolint:errcheck
	}
	time.Sleep(50 * time.Millisecond)

	// One 4ch frame: FL FR RL RR.
	frame := []byte{0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08}
	require.NoError(t, usbipClient.Submit(imp.Conn, usbip.DirOut, 1, frame, nil))

	pcm, reset, err := haptics.ReadFrame()
	require.NoError(t, err)
	assert.False(t, reset)
	assert.Equal(t, []byte{0x05, 0x06, 0x07, 0x08}, pcm)
}
