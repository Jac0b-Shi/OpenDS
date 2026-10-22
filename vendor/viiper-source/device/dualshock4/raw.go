package dualshock4

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
// 64B USB input report 0x01 (report ID + 63B payload, matching DS4Dongle's
// BT 0x11 payload copy in main.cpp on_bt_data). While set, interrupt IN
// and GET_REPORT input serve the raw bytes verbatim (real counter,
// timestamp, touch, battery); when unset the synthetic builder runs.
// Separate calls keep old clients working.
//
// While raw is set the synthetic path stops running, so its report counter
// and sensor timestamp freeze. Feeder states pushed via UpdateInputState
// are still buffered but not reported; clearing raw resumes the synthetic
// builder from its current (stale) counters.
func (d *DualShock4) SetRawInputReport(raw []byte) bool {
	if len(raw) != InputReportSize || raw[0] != ReportIDInput {
		return false
	}
	snap := &rawSnapshot{}
	copy(snap[:], raw)
	d.rawReport.Store(snap)
	return true
}

// ClearRawInputReport drops the passthrough and restores synthetic reports.
func (d *DualShock4) ClearRawInputReport() {
	d.rawReport.Store(nil)
}

// rawInputReport returns the published snapshot, or nil when passthrough is
// unset. The returned slice is read-only and must not be modified.
func (d *DualShock4) rawInputReport() []byte {
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
func RawStreamHandler(dev *DualShock4, logger *slog.Logger) func(conn net.Conn) error {
	return func(conn net.Conn) error {
		logger.Debug("dualshock4 raw stream begin")
		defer logger.Debug("dualshock4 raw stream end")
		var frame [InputReportSize]byte
		for {
			if _, err := io.ReadFull(conn, frame[:]); err != nil {
				return err
			}
			dev.SetRawInputReport(frame[:])
		}
	}
}
