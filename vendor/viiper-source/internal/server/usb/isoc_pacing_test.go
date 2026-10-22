package usb

import (
	"context"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// The first reservation must be served immediately (no artificial delay when
// the stream is idle) and anchor the following frame slots.
func TestIsocPacerIdleStartsImmediately(t *testing.T) {
	var p isocPacer
	start := time.Now()
	require.True(t, p.wait(context.Background(), 4))
	assert.Less(t, time.Since(start), 50*time.Millisecond, "idle URB must not be delayed")
}

// Consecutive reservations are spaced one USB frame per packet: a burst of
// one-packet URBs takes one millisecond each, so the host cannot consume
// audio faster than realtime.
func TestIsocPacerSerializesFrames(t *testing.T) {
	var p isocPacer
	require.True(t, p.wait(context.Background(), 1)) // anchor, immediate

	start := time.Now()
	for i := 0; i < 5; i++ {
		require.True(t, p.wait(context.Background(), 1))
	}
	elapsed := time.Since(start)
	assert.GreaterOrEqual(t, elapsed, 4*time.Millisecond, "5 frames must take ~5ms")
	assert.Less(t, elapsed, time.Second, "pacing must not run away")
}

// A URB's duration scales with its packet count (one frame per packet).
func TestIsocPacerScalesWithPacketCount(t *testing.T) {
	var p isocPacer
	require.True(t, p.wait(context.Background(), 8)) // anchor

	start := time.Now()
	require.True(t, p.wait(context.Background(), 8))
	assert.GreaterOrEqual(t, time.Since(start), 7*time.Millisecond, "8 packets occupy 8 frames")
}

// After an idle gap the credit is clamped: a burst following a pause is
// served immediately rather than replaying the missed frames.
func TestIsocPacerClampsAfterIdle(t *testing.T) {
	var p isocPacer
	require.True(t, p.wait(context.Background(), 1))

	time.Sleep(20 * time.Millisecond)
	start := time.Now()
	require.True(t, p.wait(context.Background(), 1))
	assert.Less(t, time.Since(start), 5*time.Millisecond, "idle gap must not accrue a backlog")
}

// Cancellation (UNLINK / device removal) must release a waiting URB.
func TestIsocPacerCancelsOnContext(t *testing.T) {
	var p isocPacer
	require.True(t, p.wait(context.Background(), 1)) // anchor one frame ahead

	ctx, cancel := context.WithCancel(context.Background())
	go func() {
		time.Sleep(10 * time.Millisecond)
		cancel()
	}()
	start := time.Now()
	assert.False(t, p.wait(ctx, 100), "cancelled wait must report false")
	assert.Less(t, time.Since(start), 500*time.Millisecond, "cancellation must release promptly")
}
