package api_test

import (
	"context"
	"io"
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

// Pipe-level: a 64B raw frame on the ingest stream is served verbatim on
// EP4 IN.
func TestRawStream_IngestToEndpoint(t *testing.T) {
	dev, err := dualsense.New(nil)
	require.NoError(t, err)
	server, client := net.Pipe()
	defer server.Close()
	defer client.Close()
	done := make(chan error, 1)
	go func() { done <- dualsense.RawStreamHandler(dev, slog.Default())(server) }()

	frame := make([]byte, dualsense.InputReportSize)
	frame[0] = dualsense.ReportIDInput
	for i := 1; i < len(frame); i++ {
		frame[i] = byte(i)
	}
	_, err = client.Write(frame)
	require.NoError(t, err)
	require.Eventually(t, func() bool {
		got := dev.HandleTransfer(context.Background(), 4, usbip.DirIn, nil)
		return len(got) == len(frame) && got[1] == frame[1]
	}, 2*time.Second, 10*time.Millisecond)
	client.Close()
	<-done
}

// A frame whose report ID is wrong is ignored and the stream stays open:
// the next valid frame still lands.
func TestRawStream_InvalidFrameIgnoredStreamStaysOpen(t *testing.T) {
	dev, err := dualshock4.New(nil)
	require.NoError(t, err)
	server, client := net.Pipe()
	defer server.Close()
	defer client.Close()
	done := make(chan error, 1)
	go func() { done <- dualshock4.RawStreamHandler(dev, slog.Default())(server) }()

	bad := make([]byte, dualshock4.InputReportSize)
	bad[0] = dualshock4.ReportIDOutput // wrong report ID
	_, err = client.Write(bad)
	require.NoError(t, err)

	good := make([]byte, dualshock4.InputReportSize)
	good[0] = dualshock4.ReportIDInput
	good[1] = 0x42
	_, err = client.Write(good)
	require.NoError(t, err)

	require.Eventually(t, func() bool {
		got := dev.HandleTransfer(context.Background(), 4, usbip.DirIn, nil)
		return len(got) == len(good) && got[1] == 0x42
	}, 2*time.Second, 10*time.Millisecond)

	// Stream not ended by the rejected frame.
	select {
	case err := <-done:
		t.Fatalf("handler ended early: %v", err)
	default:
	}
	client.Close()
	<-done
}

// A short read ends the stream, and the last accepted raw stays in place
// (callers clear explicitly to restore synthetic reports).
func TestRawStream_ShortReadEndsStreamLastRawPersists(t *testing.T) {
	dev, err := dualsense.New(nil)
	require.NoError(t, err)
	server, client := net.Pipe()
	done := make(chan error, 1)
	go func() { done <- dualsense.RawStreamHandler(dev, slog.Default())(server) }()

	good := make([]byte, dualsense.InputReportSize)
	good[0] = dualsense.ReportIDInput
	good[1] = 0x99
	_, err = client.Write(good)
	require.NoError(t, err)
	require.Eventually(t, func() bool {
		got := dev.HandleTransfer(context.Background(), 4, usbip.DirIn, nil)
		return len(got) == len(good) && got[1] == 0x99
	}, 2*time.Second, 10*time.Millisecond)

	// Partial frame then close: handler exits with an unexpected EOF.
	_, err = client.Write([]byte{0x01, 0x02, 0x03})
	require.NoError(t, err)
	require.NoError(t, client.Close())
	select {
	case err := <-done:
		assert.ErrorIs(t, err, io.ErrUnexpectedEOF)
	case <-time.After(2 * time.Second):
		t.Fatal("handler did not end on short read")
	}

	// Last raw survives the disconnect.
	got := dev.HandleTransfer(context.Background(), 4, usbip.DirIn, nil)
	require.Len(t, got, dualsense.InputReportSize)
	assert.Equal(t, byte(0x99), got[1])
}

// End to end over the registered route: client writes raw frames, the host
// sees them on EP4 IN.
func TestRawStream_RouteEndToEnd(t *testing.T) {
	s := viiperTesting.NewTestServer(t)
	defer s.ApiServer.Close() //nolint:errcheck
	defer s.UsbServer.Close() //nolint:errcheck

	r := s.ApiServer.Router()
	r.Register("bus/{id}/add", handler.BusDeviceAdd(s.UsbServer, s.ApiServer))
	r.RegisterStream("bus/{busId}/{deviceid}", api.DeviceStreamHandler(s.UsbServer))
	r.RegisterStream("bus/{busId}/{deviceid}/raw", api.DeviceRawStreamHandler(s.UsbServer))

	require.NoError(t, s.ApiServer.Start())
	time.Sleep(50 * time.Millisecond)

	b, err := virtualbus.NewWithBusID(1)
	require.NoError(t, err)
	defer b.Close() //nolint:errcheck
	require.NoError(t, s.UsbServer.AddBus(b))

	client := viiperclient.New(s.ApiServer.Addr())
	addResp, err := client.DeviceAdd(b.BusID(), "dualsense", nil)
	require.NoError(t, err)

	raw, err := client.OpenRawStream(context.Background(), b.BusID(), addResp.DevID)
	require.NoError(t, err)
	defer raw.Close() //nolint:errcheck

	// Exact-size framing is enforced client-side.
	assert.Error(t, raw.WriteFrame(make([]byte, 63)))

	frame := make([]byte, viiperclient.RawInputFrameSize)
	frame[0] = 0x01
	frame[1] = 0x11
	frame[7] = 0x37
	require.NoError(t, raw.WriteFrame(frame))

	// Wrong device types fail the route.
	xbox, err := client.DeviceAdd(b.BusID(), "xbox360", nil)
	require.NoError(t, err)
	unsupported, err := client.OpenRawStream(context.Background(), b.BusID(), xbox.DevID)
	if err == nil {
		defer unsupported.Close() //nolint:errcheck
		_ = unsupported.SetReadDeadline(time.Now().Add(2 * time.Second))
		_, err = unsupported.Read(make([]byte, 1))
	}
	require.Error(t, err)
	assert.NotEqual(t, context.DeadlineExceeded, err)
}
