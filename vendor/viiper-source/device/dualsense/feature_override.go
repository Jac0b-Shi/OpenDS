package dualsense

// Feeder-driven feature-report overrides for the Edge profile proxy.
// The virtual device serves zero stubs for 0x60-0x7B by default; a feeder
// with a physical Edge pushes its snapshots here so host GET_REPORTs
// return real data.

func (d *DualSense) expectedFeatureLength(id byte) (int, bool) {
	switch id {
	case featureIDCalibration:
		return 41, true
	case featureIDPairing:
		return 20, true
	case featureIDFirmware:
		return 64, true
	case featureIDCommandResponse:
		return 64, true
	case featureIDEdgeHandshake:
		if !d.edge {
			return 0, false
		}
		return 64, true
	}
	if d.edge {
		if n, ok := edgeFeatureLengths[id]; ok {
			return n, true
		}
	}
	if n, ok := dsFeatureLengths[id]; ok {
		return n, true
	}
	return 0, false
}

// SetFeatureReport stores a verbatim GET_REPORT response (report ID at
// [0], exact descriptor length). Returns false for unknown IDs,
// Edge-only IDs on the standard device, or wrong lengths.
func (d *DualSense) SetFeatureReport(report []byte) bool {
	if len(report) == 0 {
		return false
	}
	id := report[0]
	n, ok := d.expectedFeatureLength(id)
	if !ok || len(report) != n {
		return false
	}
	cp := append([]byte(nil), report...)
	d.mtx.Lock()
	defer d.mtx.Unlock()
	if d.featureOverrides == nil {
		d.featureOverrides = map[byte][]byte{}
	}
	d.featureOverrides[id] = cp
	return true
}

// ClearFeatureReport drops a feeder override, restoring the built-in
// handler or zero stub. Returns false when the ID is not valid for this
// variant.
func (d *DualSense) ClearFeatureReport(id byte) bool {
	if _, ok := d.expectedFeatureLength(id); !ok {
		return false
	}
	d.mtx.Lock()
	defer d.mtx.Unlock()
	delete(d.featureOverrides, id)
	return true
}

func (d *DualSense) featureOverride(id byte) []byte {
	d.mtx.Lock()
	defer d.mtx.Unlock()
	if b, ok := d.featureOverrides[id]; ok {
		return append([]byte(nil), b...)
	}
	return nil
}
