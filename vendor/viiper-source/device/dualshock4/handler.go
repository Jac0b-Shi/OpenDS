package dualshock4

import (
	"encoding/json"
	"fmt"
	"io"
	"log/slog"
	"net"

	"github.com/DualSenseClient/VIIPER/device"
	"github.com/DualSenseClient/VIIPER/internal/server/api"
	"github.com/DualSenseClient/VIIPER/usb"
)

func init() {
	api.RegisterDevice("dualshock4", &handler{})
}

type handler struct{}

var serials = map[string]struct{}{}

func (h *handler) CreateDevice(o *device.CreateOptions) (usb.Device, error) {
	if o == nil {
		o = &device.CreateOptions{}
	}

	metaState := MetaState{}
	if o.DeviceSpecific != "" {
		if err := json.Unmarshal([]byte(o.DeviceSpecific), &metaState); err != nil {
			return nil, fmt.Errorf("invalid device specific JSON: %w", err)
		}
	}
	serial := DefaultSerialString
	if metaState.SerialNumber != "" {
		serial = metaState.SerialNumber
	}
	serial = serialsNormalize(serial)
	if _, ok := serials[serial]; ok {
		for i := 1; i < 16; i++ {
			newSerial := fmt.Sprintf("%s%02X", serial[:len(serial)-2], i)
			if _, ok := serials[newSerial]; !ok {
				serial = newSerial
				break
			}
		}
	}
	metaState.SerialNumber = serial
	serials[serial] = struct{}{}
	b, err := json.Marshal(metaState)
	if err != nil {
		return nil, fmt.Errorf("marshal meta state: %w", err)
	}
	o.DeviceSpecific = string(b)
	return New(o)
}

func (h *handler) StreamHandler() api.StreamHandlerFunc {
	return func(conn net.Conn, devPtr *usb.Device, logger *slog.Logger) error {
		defer func() {
			ds4, ok := (*devPtr).(*DualShock4)
			if !ok {
				slog.Warn("device is not DualShock4 on disconnect")
				return
			}
			delete(serials, ds4.metaState.SerialNumber)
			slog.Debug("DS4 disconnected, serial released", "serial", ds4.metaState.SerialNumber)
		}()
		if devPtr == nil || *devPtr == nil {
			return fmt.Errorf("nil device")
		}
		ds4, ok := (*devPtr).(*DualShock4)
		if !ok {
			return fmt.Errorf("%w: expected DualShock4", device.ErrWrongDeviceType)
		}

		ds4.SetOutputCallback(func(feedback OutputState) {
			data, err := feedback.MarshalBinary()
			if err != nil {
				logger.Error("failed to marshal feedback", "error", err)
				return
			}
			if _, err := conn.Write(data); err != nil {
				logger.Error("failed to send feedback", "error", err)
			}
		})

		buf := make([]byte, 31)
		for {
			if _, err := io.ReadFull(conn, buf); err != nil {
				if err == io.EOF {
					logger.Info("client disconnected")
					return nil
				}
				return fmt.Errorf("read input state: %w", err)
			}

			var state InputState
			if err := state.UnmarshalBinary(buf); err != nil {
				return fmt.Errorf("unmarshal input state: %w", err)
			}
			ds4.UpdateInputState(&state)
		}
	}
}

// AudioStreamHandler serves speaker PCM to TCP feeders (see audio_stream.go).
func (h *handler) AudioStreamHandler() api.StreamHandlerFunc {
	return func(conn net.Conn, devPtr *usb.Device, logger *slog.Logger) error {
		if devPtr == nil || *devPtr == nil {
			return fmt.Errorf("nil device")
		}
		ds4, ok := (*devPtr).(*DualShock4)
		if !ok {
			return fmt.Errorf("%w: expected DualShock4", device.ErrWrongDeviceType)
		}
		return AudioStreamHandler(ds4, logger)(conn)
	}
}

func (h *handler) UpdateMetaState(meta string, dev *usb.Device) error {
	ds4, ok := (*dev).(*DualShock4)
	if !ok {
		return fmt.Errorf("%w: expected DualShock4", device.ErrWrongDeviceType)
	}
	var metaState MetaState
	err := json.Unmarshal([]byte(meta), &metaState)
	if err != nil {
		return fmt.Errorf("unmarshal meta state: %w", err)
	}
	ds4.SetMetaState(metaState)

	return nil
}

// MicStreamHandler ingests feeder mic frames over TCP.
func (h *handler) MicStreamHandler() api.StreamHandlerFunc {
	return func(conn net.Conn, devPtr *usb.Device, logger *slog.Logger) error {
		if devPtr == nil || *devPtr == nil {
			return fmt.Errorf("nil device")
		}
		ds4, ok := (*devPtr).(*DualShock4)
		if !ok {
			return fmt.Errorf("%w: expected DualShock4", device.ErrWrongDeviceType)
		}
		return MicStreamHandler(ds4, logger)(conn)
	}
}

// RawStreamHandler ingests feeder raw 64B input reports over TCP.
func (h *handler) RawStreamHandler() api.StreamHandlerFunc {
	return func(conn net.Conn, devPtr *usb.Device, logger *slog.Logger) error {
		if devPtr == nil || *devPtr == nil {
			return fmt.Errorf("nil device")
		}
		ds4, ok := (*devPtr).(*DualShock4)
		if !ok {
			return fmt.Errorf("%w: expected DualShock4", device.ErrWrongDeviceType)
		}
		return RawStreamHandler(ds4, logger)(conn)
	}
}
