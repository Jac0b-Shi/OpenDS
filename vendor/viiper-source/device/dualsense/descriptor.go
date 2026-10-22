package dualsense

import (
	"github.com/DualSenseClient/VIIPER/usb"
	"github.com/DualSenseClient/VIIPER/usb/hid"
)

// HID report descriptors mirror DS5Dongle src/usb_descriptors.cpp
// (desc_hid_report_ds, 321B / desc_hid_report_dse, 437B; standard build with
// ENABLE_SERIAL=OFF and no wake keyboard). DS and Edge differ in three
// places: the 0x02 output count (47 vs 63), the 0xF2 feature count (15 vs 52),
// and the Edge-only 0x60-0x65/0x68/0x70-0x7B block. Edge extras after 0x60
// carry no explicit ReportCount in the reference and inherit 63 from 0x60 —
// the tail below mirrors that by omitting ReportCount there.

// inputItems is the 0x01 input layout shared by DS and Edge. These items live
// inside the single application collection.
func inputItems() []hid.Item {
	return []hid.Item{
		hid.ReportID{ID: ReportIDInput},
		hid.Usage{Usage: hid.UsageX},
		hid.Usage{Usage: hid.UsageY},
		hid.Usage{Usage: hid.UsageZ},
		hid.Usage{Usage: hid.UsageRz},
		hid.Usage{Usage: hid.UsageRx},
		hid.Usage{Usage: hid.UsageRy},
		hid.LogicalMinimum{Min: 0},
		hid.LogicalMaximum{Max: 255},
		hid.ReportSize{Bits: 8},
		hid.ReportCount{Count: 6},
		hid.Input{Flags: hid.MainData | hid.MainVar | hid.MainAbs},

		hid.UsagePage{Page: 0xFF00},
		hid.Usage{Usage: 0x20},
		hid.ReportCount{Count: 1},
		hid.Input{Flags: hid.MainData | hid.MainVar | hid.MainAbs},

		hid.UsagePage{Page: hid.UsagePageGenericDesktop},
		hid.Usage{Usage: 0x39},
		hid.LogicalMinimum{Min: 0},
		hid.LogicalMaximum{Max: 7},
		hid.PhysicalMinimum{Min: 0},
		hid.PhysicalMaximum{Max: 315},
		hid.Unit{Value: 0x14},
		hid.ReportSize{Bits: 4},
		hid.ReportCount{Count: 1},
		hid.Input{Flags: hid.MainData | hid.MainVar | hid.MainNullState},
		hid.Unit{Value: 0},

		hid.UsagePage{Page: hid.UsagePageButton},
		hid.UsageMinimum{Min: 0x01},
		hid.UsageMaximum{Max: 0x0F},
		hid.LogicalMinimum{Min: 0},
		hid.LogicalMaximum{Max: 1},
		hid.ReportSize{Bits: 1},
		hid.ReportCount{Count: 15},
		hid.Input{Flags: hid.MainData | hid.MainVar | hid.MainAbs},

		hid.UsagePage{Page: 0xFF00},
		hid.Usage{Usage: 0x21},
		hid.ReportCount{Count: 13},
		hid.Input{Flags: hid.MainData | hid.MainVar | hid.MainAbs},

		hid.UsagePage{Page: 0xFF00},
		hid.Usage{Usage: 0x22},
		hid.LogicalMinimum{Min: 0},
		hid.LogicalMaximum{Max: 255},
		hid.ReportSize{Bits: 8},
		hid.ReportCount{Count: 52},
		hid.Input{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
	}
}

// outputItems is report 0x02 with the per-variant count (DS 47, Edge 63).
func outputItems(count uint16) []hid.Item {
	return []hid.Item{
		hid.ReportID{ID: ReportIDOutput},
		hid.Usage{Usage: 0x23},
		hid.ReportCount{Count: count},
		hid.Output{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
	}
}

// sharedFeatureItems are the feature reports identical on DS and Edge,
// except 0xF2 which takes the per-variant count (DS 15, Edge 52).
func sharedFeatureItems(f2Count uint16) []hid.Item {
	return []hid.Item{
		hid.ReportID{ID: featureIDCalibration}, hid.Usage{Usage: 0x33}, hid.ReportCount{Count: 40}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
		hid.ReportID{ID: 0x08}, hid.Usage{Usage: 0x34}, hid.ReportCount{Count: 47}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
		hid.ReportID{ID: featureIDPairing}, hid.Usage{Usage: 0x24}, hid.ReportCount{Count: 19}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
		hid.ReportID{ID: 0x0A}, hid.Usage{Usage: 0x25}, hid.ReportCount{Count: 26}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
		hid.ReportID{ID: 0x0B}, hid.Usage{Usage: 0x41}, hid.ReportCount{Count: 41}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
		hid.ReportID{ID: 0x0C}, hid.Usage{Usage: 0x42}, hid.ReportCount{Count: 41}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
		hid.ReportID{ID: featureIDFirmware}, hid.Usage{Usage: 0x26}, hid.ReportCount{Count: 63}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
		hid.ReportID{ID: 0x21}, hid.Usage{Usage: 0x27}, hid.ReportCount{Count: 4}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
		hid.ReportID{ID: 0x22}, hid.Usage{Usage: 0x40}, hid.ReportCount{Count: 63}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
		hid.ReportID{ID: 0x80}, hid.Usage{Usage: 0x28}, hid.ReportCount{Count: 63}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
		hid.ReportID{ID: 0x81}, hid.Usage{Usage: 0x29}, hid.ReportCount{Count: 63}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
		hid.ReportID{ID: 0x82}, hid.Usage{Usage: 0x2A}, hid.ReportCount{Count: 9}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
		hid.ReportID{ID: 0x83}, hid.Usage{Usage: 0x2B}, hid.ReportCount{Count: 63}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
		hid.ReportID{ID: 0x84}, hid.Usage{Usage: 0x2C}, hid.ReportCount{Count: 63}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
		hid.ReportID{ID: 0x85}, hid.Usage{Usage: 0x2D}, hid.ReportCount{Count: 2}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
		hid.ReportID{ID: 0xA0}, hid.Usage{Usage: 0x2E}, hid.ReportCount{Count: 1}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
		hid.ReportID{ID: 0xE0}, hid.Usage{Usage: 0x2F}, hid.ReportCount{Count: 63}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
		hid.ReportID{ID: 0xF0}, hid.Usage{Usage: 0x30}, hid.ReportCount{Count: 63}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
		hid.ReportID{ID: 0xF1}, hid.Usage{Usage: 0x31}, hid.ReportCount{Count: 63}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
		hid.ReportID{ID: 0xF2}, hid.Usage{Usage: 0x32}, hid.ReportCount{Count: f2Count}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
		hid.ReportID{ID: 0xF4}, hid.Usage{Usage: 0x35}, hid.ReportCount{Count: 63}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
		hid.ReportID{ID: 0xF5}, hid.Usage{Usage: 0x36}, hid.ReportCount{Count: 3}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
	}
}

// dsTail is the DS-only tail: 0xF6-0xF9.
func dsTail() []hid.Item {
	return []hid.Item{
		hid.ReportID{ID: 0xF6}, hid.Usage{Usage: 0x37}, hid.ReportCount{Count: 63}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
		hid.ReportID{ID: 0xF7}, hid.Usage{Usage: 0x38}, hid.ReportCount{Count: 63}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
		hid.ReportID{ID: 0xF8}, hid.Usage{Usage: 0x39}, hid.ReportCount{Count: 63}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
		hid.ReportID{ID: 0xF9}, hid.Usage{Usage: 0x3A}, hid.ReportCount{Count: 63}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
	}
}

// dseTail is the Edge-only tail. Reports after 0x60 omit ReportCount and
// inherit 63, exactly like the reference.
func dseTail() []hid.Item {
	inherited := func(id, usage uint8) []hid.Item {
		return []hid.Item{
			hid.ReportID{ID: id}, hid.Usage{Usage: uint16(usage)}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
		}
	}
	items := []hid.Item{
		hid.ReportID{ID: 0x60}, hid.Usage{Usage: 0x41}, hid.ReportCount{Count: 63}, hid.Feature{Flags: hid.MainData | hid.MainVar | hid.MainAbs},
	}
	ordered := []struct {
		id, usage uint8
	}{
		{0x61, 0x42}, {0x62, 0x43}, {0x63, 0x44}, {0x64, 0x45}, {0x65, 0x46},
		{0x68, 0x47},
		{0x70, 0x48}, {0x71, 0x49}, {0x72, 0x4A}, {0x73, 0x4B}, {0x74, 0x4C},
		{0x75, 0x4D}, {0x76, 0x4E}, {0x77, 0x4F}, {0x78, 0x50}, {0x79, 0x51},
		{0x7A, 0x52}, {0x7B, 0x53},
	}
	for _, o := range ordered {
		items = append(items, inherited(o.id, o.usage)...)
	}
	return append(items, dsTail()...)
}

// reportDescriptor assembles the single application collection.
func reportDescriptor(outputCount, f2Count uint16, tail []hid.Item) hid.ReportDescriptor {
	inner := inputItems()
	inner = append(inner, outputItems(outputCount)...)
	inner = append(inner, sharedFeatureItems(f2Count)...)
	inner = append(inner, tail...)
	return hid.ReportDescriptor{Items: []hid.Item{
		hid.UsagePage{Page: hid.UsagePageGenericDesktop},
		hid.Usage{Usage: hid.UsageGamePad},
		hid.Collection{Kind: hid.CollectionApplication, Items: inner},
	}}
}

var (
	dsReportDescriptor  = reportDescriptor(47, 15, dsTail())
	dseReportDescriptor = reportDescriptor(63, 52, dseTail())
)

// audioInterfaces builds IF0 (Audio Control) + IF1 AS OUT alt0/1 (4ch
// 16-bit 48kHz, EP1 OUT isochronous adaptive, 392B) + IF2 AS IN alt0/1 (2ch
// 16-bit 48kHz, EP2 IN isochronous asynchronous, 196B), byte-identical in
// structure to DS5Dongle descriptor_configuration (standard build). Shared
// by DS and Edge; only the HID report length differs per variant.
func audioInterfaces() []usb.InterfaceConfig {
	acBlobs := func(payloads ...[]byte) []usb.ClassSpecificDescriptor {
		out := make([]usb.ClassSpecificDescriptor, 0, len(payloads))
		for _, p := range payloads {
			out = append(out, usb.ClassSpecificDescriptor{DescriptorType: 0x24, Payload: p})
		}
		return out
	}
	return []usb.InterfaceConfig{
		{
			Descriptor: usb.InterfaceDescriptor{
				BInterfaceNumber: 0x00, BAlternateSetting: 0x00, BNumEndpoints: 0x00,
				BInterfaceClass: 0x01, BInterfaceSubClass: 0x01, BInterfaceProtocol: 0x00,
			},
			ClassDescriptors: acBlobs(
				// AC Header: bcdADC 1.00, wTotalLength 73, 2 streaming IFs (1, 2).
				[]byte{0x01, 0x00, 0x01, 0x49, 0x00, 0x02, 0x01, 0x02},
				// Input Terminal 1 (USB streaming, 4ch) -> assoc 6.
				[]byte{0x02, 0x01, 0x01, 0x01, 0x06, 0x04, 0x33, 0x00, 0x00, 0x00},
				// Feature Unit 2 (mute+volume master).
				[]byte{0x06, 0x02, 0x01, 0x01, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00},
				// Output Terminal 3 (speaker) <- unit 2.
				[]byte{0x03, 0x03, 0x01, 0x03, 0x04, 0x02, 0x00},
				// Input Terminal 4 (headset mic, 2ch) assoc 3.
				[]byte{0x02, 0x04, 0x02, 0x04, 0x03, 0x02, 0x03, 0x00, 0x00, 0x00},
				// Feature Unit 5 (mute+volume master).
				[]byte{0x06, 0x05, 0x04, 0x01, 0x03, 0x00, 0x00},
				// Output Terminal 6 (USB streaming) <- unit 5.
				[]byte{0x03, 0x06, 0x01, 0x01, 0x01, 0x05, 0x00},
			),
		},
		{
			Descriptor: usb.InterfaceDescriptor{
				BInterfaceNumber: 0x01, BAlternateSetting: 0x00, BNumEndpoints: 0x00,
				BInterfaceClass: 0x01, BInterfaceSubClass: 0x02, BInterfaceProtocol: 0x00,
			},
		},
		{
			Descriptor: usb.InterfaceDescriptor{
				BInterfaceNumber: 0x01, BAlternateSetting: 0x01, BNumEndpoints: 0x01,
				BInterfaceClass: 0x01, BInterfaceSubClass: 0x02, BInterfaceProtocol: 0x00,
			},
			ClassDescriptors: acBlobs(
				// AS General: terminal 1, 1 frame delay, PCM.
				[]byte{0x01, 0x01, 0x01, 0x01, 0x00},
				// Format Type I: 4ch, 2B subframe, 16-bit, 48kHz.
				[]byte{0x02, 0x01, 0x04, 0x02, 0x10, 0x01, 0x80, 0xBB, 0x00},
			),
			Endpoints: []usb.EndpointDescriptor{
				{
					BEndpointAddress: 0x01, // OUT EP1
					BMAttributes:     0x09, // Isochronous, adaptive
					WMaxPacketSize:   392,
					BInterval:        1,
					AudioIso:         true, // DS5Dongle: 9-byte audio EP (bRefresh/bSynchAddress)
					ClassDescriptors: []usb.ClassSpecificDescriptor{
						{DescriptorType: 0x25, Payload: []byte{0x01, 0x00, 0x00, 0x00, 0x00}},
					},
				},
			},
		},
		{
			Descriptor: usb.InterfaceDescriptor{
				BInterfaceNumber: 0x02, BAlternateSetting: 0x00, BNumEndpoints: 0x00,
				BInterfaceClass: 0x01, BInterfaceSubClass: 0x02, BInterfaceProtocol: 0x00,
			},
		},
		{
			Descriptor: usb.InterfaceDescriptor{
				BInterfaceNumber: 0x02, BAlternateSetting: 0x01, BNumEndpoints: 0x01,
				BInterfaceClass: 0x01, BInterfaceSubClass: 0x02, BInterfaceProtocol: 0x00,
			},
			ClassDescriptors: acBlobs(
				// AS General: terminal 6, 1 frame delay, PCM.
				[]byte{0x01, 0x06, 0x01, 0x01, 0x00},
				// Format Type I: 2ch, 2B subframe, 16-bit, 48kHz.
				[]byte{0x02, 0x01, 0x02, 0x02, 0x10, 0x01, 0x80, 0xBB, 0x00},
			),
			Endpoints: []usb.EndpointDescriptor{
				{
					BEndpointAddress: 0x82, // IN EP2
					BMAttributes:     0x05, // Isochronous, asynchronous
					WMaxPacketSize:   196,
					BInterval:        1,
					AudioIso:         true, // DS5Dongle: 9-byte audio EP (bRefresh/bSynchAddress)
					ClassDescriptors: []usb.ClassSpecificDescriptor{
						{DescriptorType: 0x25, Payload: []byte{0x01, 0x00, 0x00, 0x00, 0x00}},
					},
				},
			},
		},
	}
}

// hidInterface is the HID gamepad interface (IF3 once audio is present).
func hidInterface(report hid.ReportDescriptor) usb.InterfaceConfig {
	return usb.InterfaceConfig{
		Descriptor: usb.InterfaceDescriptor{
			BInterfaceNumber:   0x03,
			BAlternateSetting:  0x00,
			BNumEndpoints:      0x02,
			BInterfaceClass:    0x03, // HID
			BInterfaceSubClass: 0x00,
			BInterfaceProtocol: 0x00,
			IInterface:         0x00,
		},
		HID: &usb.HIDFunction{
			Descriptor: usb.HIDDescriptor{
				BcdHID:       0x0111,
				BCountryCode: 0x00,
				Descriptors: []usb.HIDSubDescriptor{
					{Type: usb.ReportDescType},
				},
			},
			ReportDescriptor: report,
		},
		Endpoints: []usb.EndpointDescriptor{
			{
				BEndpointAddress: EndpointIn,
				BMAttributes:     0x03, // Interrupt
				WMaxPacketSize:   64,
				BInterval:        1,
			},
			{
				BEndpointAddress: EndpointOut,
				BMAttributes:     0x03, // Interrupt
				WMaxPacketSize:   64,
				BInterval:        1,
			},
		},
	}
}

// hidInterfaceFor builds the per-variant interface list: shared audio
// interfaces plus the variant HID interface.
func hidInterfaceFor(edge bool) []usb.InterfaceConfig {
	hidIF := hidInterface(dsReportDescriptor)
	if edge {
		hidIF = hidInterface(dseReportDescriptor)
	}
	return append(audioInterfaces(), hidIF)
}

func baseDeviceDescriptor() usb.DeviceDescriptor {
	return usb.DeviceDescriptor{
		BcdUSB:             0x0200,
		BDeviceClass:       0x00,
		BDeviceSubClass:    0x00,
		BDeviceProtocol:    0x00,
		BMaxPacketSize0:    0x40,
		IDVendor:           DefaultVID,
		IDProduct:          DefaultPIDDS,
		BcdDevice:          0x0100,
		IManufacturer:      0x01,
		IProduct:           0x02,
		ISerialNumber:      0x03,
		BNumConfigurations: 0x01,
		Speed:              2, // Full speed
	}
}

func baseConfiguration() usb.ConfigurationDescriptor {
	return usb.ConfigurationDescriptor{
		BConfigurationValue: 0x01,
		BMAttributes:        0xC0, // Self-powered, no remote wakeup
		BMaxPower:           0xFA, // 500mA
	}
}
