package viiperclient

import (
	"context"
	"fmt"
	"io"
	"net"
	"time"
)

// RawInputFrameSize is the exact 64B USB input report framing
// (route bus/{busId}/{deviceid}/raw, feeder to
// server): exact 64B USB input reports 0x01 back-to-back. The core serves
// them verbatim while set and synthesizes otherwise; never setting raw
// preserves today's behavior.
const RawInputFrameSize = 64

// RawStream is a feeder connection pushing raw 64B input reports into a device.
type RawStream struct {
	conn   net.Conn
	BusID  uint32
	DevID  string
	closed bool
}

// OpenRawStream connects to an existing device's raw-input ingest stream.
// Fails for device types without raw input (server closes with "not supported").
func (c *Client) OpenRawStream(ctx context.Context, busID uint32, devID string) (*RawStream, error) {
	conn, err := c.dialStream(ctx, fmt.Sprintf("bus/%d/%s/raw\x00", busID, devID))
	if err != nil {
		return nil, err
	}
	return &RawStream{conn: conn, BusID: busID, DevID: devID}, nil
}

// WriteFrame writes one exact 64B raw input report.
func (s *RawStream) WriteFrame(frame []byte) error {
	if s.closed {
		return fmt.Errorf("stream closed")
	}
	if len(frame) != RawInputFrameSize {
		return fmt.Errorf("raw frame must be exactly %d bytes, got %d", RawInputFrameSize, len(frame))
	}
	_, err := s.conn.Write(frame)
	return err
}

// SetReadDeadline sets the read deadline for the underlying connection. A
// read is the way to detect the server closing the route (for example an
// unsupported device type), so expose it like the audio/mic streams.
func (s *RawStream) SetReadDeadline(t time.Time) error {
	return s.conn.SetReadDeadline(t)
}

// Read receives bytes from the underlying connection. Normally unused: the
// route is feeder-to-server only, and reads surface server-side closes.
func (s *RawStream) Read(buf []byte) (int, error) {
	if s.closed {
		return 0, fmt.Errorf("stream closed")
	}
	return s.conn.Read(buf)
}

var _ io.Reader = (*RawStream)(nil)

// Close closes the raw stream connection.
func (s *RawStream) Close() error {
	if s.closed {
		return nil
	}
	s.closed = true
	return s.conn.Close()
}
