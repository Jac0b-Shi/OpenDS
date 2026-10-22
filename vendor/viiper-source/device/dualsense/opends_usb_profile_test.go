package dualsense

import (
	"context"
	"encoding/hex"
	"github.com/DualSenseClient/VIIPER/usbip"
	"github.com/stretchr/testify/require"
	"testing"
	"time"
)

func TestOpenDSPhysicalUSBProfileMatchesWiredCapture(t *testing.T) {
	d, err := new(nil, false)
	require.NoError(t, err)
	require.True(t, d.UsePhysicalUSBProfile())
	expected, err := hex.DecodeString("05010905a1018501093009310932093509330934150026ff007508950681020600ff09209501810205010939150025073500463b016514750495018142650005091901290f150025017501950f81020600ff0921950d81020600ff0922150026ff0075089534810285020923952f9102850509339528b10285080934952fb102850909249513b102850a0925951ab102850b09419529b102850c09429529b10285200926953fb102852109279504b10285220940953fb10285800928953fb10285810929953fb1028582092a9509b1028583092b953fb1028584092c953fb1028585092d9502b10285a0092e9501b10285e0092f953fb10285f00930953fb10285f10931953fb10285f20932950fb10285f40935953fb10285f509369503b102c0")
	require.NoError(t, err)
	for _, iface := range d.GetDescriptor().Interfaces {
		if iface.HID == nil {
			continue
		}
		actual, err := iface.HID.ReportBytes()
		require.NoError(t, err)
		require.Equal(t, expected, []byte(actual), "wired HID descriptor must match captured device")
		for _, ep := range iface.Endpoints {
			require.Equal(t, uint8(4), ep.BInterval)
		}
	}
	require.Equal(t, uint8(0), d.GetDescriptor().Device.ISerialNumber)
	edge, err := new(nil, true)
	require.NoError(t, err)
	require.False(t, edge.UsePhysicalUSBProfile())
	require.Equal(t, uint8(3), edge.GetDescriptor().Device.ISerialNumber)
}

func TestOpenDSPacedInputWaitsForFreshFrameAndCancels(t *testing.T) {
	d, err := new(nil, false)
	require.NoError(t, err)
	require.True(t, d.UsePhysicalUSBProfile())
	raw := make([]byte, InputReportSize)
	raw[0] = ReportIDInput
	require.True(t, d.SetRawInputReport(raw))
	require.Equal(t, raw, d.HandleTransfer(context.Background(), 4, usbip.DirIn, nil))
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	result := make(chan []byte, 1)
	go func() { result <- d.HandleTransfer(ctx, 4, usbip.DirIn, nil) }()
	select {
	case <-result:
		t.Fatal("duplicate input completed without a fresh publication")
	case <-time.After(20 * time.Millisecond):
	}
	// GET_REPORT must remain immediate while an interrupt read waits.
	got, handled := d.HandleControl(0xa1, 1, 0x0101, 3, 64, nil)
	require.True(t, handled)
	require.Equal(t, raw, got)
	raw[7] = 42
	require.True(t, d.SetRawInputReport(raw))
	select {
	case got = <-result:
		require.Equal(t, raw, got)
	case <-time.After(2 * time.Second):
		t.Fatal("new frame did not wake interrupt read")
	}
	go func() { result <- d.HandleTransfer(ctx, 4, usbip.DirIn, nil) }()
	cancel()
	select {
	case got = <-result:
		require.Nil(t, got)
	case <-time.After(2 * time.Second):
		t.Fatal("cancel did not release pending interrupt read")
	}
}
