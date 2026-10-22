package dualsense

import (
	"encoding/binary"
	"fmt"
	"io"
	"log/slog"
	"net"
)

// Audio stream framing (TCP route bus/{busId}/{deviceid}/audio, server to
// feeder): u16 LE length N followed by N bytes of speaker PCM (exact
// host-written EP1 OUT payloads, 4ch S16LE @48kHz, up to 392B).
// N == audioResetSentinel carries a speaker-reset barrier event with no
// payload (emitted once per streaming generation; see the reset hook).
// Feeder to server (mic ingest) arrives in a later commit.
const (
	audioMaxFrame      = 392
	audioResetSentinel = 0xFFFF
)

// writeAudioFrame writes one length-prefixed speaker item: PCM bytes, or
// the two-byte reset-barrier sentinel with no payload.
func writeAudioFrame(conn net.Conn, ev SpeakerEvent) error {
	if ev.Reset {
		var hdr [2]byte
		binary.LittleEndian.PutUint16(hdr[:], audioResetSentinel)
		_, err := conn.Write(hdr[:])
		return err
	}
	pcm := ev.PCM
	if len(pcm) > audioMaxFrame {
		return fmt.Errorf("speaker frame too large: %d", len(pcm))
	}
	var hdr [2]byte
	binary.LittleEndian.PutUint16(hdr[:], uint16(len(pcm)))
	if _, err := conn.Write(hdr[:]); err != nil {
		return err
	}
	if _, err := conn.Write(pcm); err != nil {
		return err
	}
	return nil
}

// AudioStreamHandler returns a StreamHandlerFunc serving speaker PCM and
// reset barriers to one feeder over TCP. It ends when the connection
// breaks; the subscription is always released.
func AudioStreamHandler(dev *DualSense, logger *slog.Logger) func(conn net.Conn) error {
	return func(conn net.Conn) error {
		logger.Debug("dualsense audio stream begin")
		defer logger.Debug("dualsense audio stream end")
		ch, unsub := dev.SubscribeSpeaker()
		defer unsub()
		for ev := range ch {
			if err := writeAudioFrame(conn, ev); err != nil {
				return err
			}
		}
		return nil
	}
}

// HapticsStreamHandler serves the rear voice-coil pair (2ch S16LE @48kHz)
// and reset barriers to one feeder. Same framing as the speaker stream.
func HapticsStreamHandler(dev *DualSense, logger *slog.Logger) func(conn net.Conn) error {
	return func(conn net.Conn) error {
		logger.Debug("dualsense haptics stream begin")
		defer logger.Debug("dualsense haptics stream end")
		ch, unsub := dev.SubscribeHaptics()
		defer unsub()
		for ev := range ch {
			if err := writeAudioFrame(conn, SpeakerEvent(ev)); err != nil {
				return err
			}
		}
		return nil
	}
}

// MicStreamHandler ingests fixed 192B feeder mic frames (2ch S16LE @48kHz)
// for EP2 IN. Short reads end the stream. Frames arriving while the host
// has not opened the mic interface are dropped (DS5Dongle mic_add_queue
// gates the same way) and the stream stays open.
func MicStreamHandler(dev *DualSense, logger *slog.Logger) func(conn net.Conn) error {
	return func(conn net.Conn) error {
		logger.Debug("dualsense mic stream begin")
		defer logger.Debug("dualsense mic stream end")
		var frame [micFrameSize]byte
		for {
			if _, err := io.ReadFull(conn, frame[:]); err != nil {
				return err
			}
			dev.QueueMicrophonePCM(frame[:])
		}
	}
}
