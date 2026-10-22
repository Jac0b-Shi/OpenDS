package dualshock4

import (
	"context"
	"encoding/binary"
	"io"
	"log/slog"
	"net"
	"testing"
	"time"

	"github.com/DualSenseClient/VIIPER/usbip"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestSetInterfaceAlt(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)

	// SET_INTERFACE: bmRequestType 0x01, bRequest 0x0B, wValue alt, wIndex iface.
	_, handled := d.HandleControl(0x01, 0x0B, 1, 1, 0, nil)
	require.True(t, handled)
	d.mtx.Lock()
	assert.Equal(t, uint8(1), d.alts[1])
	d.mtx.Unlock()

	_, handled = d.HandleControl(0x01, 0x0B, 1, 2, 0, nil)
	require.True(t, handled)
	d.mtx.Lock()
	assert.Equal(t, uint8(1), d.alts[2])
	d.mtx.Unlock()

	// Out-of-range interface is accepted but ignored.
	_, handled = d.HandleControl(0x01, 0x0B, 1, 9, 0, nil)
	require.True(t, handled)
}

//nolint:unparam // bm documents USB bmRequestType (always 0xA1 for UAC GET).
func uacGet(t *testing.T, d *DualShock4, bm, req uint8, cs, entity uint8) []byte {
	t.Helper()
	wValue := uint16(cs)<<8 | 0 // master channel
	wIndex := uint16(entity)<<8 | 0
	resp, handled := d.HandleControl(bm, req, wValue, wIndex, 64, nil)
	require.True(t, handled, "bm=0x%02X req=0x%02X cs=%d entity=0x%02X", bm, req, cs, entity)
	return resp
}

func uacSet(t *testing.T, d *DualShock4, cs, entity uint8, data []byte) {
	t.Helper()
	wValue := uint16(cs)<<8 | 0
	wIndex := uint16(entity)<<8 | 0
	_, handled := d.HandleControl(0x21, 0x01, wValue, wIndex, uint16(len(data)), data)
	require.True(t, handled)
}

func TestUACMuteRoundTrip(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)

	assert.Equal(t, []byte{0x00}, uacGet(t, d, 0xA1, 0x81, 1, 0x02))
	uacSet(t, d, 1, 0x02, []byte{0x01})
	assert.Equal(t, []byte{0x01}, uacGet(t, d, 0xA1, 0x81, 1, 0x02))

	assert.Equal(t, []byte{0x00}, uacGet(t, d, 0xA1, 0x81, 1, 0x05))
	uacSet(t, d, 1, 0x05, []byte{0x01})
	assert.Equal(t, []byte{0x01}, uacGet(t, d, 0xA1, 0x81, 1, 0x05))
}

// DS4Dongle parity: speaker powers up at -1dB, mic at 0dB; mute answers
// every GET with the mute byte; the channel number is ignored.
func TestUACDongleParity(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)

	// Power-up defaults: speaker -1dB (0xFF00), mic 0dB.
	assert.Equal(t, []byte{0x00, 0xFF}, uacGet(t, d, 0xA1, 0x81, 2, 0x02))
	assert.Equal(t, []byte{0x00, 0x00}, uacGet(t, d, 0xA1, 0x81, 2, 0x05))

	// Speaker ranges mirror the real DS4 v2.
	assert.Equal(t, []byte{0x00, 0xB7}, uacGet(t, d, 0xA1, 0x82, 2, 0x02))
	assert.Equal(t, []byte{0x00, 0xFF}, uacGet(t, d, 0xA1, 0x83, 2, 0x02))
	assert.Equal(t, []byte{0x00, 0x01}, uacGet(t, d, 0xA1, 0x84, 2, 0x02))
	// Mic ranges mirror the real DS4 v2.
	assert.Equal(t, []byte{0xC0, 0xE8}, uacGet(t, d, 0xA1, 0x82, 2, 0x05))
	assert.Equal(t, []byte{0x00, 0x18}, uacGet(t, d, 0xA1, 0x83, 2, 0x05))
	assert.Equal(t, []byte{0xC0, 0x00}, uacGet(t, d, 0xA1, 0x84, 2, 0x05))

	// Mute answers GET_MIN/MAX/RES with the mute byte, like GET_CUR.
	uacSet(t, d, 1, 0x02, []byte{0x01})
	for _, req := range []uint8{0x81, 0x82, 0x83, 0x84} {
		assert.Equal(t, []byte{0x01}, uacGet(t, d, 0xA1, req, 1, 0x02),
			"req=0x%02X", req)
	}

	// Channel number is ignored: channel 1 SET/GET hits the master state.
	_, handled := d.HandleControl(0x21, 0x01, 0x0101, 0x0500, 1, []byte{0x01})
	require.True(t, handled)
	assert.Equal(t, []byte{0x01}, uacGet(t, d, 0xA1, 0x81, 1, 0x05))

	// SET_CUR round-trips exactly through GET_CUR (Linux sticky-volume
	// probe requires the readback to match).
	uacSet(t, d, 2, 0x02, []byte{0x34, 0x12})
	assert.Equal(t, []byte{0x34, 0x12}, uacGet(t, d, 0xA1, 0x81, 2, 0x02))
}

