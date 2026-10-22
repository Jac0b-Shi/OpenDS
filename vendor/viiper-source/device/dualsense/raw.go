package dualsense

import (
	"io"
	"log/slog"
	"net"
)

// rawSnapshot is one published 64B USB input report. Snapshots are
// immutable once published, so the interrupt path can hand out the slice
// without copying or locking: SetRawInputReport always allocates a fresh
// one instead of mutating in place.
type rawSnapshot [InputReportSize]byte

// SetRawInputReport pipes the exact
// 64B USB input report 0x01 (report ID + 63B payload, e.g. captured from
// hardware or DS5Dongle passthrough in main.cpp). While set, interrupt IN
// and GET_REPORT input serve the raw bytes verbatim (including the real
// seq/counter/timestamps/touch/trigger-echo/headset/AES fields); when unset
// the synthetic builder runs. Separate calls keep old clients working:
// never setting raw preserves today's behavior.
//
// While raw is set the synthetic path stops running, so its internal
// sequence counter and sensor timestamp freeze. Feeder states pushed via
// UpdateInputState are still buffered but not reported; clearing raw makes
// the synthetic builder resume from its current (stale) counters.
func (d *DualSense) SetRawInputReport(raw []byte) bool {
	if len(raw) != InputReportSize || raw[0] != ReportIDInput {
		return false
	}
	snap := &rawSnapshot{}
	copy(snap[:], raw)
	d.rawReport.Store(snap)
	select {
	case d.rawReady <- struct{}{}:
	default:
	}
	return true
}

// ClearRawInputReport drops the passthrough and restores synthetic reports.
func (d *DualSense) ClearRawInputReport() {
	d.rawReport.Store(nil)
	select {
	case d.rawReady <- struct{}{}:
	default:
	}
}

// rawInputReport returns the published snapshot, or nil when passthrough is
// unset. The returned slice is read-only and must not be modified.
func (d *DualSense) rawInputReport() []byte {
	snap := d.rawReport.Load()
	if snap == nil {
		return nil
	}
	return snap[:]
}

// RawStreamHandler ingests fixed 64B feeder raw reports over TCP for EP4
// IN. Short reads end the stream. Invalid frames are ignored and the
// stream stays open; closing the stream leaves the last raw in place
// (clear explicitly to restore synthetic).
func RawStreamHandler(dev *DualSense, logger *slog.Logger) func(conn net.Conn) error {
	return func(conn net.Conn) error {
		logger.Debug("dualsense raw stream begin")
		defer logger.Debug("dualsense raw stream end")
		var frame [InputReportSize]byte
		for {
			if _, err := io.ReadFull(conn, frame[:]); err != nil {
				return err
			}
			dev.SetRawInputReport(frame[:])
		}
	}
}
