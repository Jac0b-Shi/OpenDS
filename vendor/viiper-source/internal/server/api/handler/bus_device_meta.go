package handler

import (
	"encoding/json"
	"fmt"
	"log/slog"
	"strconv"
	"strings"

	"github.com/DualSenseClient/VIIPER/internal/server/api"
	apierror "github.com/DualSenseClient/VIIPER/internal/server/api/error"
	"github.com/DualSenseClient/VIIPER/internal/server/usb"
	"github.com/DualSenseClient/VIIPER/viipertypes"
)

// BusDeviceMeta returns a handler that merge-updates a device's
// device-specific metadata. Payload is "<devId> <meta-json>".
func BusDeviceMeta(s *usb.Server) api.HandlerFunc {
	return func(req *api.Request, res *api.Response, logger *slog.Logger) error {
		idStr, ok := req.Params["id"]
		if !ok {
			return apierror.ErrBadRequest("missing id parameter")
		}
		busID, err := strconv.ParseUint(idStr, 10, 32)
		if err != nil {
			return apierror.ErrBadRequest(fmt.Sprintf("invalid busId: %v", err))
		}
		sep := strings.Index(req.Payload, " ")
		if sep < 0 {
			return apierror.ErrBadRequest("missing device number and meta JSON")
		}
		deviceID, metaJSON := req.Payload[:sep], req.Payload[sep+1:]
		if !json.Valid([]byte(metaJSON)) {
			return apierror.ErrBadRequest("invalid meta JSON")
		}

		b := s.GetBus(uint32(busID))
		if b == nil {
			return apierror.ErrNotFound(fmt.Sprintf("bus %d not found", busID))
		}
		var found bool
		var updated viipertypes.Device
		for _, m := range b.GetAllDeviceMetas() {
			if fmt.Sprintf("%d", m.Meta.DevID) != deviceID {
				continue
			}
			found = true
			if err := api.UpdateDeviceMeta(m.Dev, metaJSON); err != nil {
				return apierror.ErrBadRequest(fmt.Sprintf("meta update failed: %v", err))
			}
			updated = viipertypes.Device{
				BusID:          m.Meta.BusID,
				DevID:          deviceID,
				Vid:            fmt.Sprintf("0x%04x", m.Dev.GetDescriptor().Device.IDVendor),
				Pid:            fmt.Sprintf("0x%04x", m.Dev.GetDescriptor().Device.IDProduct),
				Type:           inferDeviceType(m.Dev),
				DeviceSpecific: m.Dev.GetDeviceSpecificArgs(),
			}
		}
		if !found {
			return apierror.ErrNotFound(fmt.Sprintf("device %s not found on bus %d", deviceID, busID))
		}
		j, err := json.Marshal(updated)
		if err != nil {
			return apierror.ErrInternal(fmt.Sprintf("failed to marshal response: %v", err))
		}
		res.JSON = string(j)
		return nil
	}
}