func TestUACUnknownEntityStalls(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)
	_, handled := d.HandleControl(0xA1, 0x81, 0x0100, 0x0700, 64, nil)
	assert.False(t, handled)
	// HID class traffic on the same bmRequestType bytes still falls through.
	_, handled = d.HandleControl(0x21, 0x09, 0x0205, 0x0000, 11, make([]byte, 11))
	assert.True(t, handled)
}

func TestIsochronousTransfers(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)
	ctx := context.Background()

	// Mic IN returns one silent 1ms frame (16 mono samples).
	got := d.HandleTransfer(ctx, 2, usbip.DirIn, nil)
	require.Len(t, got, 32)
	for _, b := range got {
		assert.Zero(t, b)
	}

	// Speaker OUT is absorbed.
	assert.Nil(t, d.HandleTransfer(ctx, 1, usbip.DirOut, make([]byte, 132)))
}

func TestSpeakerSubscriptionExactBytes(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)
	ctx := context.Background()

	ch, unsub := d.SubscribeSpeaker()
	defer unsub()

	frame := make([]byte, 132)
	for i := range frame {
		frame[i] = byte(i)
	}
	// HandleTransfer takes caller scratch: mutate afterwards to prove copy.
	d.HandleTransfer(ctx, 1, usbip.DirOut, frame)
	for i := range frame {
		frame[i] = 0xFF
	}

	select {
	case ev := <-ch:
		assert.False(t, ev.Reset)
		require.Len(t, ev.PCM, 132)
		for i := range ev.PCM {
			assert.Equal(t, byte(i), ev.PCM[i])
		}
	default:
		t.Fatal("no speaker frame delivered")
	}
}

func TestSpeakerSubscriptionSlowDropsOldest(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)
	ctx := context.Background()

	ch, unsub := d.SubscribeSpeaker()
	defer unsub()

	// Overflow the 32-frame buffer without draining.
	for i := 0; i < 40; i++ {
		d.HandleTransfer(ctx, 1, usbip.DirOut, []byte{byte(i), 0xAA})
	}
	// Freshest frames survive; count what remains.
	var last byte
	n := 0
drain:
	for {
		select {
		case ev := <-ch:
			last = ev.PCM[0]
			n++
		default:
			break drain
		}
	}
	assert.Equal(t, 32, n)
	assert.Equal(t, byte(39), last)
}

func TestSpeakerUnsubscribeStops(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)
	ctx := context.Background()

	ch, unsub := d.SubscribeSpeaker()
	unsub()
	unsub() // idempotent
	d.HandleTransfer(ctx, 1, usbip.DirOut, []byte{0x01})
	// Closed channel: receives zero value with ok=false, never a frame.
	_, ok := <-ch
	assert.False(t, ok)
}

func TestResetBarrierOnAltChange(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)

	ch, unsub := d.SubscribeSpeaker()
	defer unsub()

	// No barrier at init.
	select {
	case <-ch:
		t.Fatal("barrier at init")
	default:
	}

	// Speaker alt 0 -> 1 fires exactly one barrier.
	_, handled := d.HandleControl(0x01, 0x0B, 1, 1, 0, nil)
	require.True(t, handled)
	select {
	case ev := <-ch:
		assert.True(t, ev.Reset)
	default:
		t.Fatal("no barrier on speaker alt change")
	}

	// Same value: no barrier.
	_, handled = d.HandleControl(0x01, 0x0B, 1, 1, 0, nil)
	require.True(t, handled)
	select {
	case <-ch:
		t.Fatal("barrier without change")
	default:
	}

	// Mic alt 0 -> 1 fires as well.
	_, handled = d.HandleControl(0x01, 0x0B, 1, 2, 0, nil)
	require.True(t, handled)
	select {
	case ev := <-ch:
		assert.True(t, ev.Reset)
	default:
		t.Fatal("no barrier on mic alt change")
	}

	// Non-audio interface: no barrier.
	_, handled = d.HandleControl(0x01, 0x0B, 1, 3, 0, nil)
	require.True(t, handled)
	select {
	case <-ch:
		t.Fatal("barrier on HID alt change")
	default:
	}
}

