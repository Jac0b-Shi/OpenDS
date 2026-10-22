package dualshock4

import (
	"encoding/binary"
	"fmt"
	"io"
	"log/slog"
	"net"
)

// Audio stream framing (TCP route bus/{busId}/{deviceid}/audio, server to
// feeder): u16 LE length N followed by N bytes of speaker PCM (exact
// host-written EP1 OUT payloads, 2ch S16LE @32kHz, up to 132B).
// N == audioResetSentinel carries a speaker-reset barrier event with no
// payload (emitted once per streaming generation; see the reset hook).
const (
	audioMaxFrame      = 132
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
func AudioStreamHandler(dev *DualShock4, logger *slog.Logger) func(conn net.Conn) error {
	return func(conn net.Conn) error {
		logger.Debug("dualshock4 audio stream begin")
		defer logger.Debug("dualshock4 audio stream end")
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

// MicStreamHandler ingests fixed 32B feeder mic frames (mono S16LE @16kHz)
// for EP2 IN. Short reads end the stream. Frames arriving while the host
// has not opened the mic interface are dropped (DS4Dongle gates the same
// way) and the stream stays open.
func MicStreamHandler(dev *DualShock4, logger *slog.Logger) func(conn net.Conn) error {
	return func(conn net.Conn) error {
		logger.Debug("dualshock4 mic stream begin")
		defer logger.Debug("dualshock4 mic stream end")
		var frame [micFrameSize]byte
		for {
			if _, err := io.ReadFull(conn, frame[:]); err != nil {
				return err
			}
			dev.QueueMicrophonePCM(frame[:])
		}
	}
}
