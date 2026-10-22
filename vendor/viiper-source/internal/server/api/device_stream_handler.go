package api

import (
	"fmt"
	"log/slog"
	"net"
	"path/filepath"
	"reflect"
	"strings"

	"github.com/DualSenseClient/VIIPER/internal/server/usb"
	pusb "github.com/DualSenseClient/VIIPER/usb"
)

// AudioStreamProvider is an optional DeviceHandler extension for devices
// that stream speaker PCM to feeders (DualSense). Devices without it fail
// the audio route with "not supported".
type AudioStreamProvider interface {
	AudioStreamHandler() StreamHandlerFunc
}

// HapticsStreamProvider is the rear-haptics counterpart of
// AudioStreamProvider.
type HapticsStreamProvider interface {
	HapticsStreamHandler() StreamHandlerFunc
}

// MicStreamProvider is the microphone-ingest counterpart: feeders write
// fixed 192B PCM frames (2ch S16LE @48kHz) for EP2 IN.
type MicStreamProvider interface {
	MicStreamHandler() StreamHandlerFunc
}

// RawStreamProvider is the raw-input ingest counterpart: feeders with a
// real controller write exact 64B USB input reports 0x01; the core serves
// them verbatim while set and synthesizes otherwise.
type RawStreamProvider interface {
	RawStreamHandler() StreamHandlerFunc
}

// DeviceAudioStreamHandler dispatches audio streams to device handlers
// implementing AudioStreamProvider.
func DeviceAudioStreamHandler(srv *usb.Server) StreamHandlerFunc {
	return deviceSubStreamHandler(srv, "audio", func(reg DeviceHandler) (StreamHandlerFunc, bool) {
		provider, ok := reg.(AudioStreamProvider)
		if !ok {
			return nil, false
		}
		return provider.AudioStreamHandler(), true
	})
}

// DeviceHapticsStreamHandler dispatches haptics streams to device handlers
// implementing HapticsStreamProvider.
func DeviceHapticsStreamHandler(srv *usb.Server) StreamHandlerFunc {
	return deviceSubStreamHandler(srv, "haptics", func(reg DeviceHandler) (StreamHandlerFunc, bool) {
		provider, ok := reg.(HapticsStreamProvider)
		if !ok {
			return nil, false
		}
		return provider.HapticsStreamHandler(), true
	})
}

// DeviceMicStreamHandler dispatches mic-ingest streams to device handlers
// implementing MicStreamProvider.
func DeviceMicStreamHandler(srv *usb.Server) StreamHandlerFunc {
	return deviceSubStreamHandler(srv, "mic", func(reg DeviceHandler) (StreamHandlerFunc, bool) {
		provider, ok := reg.(MicStreamProvider)
		if !ok {
			return nil, false
		}
		return provider.MicStreamHandler(), true
	})
}

// DeviceRawStreamHandler dispatches raw-input ingest streams to device
// handlers implementing RawStreamProvider.
func DeviceRawStreamHandler(srv *usb.Server) StreamHandlerFunc {
	return deviceSubStreamHandler(srv, "raw", func(reg DeviceHandler) (StreamHandlerFunc, bool) {
		provider, ok := reg.(RawStreamProvider)
		if !ok {
			return nil, false
		}
		return provider.RawStreamHandler(), true
	})
}

func deviceSubStreamHandler(_ *usb.Server, kind string, pick func(DeviceHandler) (StreamHandlerFunc, bool)) StreamHandlerFunc {
	return func(conn net.Conn, dev *pusb.Device, logger *slog.Logger) error {
		defer conn.Close() //nolint:errcheck

		if dev == nil || *dev == nil {
			return fmt.Errorf("nil device")
		}

		deviceType := inferDeviceType(*dev)
		reg := GetRegistration(deviceType)
		if reg == nil {
			return fmt.Errorf("no handler for device type: %s", deviceType)
		}
		handler, ok := pick(reg)
		if !ok {
			return fmt.Errorf("%s not supported for device type: %s", kind, deviceType)
		}
		if err := handler(conn, dev, logger); err != nil {
			return err
		}
		return nil
	}
}

// DeviceStreamHandler returns a stream handler func that dynamically dispatches
// to device-specific handlers based on device type.
func DeviceStreamHandler(srv *usb.Server) StreamHandlerFunc {
	return func(conn net.Conn, dev *pusb.Device, logger *slog.Logger) error {
		defer conn.Close() //nolint:errcheck

		if dev == nil || *dev == nil {
			return fmt.Errorf("nil device")
		}

		deviceType := inferDeviceType(*dev)
		reg := GetRegistration(deviceType)
		if reg == nil {
			return fmt.Errorf("no handler for device type: %s", deviceType)
		}
		handler := reg.StreamHandler()
		if err := handler(conn, dev, logger); err != nil {
			return err
		}
		return nil
	}
}

func inferDeviceType(dev any) string {
	if dev == nil {
		return ""
	}
	t := reflect.TypeOf(dev)
	if t.Kind() == reflect.Pointer {
		t = t.Elem()
	}
	pkg := t.PkgPath() // e.g., "github.com/DualSenseClient/VIIPER/device/xbox360"
	if pkg != "" {
		base := filepath.Base(pkg)
		if base != "." && base != string(filepath.Separator) {
			return strings.ToLower(base)
		}
	}
	return strings.ToLower(t.Name())
}
