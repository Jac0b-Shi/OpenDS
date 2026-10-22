package dualsense

// UsePhysicalUSBProfile selects the observed standard DualSense USB profile.
// Must be called before attaching the device; Edge keeps its upstream profile.
// usbip-win2 maps the 4ms full-speed interrupt interval to HS bInterval=6.
func (d *DualSense) UsePhysicalUSBProfile() bool {
	if d.edge {
		return false
	}
	d.descriptor.Device.ISerialNumber = 0
	d.paceRaw = true
	for i := range d.descriptor.Interfaces {
		iface := &d.descriptor.Interfaces[i]
		if iface.HID == nil {
			continue
		}
		iface.HID.ReportDescriptor = reportDescriptor(47, 15, nil)
		for j := range iface.Endpoints {
			iface.Endpoints[j].BInterval = 4
		}
	}
	return true
}
