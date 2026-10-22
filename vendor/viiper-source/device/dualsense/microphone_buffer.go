package dualsense

import (
	"encoding/binary"
	"math"
)

const (
	micSamplesPerFrame = 48
	micPreRollFrames   = 128 // milliseconds, independent of speaker/haptics latency
	micQueueDepth      = 256
	micSampleCapacity  = micQueueDepth * micSamplesPerFrame
)

// microphoneBuffer bridges a bursty BT source clock to the USB frame clock.
// Protected by DualSense.mtx; never sleeps or waits for the producer. Fixed
// storage bounds latency and avoids per-enqueue allocations. The two channels
// retain independent samples, sharing only the fractional read position.
type microphoneBuffer struct {
	samples               [micSampleCapacity][2]int16
	head, count           int
	position, ratio, mean float64
	primed                bool
	underruns, dropped    uint64
}

func (b *microphoneBuffer) push(frame []byte) {
	if b.count+micSamplesPerFrame > micSampleCapacity {
		b.head = (b.head + micSamplesPerFrame) % micSampleCapacity
		b.count -= micSamplesPerFrame
		b.position = 0
		b.dropped += micSamplesPerFrame
	}
	for i := 0; i < micSamplesPerFrame; i++ {
		at := (b.head + b.count) % micSampleCapacity
		b.samples[at][0] = int16(binary.LittleEndian.Uint16(frame[i*4:]))
		b.samples[at][1] = int16(binary.LittleEndian.Uint16(frame[i*4+2:]))
		b.count++
	}
}

func (b *microphoneBuffer) pop() []byte {
	const target = float64(micPreRollFrames * micSamplesPerFrame)
	if !b.primed {
		if b.count < int(target) {
			return nil
		}
		b.primed, b.ratio, b.mean = true, 1, target
	}
	// Occupancy feedback handles independent clock rates, rather than
	// periodically inserting zero frames when a slightly slower source drains
	// even a large fixed FIFO. Smooth both the 10ms packet staircase and the
	// correction; bound time scaling to +/-5%, with no sample block splices.
	b.mean += (float64(b.count) - b.mean) * 0.02
	error := b.mean - target
	if math.Abs(error) < micSamplesPerFrame {
		error = 0
	}
	wanted := max(0.95, min(1.05, 1+error/(48000*0.5)))
	b.ratio += max(-0.00005, min(0.00005, wanted-b.ratio))
	need := int(math.Ceil(b.position+micSamplesPerFrame*b.ratio)) + 1
	if b.count < need {
		b.primed = false
		b.position = 0
		b.underruns++
		return nil // Re-prime once, instead of alternating tiny PCM/zero bursts.
	}
	frame := make([]byte, micFrameSize)
	for i := 0; i < micSamplesPerFrame; i++ {
		index := int(b.position)
		fraction := b.position - float64(index)
		a := b.samples[(b.head+index)%micSampleCapacity]
		z := b.samples[(b.head+index+1)%micSampleCapacity]
		for ch := 0; ch < 2; ch++ {
			value := math.Round(float64(a[ch]) + (float64(z[ch])-float64(a[ch]))*fraction)
			binary.LittleEndian.PutUint16(frame[i*4+ch*2:], uint16(int16(value)))
		}
		b.position += b.ratio
	}
	consumed := int(b.position)
	b.position -= float64(consumed)
	b.head = (b.head + consumed) % micSampleCapacity
	b.count -= consumed
	return frame
}
