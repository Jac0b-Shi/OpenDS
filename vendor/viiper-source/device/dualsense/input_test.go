package dualsense

import (
	"context"
	"testing"

	"github.com/DualSenseClient/VIIPER/usbip"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func neutralReport(t *testing.T, d *DualSense) []byte {
	t.Helper()
	d.mtx.Lock()
	is := *d.inputState
	ms := *d.metaState
	d.mtx.Unlock()
	b := d.buildUSBInputReport(&is, &ms)
	require.Len(t, b, InputReportSize)
	return b
}

func TestInputReportBasics(t *testing.T) {
	d, err := new(nil, false)
	require.NoError(t, err)
	b := neutralReport(t, d)
	assert.Equal(t, uint8(ReportIDInput), b[0])
	// Neutral sticks center at 128.
	assert.Equal(t, uint8(128), b[1])
	assert.Equal(t, uint8(128), b[2])
	// Neutral hat, no buttons.
	assert.Equal(t, uint8(DPadUSBNeutral), b[8]&DPadMask)
	assert.Equal(t, uint8(0), b[9])
	assert.Equal(t, uint8(0), b[10])
	// Untouched contacts carry the inactive mask.
	assert.Equal(t, TouchInactiveMask, b[33])
	assert.Equal(t, TouchInactiveMask, b[37])
	// Battery status from meta.
	assert.Equal(t, uint8(DefaultBatteryStatus), b[53])
}

func TestInputReportButtonsAndTriggers(t *testing.T) {
	d, err := new(nil, false)
	require.NoError(t, err)
	s := NewInputState()
	s.Buttons = ButtonCross | ButtonL1 | ButtonPS
	s.DPad = DPadUp
	s.L2 = 0xFF
	d.UpdateInputState(s)
	b := neutralReport(t, d)
	assert.Equal(t, uint8(0xFF), b[5])
	assert.Equal(t, uint8(DPadUSBUp), b[8]&DPadMask)
	assert.NotZero(t, b[9])
	assert.NotZero(t, b[10])
}

func TestInputReportTouchTracking(t *testing.T) {
	d, err := new(nil, false)
	require.NoError(t, err)

	// Feeder-supplied tracking IDs flow verbatim into the contact bytes.
	s := NewInputState()
	s.Touch1Active = true
	s.Touch1Tracking = 0x2A
	s.Touch1X, s.Touch1Y = 100, 200
	d.UpdateInputState(s)
	b := neutralReport(t, d)
	assert.Equal(t, uint8(0x2A), b[33])
	assert.Equal(t, uint8(100), b[34])

	// Release keeps the supplied ID under the inactive mask.
	s.Touch1Active = false
	d.UpdateInputState(s)
	b = neutralReport(t, d)
	assert.Equal(t, TouchInactiveMask|0x2A, b[33])

	// Untouched second finger stays at never-touched 0x80.
	assert.Equal(t, TouchInactiveMask, b[37])

	// Tracking survives the wire round-trip.
	wire, err := s.MarshalBinary()
	require.NoError(t, err)
	require.Len(t, wire, InputStateSize)
	var back InputState
	require.NoError(t, back.UnmarshalBinary(wire))
	assert.Equal(t, uint8(0x2A), back.Touch1Tracking)
}

func TestInputReportSeqAdvances(t *testing.T) {
	d, err := new(nil, false)
	require.NoError(t, err)
	a := neutralReport(t, d)
	b := neutralReport(t, d)
	assert.Equal(t, a[7]+1, b[7])
}

func TestInputReportTemperatureByte(t *testing.T) {
	d, err := new(nil, false)
	require.NoError(t, err)
	// Default 28C from metadata lands in b[32] as int8.
	assert.Equal(t, uint8(28), neutralReport(t, d)[32])

	d.MergeMetaState(MetaState{TemperatureCelsius: -5})
	assert.Equal(t, uint8(0xFB), neutralReport(t, d)[32])
}

func TestRawInputPassthroughPrecedence(t *testing.T) {
	d, err := new(nil, false)
	require.NoError(t, err)
	ctx := context.Background()

	// Invalid raws rejected, synthetic still served.
	assert.False(t, d.SetRawInputReport(nil))
	assert.False(t, d.SetRawInputReport(make([]byte, 10)))
	bad := make([]byte, InputReportSize)
	bad[0] = 0x02
	assert.False(t, d.SetRawInputReport(bad))
	assert.Equal(t, uint8(ReportIDInput), neutralReport(t, d)[0])

	// Valid raw served verbatim on interrupt and GET_REPORT.
	raw := make([]byte, InputReportSize)
	raw[0] = ReportIDInput
	for i := 1; i < InputReportSize; i++ {
		raw[i] = byte(i)
	}
	require.True(t, d.SetRawInputReport(raw))
	raw[1] = 0xFF // caller scratch: core copied
	got := d.HandleTransfer(ctx, 4, usbip.DirIn, nil)
	require.Len(t, got, InputReportSize)
	assert.Equal(t, byte(0x01), got[1])
	b, handled := d.HandleControl(hidClassIN, hidGetReport,
		uint16(reportTypeInput)<<8|uint16(ReportIDInput), 0, 64, nil)
	require.True(t, handled)
	assert.Equal(t, got, b)

	// Clear restores synthetic.
	d.ClearRawInputReport()
	assert.Equal(t, uint8(128), neutralReport(t, d)[1])
}

// The interrupt path hands out the published snapshot without allocating:
// it runs at up to 1kHz, so a copy per poll would be measurable.
func TestRawInputReadPathDoesNotAllocate(t *testing.T) {
	d, err := new(nil, false)
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

// While raw is set the synthetic sequence counter freezes; clearing raw
// resumes synthetic reporting from that stale counter.
func TestRawPassthroughFreezesSyntheticSeq(t *testing.T) {
	d, err := new(nil, false)
	require.NoError(t, err)
	ctx := context.Background()
	before := neutralReport(t, d)[7]
	raw := make([]byte, InputReportSize)
	raw[0] = ReportIDInput
	raw[7] = 0x5A
	require.True(t, d.SetRawInputReport(raw))
	// Two polls, same frozen seq, and the raw byte is what is served.
	assert.Equal(t, byte(0x5A), d.HandleTransfer(ctx, 4, usbip.DirIn, nil)[7])
	assert.Equal(t, byte(0x5A), d.HandleTransfer(ctx, 4, usbip.DirIn, nil)[7])
	d.ClearRawInputReport()
	assert.Equal(t, before+1, neutralReport(t, d)[7])
}

// The input report echoes the host-driven mute LED in b[54] bit 2,
// synthesized from the last output report (the dongle forwards the
// controller bit; the virtual device has no backing controller).
func TestInputReportMuteLightEcho(t *testing.T) {
	d, err := new(nil, false)
	require.NoError(t, err)
	ctx := context.Background()

	// 48B output report with Flags1 + MuteLightMode at out[2]/out[9].
	output := func(flags1, mode uint8) []byte {
		out := make([]byte, 48)
		out[0] = ReportIDOutput
		out[2] = flags1
		out[9] = mode
		return out
	}

	// Default off.
	assert.Zero(t, neutralReport(t, d)[54]&0x04)

	// Without AllowMuteLight the mode byte is ignored.
	d.HandleTransfer(ctx, 3, usbip.DirOut, output(0x00, MuteLightOn))
	assert.Zero(t, neutralReport(t, d)[54]&0x04)

	// AllowMuteLight + On lights the bit; Breathing keeps it; Off clears.
	d.HandleTransfer(ctx, 3, usbip.DirOut, output(Flag1AllowMuteLight, MuteLightOn))
	assert.Equal(t, uint8(0x04), neutralReport(t, d)[54]&0x04)
	d.HandleTransfer(ctx, 3, usbip.DirOut, output(Flag1AllowMuteLight, MuteLightBreathing))
	assert.Equal(t, uint8(0x04), neutralReport(t, d)[54]&0x04)
	d.HandleTransfer(ctx, 3, usbip.DirOut, output(Flag1AllowMuteLight, MuteLightOff))
	assert.Zero(t, neutralReport(t, d)[54]&0x04)

	// DoNothing leaves the previous state (on here).
	d.HandleTransfer(ctx, 3, usbip.DirOut, output(Flag1AllowMuteLight, MuteLightOn))
	d.HandleTransfer(ctx, 3, usbip.DirOut, output(Flag1AllowMuteLight, MuteLightDoNothing))
	assert.Equal(t, uint8(0x04), neutralReport(t, d)[54]&0x04)

	// The control-path SET_REPORT output feeds the same synthesis.
	d.HandleTransfer(ctx, 3, usbip.DirOut, output(Flag1AllowMuteLight, MuteLightOff))
	assert.Zero(t, neutralReport(t, d)[54]&0x04)
	_, handled := d.HandleControl(0x21, 0x09, 0x0202, 0x0003, 48,
		output(Flag1AllowMuteLight, MuteLightOn))
	require.True(t, handled)
	assert.Equal(t, uint8(0x04), neutralReport(t, d)[54]&0x04)
}
