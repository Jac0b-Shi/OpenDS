package dualshock4

import (
	"context"
	"log/slog"
	"net"
	"testing"
	"time"

	"github.com/DualSenseClient/VIIPER/usbip"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestRawInputPassthroughPrecedence(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)

	assert.False(t, d.SetRawInputReport(nil))
	assert.False(t, d.SetRawInputReport(make([]byte, 10)))
	bad := make([]byte, InputReportSize)
	bad[0] = ReportIDOutput
	assert.False(t, d.SetRawInputReport(bad))

	raw := make([]byte, InputReportSize)
	raw[0] = ReportIDInput
	for i := 1; i < InputReportSize; i++ {
		raw[i] = byte(i)
	}
	require.True(t, d.SetRawInputReport(raw))
	raw[1] = 0xFF // caller scratch: core copied
	got := d.HandleTransfer(context.Background(), 4, usbip.DirIn, nil)
	require.Len(t, got, InputReportSize)
	assert.Equal(t, byte(0x01), got[1])
	b, handled := d.HandleControl(hidClassIN, hidGetReport,
		uint16(reportTypeInput)<<8|uint16(ReportIDInput), 0, 64, nil)
	require.True(t, handled)
	assert.Equal(t, got, b)

	d.ClearRawInputReport()
	s := NewInputState()
	m := *d.metaState
	assert.Equal(t, uint8(0x80), d.buildUSBInputReport(s, &m)[1])
}

// The interrupt path hands out the published snapshot without allocating.
func TestRawInputReadPathDoesNotAllocate(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)
	raw := make([]byte, InputReportSize)
	raw[0] = ReportIDInput
	raw[1] = 0x7F
	require.True(t, d.SetRawInputReport(raw))
	ctx := context.Background()

	allocs := testing.AllocsPerRun(200, func() {
		got := d.HandleTransfer(ctx, 4, usbip.DirIn, nil)
		if len(got) != InputReportSize {
			t.Fatalf("bad length %d", len(got))
		}
	})
	assert.Zero(t, allocs)
}

func TestRawStreamIngestServesVerbatim(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)
	server, client := net.Pipe()
	defer server.Close()
	defer client.Close()
	done := make(chan error, 1)
	go func() {
		done <- RawStreamHandler(d, slog.Default())(server)
	}()
	raw := make([]byte, InputReportSize)
	raw[0] = ReportIDInput
	for i := 1; i < InputReportSize; i++ {
		raw[i] = byte(0xC0 + i)
	}
	_, err = client.Write(raw)
	require.NoError(t, err)
	require.Eventually(t, func() bool {
		return d.HandleTransfer(context.Background(), 4, usbip.DirIn, nil)[1] == raw[1]
	}, 2*time.Second, 10*time.Millisecond)
	client.Close()
	<-done
}
