package viiperclient

import (
	"context"
	"encoding/binary"
	"fmt"
	"io"
	"net"
	"time"
)

// Audio stream framing (route bus/{busId}/{deviceid}/audio, server to
// feeder): u16 LE length N followed by N bytes of speaker PCM (exact
// host-written EP1 OUT payloads, 4ch S16LE @48kHz, up to 392B).
// N == AudioResetSentinel carries a speaker-reset barrier event with no
// payload (emitted once per streaming generation).
const (
	AudioMaxFrame      = 392
	AudioResetSentinel = 0xFFFF
)

// AudioStream is a feeder connection to a device's speaker PCM stream.
type AudioStream struct {
	conn   net.Conn
	BusID  uint32
	DevID  string
	closed bool
}

// OpenAudioStream connects to an existing device's speaker PCM stream.
// Fails for device types without audio (server closes with "not supported").
func (c *Client) OpenAudioStream(ctx context.Context, busID uint32, devID string) (*AudioStream, error) {
	conn, err := c.dialStream(ctx, fmt.Sprintf("bus/%d/%s/audio\x00", busID, devID))
	if err != nil {
		return nil, err
	}
	return &AudioStream{conn: conn, BusID: busID, DevID: devID}, nil
}

// OpenHapticsStream connects to an existing device's rear-haptics PCM
// stream (2ch S16LE @48kHz, same framing as audio, barriers included).
func (c *Client) OpenHapticsStream(ctx context.Context, busID uint32, devID string) (*AudioStream, error) {
	conn, err := c.dialStream(ctx, fmt.Sprintf("bus/%d/%s/audio/haptics\x00", busID, devID))
	if err != nil {
		return nil, err
	}
	return &AudioStream{conn: conn, BusID: busID, DevID: devID}, nil
}

// OpenMicStream connects to an existing device's microphone ingest stream.
// The feeder writes exact 192B PCM frames (2ch S16LE @48kHz).
func (c *Client) OpenMicStream(ctx context.Context, busID uint32, devID string) (*MicStream, error) {
	conn, err := c.dialStream(ctx, fmt.Sprintf("bus/%d/%s/audio/mic\x00", busID, devID))
	if err != nil {
		return nil, err
	}
	return &MicStream{conn: conn, BusID: busID, DevID: devID, frameSize: 192}, nil
}

// OpenDS4MicStream connects to an existing DualShock 4 microphone ingest
// stream. The feeder writes exact 32B PCM frames (mono S16LE @16kHz).
func (c *Client) OpenDS4MicStream(ctx context.Context, busID uint32, devID string) (*MicStream, error) {
	conn, err := c.dialStream(ctx, fmt.Sprintf("bus/%d/%s/audio/mic\x00", busID, devID))
	if err != nil {
		return nil, err
	}
	return &MicStream{conn: conn, BusID: busID, DevID: devID, frameSize: 32}, nil
}

// MicStream is a feeder connection pushing mic PCM into a device.
type MicStream struct {
	conn   net.Conn
	BusID  uint32
	DevID  string
	closed bool
	// frameSize is the exact PCM frame size in bytes (192 DualSense,
	// 32 DualShock 4).
	frameSize int
}

// WriteFrame writes one exact mic frame (see frameSize).
func (s *MicStream) WriteFrame(frame []byte) error {
	if s.closed {
		return fmt.Errorf("stream closed")
	}
	if len(frame) != s.frameSize {
		return fmt.Errorf("mic frame must be exactly %d bytes, got %d", s.frameSize, len(frame))
	}
	_, err := s.conn.Write(frame)
	return err
}

// SetReadDeadline sets the read deadline for the underlying connection.
func (s *MicStream) SetReadDeadline(t time.Time) error {
	return s.conn.SetReadDeadline(t)
}

// SetWriteDeadline sets the write deadline for the underlying connection.
func (s *MicStream) SetWriteDeadline(t time.Time) error {
	return s.conn.SetWriteDeadline(t)
}

// Close closes the mic stream connection.
func (s *MicStream) Close() error {
	if s.closed {
		return nil
	}
	s.closed = true
	return s.conn.Close()
}

// ReadFrame reads one speaker frame. reset is true for barrier events
// (pcm is nil then). Either a frame or a barrier is returned per call.
func (s *AudioStream) ReadFrame() (pcm []byte, reset bool, err error) {
	if s.closed {
		return nil, false, fmt.Errorf("stream closed")
	}
	var hdr [2]byte
	if _, err := io.ReadFull(s.conn, hdr[:]); err != nil {
		return nil, false, err
	}
	n := binary.LittleEndian.Uint16(hdr[:])
	if n == AudioResetSentinel {
		return nil, true, nil
	}
	if n > AudioMaxFrame {
		return nil, false, fmt.Errorf("speaker frame too large: %d", n)
	}
	pcm = make([]byte, n)
	if _, err := io.ReadFull(s.conn, pcm); err != nil {
		return nil, false, err
	}
	return pcm, false, nil
}

// SetReadDeadline sets the read deadline for the underlying connection.
func (s *AudioStream) SetReadDeadline(t time.Time) error {
	return s.conn.SetReadDeadline(t)
}

// Close closes the audio stream connection.
func (s *AudioStream) Close() error {
	if s.closed {
		return nil
	}
	s.closed = true
	return s.conn.Close()
}