// Pipe-level: TCP audio ingest serves framed speaker PCM from EP1 OUT.
func TestAudioStream_ServesFrames(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)
	ctx := context.Background()

	server, client := net.Pipe()
	done := make(chan error, 1)
	go func() {
		done <- AudioStreamHandler(d, slog.Default())(server)
	}()

	frame := make([]byte, 132)
	for i := range frame {
		frame[i] = byte(i + 1)
	}
	// The handler subscribes asynchronously; fire until its subscription
	// exists (pre-subscription frames are absorbed). Surviving frames are
	// identical, so any delivered frame matches.
	for i := 0; i < 50; i++ {
		d.HandleTransfer(ctx, 1, usbip.DirOut, frame)
		time.Sleep(10 * time.Millisecond)
	}
	require.NoError(t, client.SetReadDeadline(time.Now().Add(5*time.Second)))
	var hdr [2]byte
	_, err = io.ReadFull(client, hdr[:])
	require.NoError(t, err)
	n := binary.LittleEndian.Uint16(hdr[:])
	require.Equal(t, uint16(132), n)
	got := make([]byte, n)
	_, err = io.ReadFull(client, got)
	require.NoError(t, err)
	require.Len(t, got, 132)
	assert.Equal(t, frame, got)

	// Closing the client plus one wake-up frame ends the handler: the
	// write to the closed pipe fails.
	require.NoError(t, client.Close())
	d.HandleTransfer(ctx, 1, usbip.DirOut, frame)
	select {
	case err := <-done:
		assert.Error(t, err)
	case <-time.After(2 * time.Second):
		t.Fatal("audio handler did not end after close")
	}
}

// openMic selects IF2 alt 1 (SET_INTERFACE), as a host starting capture does.
func openMic(t *testing.T, d *DualShock4) {
	t.Helper()
	_, handled := d.HandleControl(0x01, 0x0B, 1, 2, 0, nil)
	require.True(t, handled)
}

func TestMicrophoneQueueExactSize(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)

	assert.False(t, d.QueueMicrophonePCM(nil))
	assert.False(t, d.QueueMicrophonePCM(make([]byte, 31)))
	assert.False(t, d.QueueMicrophonePCM(make([]byte, 33)))

	// Gated while the host has not opened the mic interface.
	assert.False(t, d.QueueMicrophonePCM(make([]byte, 32)))

	openMic(t, d)
	frame := make([]byte, 32)
	for i := range frame {
		frame[i] = byte(i)
	}
	assert.True(t, d.QueueMicrophonePCM(frame))
	frame[0] = 0xFF // caller scratch: queue copied
	assert.Equal(t, byte(0), d.popMicrophoneFrame()[0])
}

func TestMicrophoneServeOrderAndUnderrun(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)
	ctx := context.Background()
	openMic(t, d)

	// Empty queue serves silence.
	got := d.HandleTransfer(ctx, 2, usbip.DirIn, nil)
	require.Len(t, got, 32)
	for _, b := range got {
		assert.Zero(t, b)
	}

	first := make([]byte, 32)
	second := make([]byte, 32)
	for i := range first {
		first[i] = 0x11
		second[i] = 0x22
	}
	require.True(t, d.QueueMicrophonePCM(first))
	require.True(t, d.QueueMicrophonePCM(second))

	assert.Equal(t, first, d.HandleTransfer(ctx, 2, usbip.DirIn, nil))
	assert.Equal(t, second, d.HandleTransfer(ctx, 2, usbip.DirIn, nil))

	// Drained queue is silence again.
	got = d.HandleTransfer(ctx, 2, usbip.DirIn, nil)
	for _, b := range got {
		assert.Zero(t, b)
	}
}

func TestMicrophoneQueueDropsOldest(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)
	ctx := context.Background()
	openMic(t, d)

	for i := 0; i < 40; i++ {
		frame := make([]byte, 32)
		frame[0] = byte(i)
		require.True(t, d.QueueMicrophonePCM(frame))
	}
	// 32 newest survive: first served is frame 8.
	got := d.HandleTransfer(ctx, 2, usbip.DirIn, nil)
	assert.Equal(t, byte(8), got[0])
}

// Closing the mic interface flushes queued frames and gates ingest until
// reopen: a reopen never replays stale PCM.
func TestMicrophoneFlushOnAltChange(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)
	ctx := context.Background()

	// Closed mic rejects ingest.
	closed := make([]byte, 32)
	closed[0] = 0xAA
	assert.False(t, d.QueueMicrophonePCM(closed))

	openMic(t, d)
	first := make([]byte, 32)
	first[0] = 0x11
	second := make([]byte, 32)
	second[0] = 0x22
	require.True(t, d.QueueMicrophonePCM(first))
	require.True(t, d.QueueMicrophonePCM(second))

	// Close (alt 1 -> 0): queue flushed, ingest gated again.
	_, handled := d.HandleControl(0x01, 0x0B, 0, 2, 0, nil)
	require.True(t, handled)
	assert.False(t, d.QueueMicrophonePCM(closed))
	assert.Nil(t, d.popMicrophoneFrame())

	// EP2 serves silence, not stale frames.
	got := d.HandleTransfer(ctx, 2, usbip.DirIn, nil)
	for _, b := range got {
		assert.Zero(t, b)
	}

	// Reopen: fresh frames flow, nothing stale.
	openMic(t, d)
	third := make([]byte, 32)
	third[0] = 0x33
	require.True(t, d.QueueMicrophonePCM(third))
	assert.Equal(t, third, d.HandleTransfer(ctx, 2, usbip.DirIn, nil))
}
