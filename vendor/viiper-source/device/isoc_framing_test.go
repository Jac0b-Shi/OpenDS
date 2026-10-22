package device_test

import (
	"context"
	"encoding/binary"
	"testing"
	"time"

	"github.com/DualSenseClient/VIIPER/internal/server/api"
	"github.com/DualSenseClient/VIIPER/internal/server/api/handler"
	"github.com/DualSenseClient/VIIPER/usbip"
	"github.com/DualSenseClient/VIIPER/viiperclient"
	"github.com/DualSenseClient/VIIPER/virtualbus"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	viiperTesting "github.com/DualSenseClient/VIIPER/_testing"

	_ "github.com/DualSenseClient/VIIPER/internal/registry" // Register devices
)

// Isochronous CMD_SUBMITs carry one 16-byte packet descriptor per packet after
// the transfer buffer. The server must consume them: leaving them unread
// desyncs the TCP stream and the next header parses as garbage (surfaced as
// "unsupported cmd 0", killing the connection). DualSense is the only VIIPER
// device with isochronous endpoints, so this exercises a full audio OUT + mic
// IN exchange followed by control + interrupt traffic on the same stream.
func TestIsocFramingKeepsStreamInSync(t *testing.T) {
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
	stream, addResp, err := c.AddDeviceAndConnect(context.Background(), b.BusID(), "dualsense", nil)
	require.NoError(t, err)
	require.NotNil(t, stream)
	require.NotNil(t, addResp)
	if stream != nil {
		defer stream.Close() //nolint:errcheck
	}

	usbipClient := viiperTesting.NewUsbIpClient(t, s.UsbServer.Addr())
	var devs []viiperTesting.Device
	require.Eventually(t, func() bool {
		list, err := usbipClient.ListDevices()
		if err != nil {
			return false
		}
		devs = list
		return len(devs) == 1
	}, 1*time.Second, 10*time.Millisecond)

	imp, err := usbipClient.AttachDevice(devs[0].BusID)
	require.NoError(t, err)
	require.NotNil(t, imp.Conn)
	defer imp.Conn.Close() // nolint

	setup := [8]byte{0x80, 0x06, 0x00, 0x01, 0x00, 0x00, 0x12, 0x00}

	// Peers may leave number_of_packets unset (0xFFFFFFFF) on non-isochronous
	// endpoints; the server must ignore the field there and stay in sync.
	require.NoError(t, usbipClient.SubmitJunkPackets(imp.Conn, usbip.DirIn, 0, nil, &setup, 0xFFFFFFFF))

	// CLEAR_FEATURE(ENDPOINT_HALT) on the speaker endpoint is host pipe-reset
	// hygiene (audio stream stop); the server ACKs it as a noop.
	clearHalt := [8]byte{0x02, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00}
	require.NoError(t, usbipClient.Submit(imp.Conn, usbip.DirOut, 0, nil, &clearHalt))

	// Speaker OUT (EP1): payload + 1 descriptor. Must not error or desync.
	outPayload := make([]byte, 16)
	for i := range outPayload {
		outPayload[i] = byte(i)
	}
	_, outDescs, err := usbipClient.SubmitIso(imp.Conn, usbip.DirOut, 1, outPayload, []uint32{16})
	require.NoError(t, err)
	require.Len(t, outDescs, 1)
	assert.Equal(t, uint32(0), outDescs[0].Status)
	assert.Equal(t, uint32(16), outDescs[0].ActualLength)

	// Mic IN (EP2): empty queue serves one silent 192B frame + 1 descriptor.
	mic, micDescs, err := usbipClient.SubmitIso(imp.Conn, usbip.DirIn, 2, nil, []uint32{192})
	require.NoError(t, err)
	require.Len(t, mic, 192)
	require.Len(t, micDescs, 1)
	assert.Equal(t, uint32(0), micDescs[0].Status)
	assert.Equal(t, uint32(192), micDescs[0].ActualLength)

	// Pipelined back-to-back mic IN URBs: the server must hold each reply for
	// one USB frame per packet so the host cannot fast-forward the stream. Ten
	// 1-packet URBs occupy ten frames (~10ms); a wall-clock budget of a few
	// hundred microseconds for the same batch means the frame clock is not
	// being honoured (which is exactly what makes games and video players run
	// at high speed when the virtual device is the default audio output).
	const urbs = 10
	burstStart := time.Now()
	var total uint32
	for i := 0; i < urbs; i++ {
		data, descs, err := usbipClient.SubmitIso(imp.Conn, usbip.DirIn, 2, nil, []uint32{192})
		require.NoError(t, err)
		require.Len(t, descs, 1)
		assert.Equal(t, uint32(0), descs[0].Status)
		total += uint32(len(data))
	}
	burst := time.Since(burstStart)
	assert.Equal(t, uint32(urbs*192), total, "every pipelined URB must be served")
	assert.GreaterOrEqual(t, burst, 9*time.Millisecond,
		"%d one-packet URBs must occupy ~%d USB frames (took %v)", urbs, urbs, burst)
	assert.Less(t, burst, 5*time.Second, "frame pacing must not wedge the stream")

	// A multi-packet URB covers several USB frames, and each packet is its own
	// service opportunity: every packet must carry a real mic frame (192B),
	// not one frame followed by empty packets.
	micMulti, multiDescs, err := usbipClient.SubmitIso(imp.Conn, usbip.DirIn, 2, nil, []uint32{192, 192, 192, 192})
	require.NoError(t, err)
	require.Len(t, multiDescs, 4)
	require.Len(t, micMulti, 4*192)
	for i, d := range multiDescs {
		assert.Equal(t, uint32(192), d.ActualLength, "packet %d must carry a full frame", i)
	}

	// Windows reserves the asynchronous endpoint's 196-byte maximum per packet.
	// The device still returns 192 PCM bytes at every service opportunity. USB/IP
	// packs those bytes contiguously and the descriptors retain requested offsets.
	micAsync, asyncDescs, err := usbipClient.SubmitIso(imp.Conn, usbip.DirIn, 2, nil, []uint32{196, 196, 196, 196})
	require.NoError(t, err)
	require.Len(t, micAsync, 4*192)
	require.Len(t, asyncDescs, 4)
	for i, d := range asyncDescs {
		assert.Equal(t, uint32(i*196), d.Offset)
		assert.Equal(t, uint32(196), d.Length)
		assert.Equal(t, uint32(192), d.ActualLength, "packet %d", i)
		assert.Zero(t, d.Status, "packet %d", i)
	}

	// A smaller host request must truncate each packet before packing it.
	micShort, shortDescs, err := usbipClient.SubmitIso(imp.Conn, usbip.DirIn, 2, nil, []uint32{96, 96})
	require.NoError(t, err)
	require.Len(t, micShort, 192)
	require.Len(t, shortDescs, 2)
	for _, d := range shortDescs {
		assert.Equal(t, uint32(96), d.ActualLength)
		assert.Zero(t, d.Status)
	}

	// Queue numbered synthetic mic frames, then pipeline multi-packet URBs as
	// Windows does. Sharing a per-frame clock between URB goroutines must not
	// round-robin their reads: each host buffer needs consecutive audio frames.
	openMic := [8]byte{0x01, 0x0b, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00}
	require.NoError(t, usbipClient.Submit(imp.Conn, usbip.DirOut, 0, nil, &openMic))
	micDevice, ok := b.Devices()[0].(interface{ QueueMicrophonePCM([]byte) bool })
	require.True(t, ok)
	const pipelineURBs = 4
	const packetsPerURB = 4
	// Fill the mic's independent 128ms pre-roll before issuing the URBs.
	for i := 0; i < 128; i++ {
		frame := make([]byte, 192)
		for j := range frame {
			frame[j] = byte(i + 1)
		}
		require.True(t, micDevice.QueueMicrophonePCM(frame))
	}
	require.NoError(t, imp.Conn.SetDeadline(time.Now().Add(2*time.Second)))
	for i := uint32(0); i < pipelineURBs; i++ {
		cmd := usbip.CmdSubmit{
			Basic:             usbip.HeaderBasic{Command: usbip.CmdSubmitCode, Seqnum: 10000 + i, Dir: usbip.DirIn, Ep: 2},
			TransferBufferLen: packetsPerURB * 196,
			NumberOfPackets:   packetsPerURB,
		}
		require.NoError(t, cmd.Write(imp.Conn))
		descriptors := make([]byte, packetsPerURB*16)
		for j := uint32(0); j < packetsPerURB; j++ {
			binary.BigEndian.PutUint32(descriptors[j*16:j*16+4], j*196)
			binary.BigEndian.PutUint32(descriptors[j*16+4:j*16+8], 196)
		}
		_, err := imp.Conn.Write(descriptors)
		require.NoError(t, err)
	}
	for i := 0; i < pipelineURBs; i++ {
		header := make([]byte, 48)
		require.NoError(t, usbip.ReadExactly(imp.Conn, header))
		seq := binary.BigEndian.Uint32(header[4:8])
		require.Equal(t, uint32(10000+i), seq, "isochronous mic URBs must finish in submission order")
		require.Equal(t, uint32(packetsPerURB*192), binary.BigEndian.Uint32(header[24:28]))
		payload := make([]byte, packetsPerURB*192)
		require.NoError(t, usbip.ReadExactly(imp.Conn, payload))
		for j := 0; j < packetsPerURB; j++ {
			// Interior samples identify frame order independently of the mic's
			// fractional interpolation across adjacent frame boundaries.
			for _, sample := range payload[j*192+32 : (j+1)*192-32] {
				require.Equal(t, byte(i*packetsPerURB+j+1), sample, "URB %d packet %d", i, j)
			}
		}
		descriptors := make([]byte, packetsPerURB*16)
		require.NoError(t, usbip.ReadExactly(imp.Conn, descriptors))
	}
	require.NoError(t, imp.Conn.SetDeadline(time.Time{}))

	// Plain control + interrupt traffic on the same stream proves framing held.
	require.NoError(t, usbipClient.Submit(imp.Conn, usbip.DirIn, 0, nil, &setup))
	require.NoError(t, usbipClient.Submit(imp.Conn, usbip.DirIn, 4, nil, nil))
}
