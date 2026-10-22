package dualsense

import (
	"encoding/binary"
	"math"
)

// Caller holds d.mtx. Retain counters across host close/reopen, not PCM.
func (d *DualSense) flushMicrophoneBuffer() {
	if d.micBuffer != nil {
		d.micUnderruns += d.micBuffer.underruns
		d.micDropped += d.micBuffer.dropped
		d.micBuffer = nil
	}
}

func (d *DualSense) SetMicrophoneBufferMs(ms uint32) bool {
	if ms < 16 || ms > 512 {
		return false
	}
	d.mtx.Lock()
	defer d.mtx.Unlock()
	if d.micBufferMs != ms {
		d.flushMicrophoneBuffer()
		d.micBufferMs = ms
	}
	return true
}

// Queued sample frames, underrun events, dropped sample frames, rate in ppm.
func (d *DualSense) MicrophoneBufferStats() [4]uint64 {
	d.mtx.Lock()
	defer d.mtx.Unlock()
	stats := [4]uint64{0, d.micUnderruns, d.micDropped, 1000000}
	if b := d.micBuffer; b != nil {
		stats[0] = uint64(b.count)
		stats[1] += b.underruns
		stats[2] += b.dropped
		if b.ratio != 0 {
			stats[3] = uint64(math.Round(b.ratio * 1000000))
		}
	}
	return stats
}

const (
	micSamplesPerFrame = 48
	micPreRollFrames   = 128 // milliseconds, independent of speaker/haptics latency
	micQueueDepth      = 1024
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
	target                int
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
	target := float64(b.target)
	if target == 0 {
		target = micPreRollFrames * micSamplesPerFrame
	}
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
