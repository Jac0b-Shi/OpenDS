package dualshock4

import (
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// Touch packet counters increment per report while active and revert to
// 0x80 when the finger is released. The increment follows community DS4
// captures rather than DS4Dongle, which forwards controller bytes verbatim
// and documents no counter semantics.
func TestTouchCountersIncrementWhileActive(t *testing.T) {
	d, err := New(nil)
	require.NoError(t, err)
	s := NewInputState()
	s.Touch1Active = true
	s.Touch1X, s.Touch1Y = 100, 200
	m := *d.metaState

	a := d.buildUSBInputReport(s, &m)
	b := d.buildUSBInputReport(s, &m)
	require.NotEqual(t, uint8(0x80), a[35])
	assert.Equal(t, (a[35]+1)&0x7F, b[35]&0x7F)

	// Released: inactive mask set again.
	s.Touch1Active = false
	c := d.buildUSBInputReport(s, &m)
	assert.Equal(t, uint8(0x80), c[35])
	assert.Equal(t, uint8(0x80), c[39])
}
