package dualsense

import (
	"context"
	"encoding/binary"
	"math"
	"testing"

	"github.com/DualSenseClient/VIIPER/usbip"
	"github.com/stretchr/testify/require"
)

// Simulate source and USB clocks without sleeping or depending on OS timers.
// A clear continuous source must not acquire silent 1ms holes at the USB end.
func TestMicrophoneBurstJitterAndClockSkew(t *testing.T) {
	for _, skew := range []float64{0.97, 1, 1.03} {
		t.Run(fmtSkew(skew), func(t *testing.T) {
			d, err := new(nil, false)
			require.NoError(t, err)
			openMic(t, d)
			var next float64
			var sample int
			var previous int16
			maxJump, silence := 0, 0
			for ms := 0; ms < 30000; ms++ {
				// 10ms decoded packets; every half second, hold one packet
				// for 70ms then deliver the backlog, as a BT scheduler can.
				held := ms%500 >= 400 && ms%500 < 470
				if !held {
					for float64(ms) >= next {
						for frame := 0; frame < 10; frame++ {
							pcm := make([]byte, 192)
							for s := 0; s < 48; s++ {
								v := int16(12000 * math.Sin(2*math.Pi*1000*float64(sample)/48000))
								binary.LittleEndian.PutUint16(pcm[s*4:], uint16(v))
								binary.LittleEndian.PutUint16(pcm[s*4+2:], uint16(-v))
								sample++
							}
							require.True(t, d.QueueMicrophonePCM(pcm))
						}
						next += 10 / skew
					}
				}
				got := d.HandleTransfer(context.Background(), 2, usbip.DirIn, nil)
				require.Len(t, got, 192)
				if ms < 250 {
					continue
				} // Initial pre-roll is intentionally silent.
				nonzero := false
				for s := 0; s < 48; s++ {
					v := int16(binary.LittleEndian.Uint16(got[s*4:]))
					r := int16(binary.LittleEndian.Uint16(got[s*4+2:]))
					require.InDelta(t, -int(v), int(r), 1)
					nonzero = nonzero || v != 0
					if ms > 250 {
						jump := int(math.Abs(float64(int(v) - int(previous))))
						maxJump = max(maxJump, jump)
					}
					previous = v
				}
				if !nonzero {
					silence++
				}
			}
			require.Zero(t, silence, "continuous source acquired silent USB frames")
			require.Less(t, maxJump, 2000, "discontinuous splice in a 1kHz tone")
		})
	}
}

func fmtSkew(skew float64) string {
	if skew < 1 {
		return "source_slow_3_percent"
	}
	if skew > 1 {
		return "source_fast_3_percent"
	}
	return "equal_clocks"
}

func TestMicrophoneBufferConfigurationAndPersistentCounters(t *testing.T) {
	d, err := new(nil, false)
	require.NoError(t, err)
	require.False(t, d.SetMicrophoneBufferMs(15))
	require.False(t, d.SetMicrophoneBufferMs(513))
	require.True(t, d.SetMicrophoneBufferMs(160))
	openMic(t, d)
	frame := make([]byte, 192)
	for i := 0; i < 48; i++ {
		binary.LittleEndian.PutUint16(frame[i*4:], 500)
		binary.LittleEndian.PutUint16(frame[i*4+2:], 500)
	}
	for i := 0; i < 159; i++ {
		require.True(t, d.QueueMicrophonePCM(frame))
	}
	require.Nil(t, d.popMicrophoneFrame())
	require.True(t, d.QueueMicrophonePCM(frame))
	require.Equal(t, frame, d.popMicrophoneFrame())
	for i := 0; i < 400; i++ {
		d.popMicrophoneFrame()
	}
	stats := d.MicrophoneBufferStats()
	require.Equal(t, uint64(1), stats[1])
	for i := 0; i < micQueueDepth+8; i++ {
		require.True(t, d.QueueMicrophonePCM(frame))
	}
	require.Positive(t, d.MicrophoneBufferStats()[2])
	d.setAltSetting(2, 0)
	closed := d.MicrophoneBufferStats()
	require.Zero(t, closed[0])
	require.Equal(t, uint64(1), closed[1])
	require.Positive(t, closed[2])
	require.Nil(t, d.micBuffer)
	require.True(t, d.SetMicrophoneBufferMs(192))
	require.Equal(t, closed, d.MicrophoneBufferStats())
	openMic(t, d)
	require.True(t, d.SetMicrophoneBufferMs(512))
	for i := 0; i < 511; i++ {
		require.True(t, d.QueueMicrophonePCM(frame))
	}
	require.Nil(t, d.popMicrophoneFrame())
	require.True(t, d.QueueMicrophonePCM(frame))
	require.Equal(t, frame, d.popMicrophoneFrame())
}
