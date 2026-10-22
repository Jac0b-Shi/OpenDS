package api

import (
	"fmt"

	"github.com/DualSenseClient/VIIPER/usb"
)

// MetaUpdater is implemented by DeviceHandlers supporting runtime metadata
// updates (e.g., DualSense serial/battery identity).
type MetaUpdater interface {
	UpdateMetaState(meta string, dev *usb.Device) error
}

// UpdateDeviceMeta applies a device-specific metadata JSON document to a
// device via its registered handler. Returns an error when the device type
// is unknown or does not support meta updates.
func UpdateDeviceMeta(dev usb.Device, meta string) error {
	if dev == nil {
		return fmt.Errorf("nil device")
	}
	reg := GetRegistration(inferDeviceType(dev))
	if reg == nil {
		return fmt.Errorf("no handler for device type")
	}
	updater, ok := reg.(MetaUpdater)
	if !ok {
		return fmt.Errorf("meta updates not supported for device type")
	}
	return updater.UpdateMetaState(meta, &dev)
}
