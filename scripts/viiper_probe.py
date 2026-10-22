"""验证 libVIIPER 能否在本机启动 USB 服务器并创建虚拟总线。

只做只读性验证：启动服务器 + 创建总线，不创建具体设备，随后立即关闭。
若需要创建设备，用 --with-device（会在系统里出现一个虚拟手柄）。

签名严格按 libVIIPER.h：
    GoUint8 NewUSBServer(USBServerConfig*, USBServerHandle*, VIIPERLogCallback);
    GoUint8 CreateUSBBus(USBServerHandle, GoUint32*);
    GoUint8 CreateDualSenseDevice(USBServerHandle, DSDeviceHandle*, GoUint32,
                                  GoUint8, GoUint16, GoUint16, DSMetaState*);
    GoUint8 RemoveDualSenseDevice(DSDeviceHandle);
    GoUint8 CloseUSBServer(USBServerHandle);
"""

import ctypes
import sys
import time
from pathlib import Path
from ctypes import c_char_p, c_int, c_uint8, c_uint16, c_uint32, c_uint64, c_void_p, POINTER, byref

DLL = str(Path(__file__).resolve().parents[1] / "vendor" / "viiper" / "libVIIPER.dll")


class USBServerConfig(ctypes.Structure):
    _fields_ = [
        ("addr", c_char_p),
        ("connection_timeout_ms", c_uint64),
        ("device_handler_connect_timeout_ms", c_uint64),
        ("write_batch_flush_interval_ms", c_uint32),
    ]


class DSMetaState(ctypes.Structure):
    _fields_ = [
        ("SerialNumber", c_char_p),
        ("MACAddress", c_char_p),
        ("Board", c_char_p),
        ("BatteryStatus", c_uint8),
        ("TemperatureCelsius", ctypes.c_double),
        ("BatteryVoltage", ctypes.c_double),
        ("ShellColor", c_char_p),
    ]


def load():
    dll = ctypes.CDLL(DLL)
    dll.NewUSBServer.argtypes = [POINTER(USBServerConfig), POINTER(c_uint64), c_void_p]
    dll.NewUSBServer.restype = c_uint8
    dll.CloseUSBServer.argtypes = [c_uint64]
    dll.CloseUSBServer.restype = c_uint8
    dll.CreateUSBBus.argtypes = [c_uint64, POINTER(c_uint32)]
    dll.CreateUSBBus.restype = c_uint8
    dll.RemoveUSBBus.argtypes = [c_uint64, c_uint32]
    dll.RemoveUSBBus.restype = c_uint8
    dll.CreateDualSenseDevice.argtypes = [
        c_uint64, POINTER(c_uint64), c_uint32, c_uint8, c_uint16, c_uint16,
        POINTER(DSMetaState),
    ]
    dll.CreateDualSenseDevice.restype = c_uint8
    dll.RemoveDualSenseDevice.argtypes = [c_uint64]
    dll.RemoveDualSenseDevice.restype = c_uint8
    dll.SetDualSenseRawInputReport.argtypes = [c_uint64, POINTER(c_uint8), ctypes.c_size_t]
    dll.SetDualSenseRawInputReport.restype = c_uint8
    return dll


def main():
    with_device = "--with-device" in sys.argv
    dll = load()

    cfg = USBServerConfig()
    cfg.addr = b"127.0.0.1:3241"
    cfg.connection_timeout_ms = 30000
    cfg.device_handler_connect_timeout_ms = 5000
    cfg.write_batch_flush_interval_ms = 1

    handle = c_uint64(0)
    ok = dll.NewUSBServer(byref(cfg), byref(handle), None)
    print(f"NewUSBServer -> ok={ok} handle=0x{handle.value:X}")
    if not ok:
        print("服务器启动失败：可能是 UDE 驱动未就绪或端口被占用")
        return 1

    try:
        bus = c_uint32(0)
        ok = dll.CreateUSBBus(handle, byref(bus))
        print(f"CreateUSBBus  -> ok={ok} busID={bus.value}")
        if not ok:
            return 1

        if not with_device:
            print("\n(未创建虚拟设备；加 --with-device 可创建并观察系统反应)")
            return 0

        dev = c_uint64(0)
        t0 = time.perf_counter()
        ok = dll.CreateDualSenseDevice(handle, byref(dev), bus.value, 1, 0x054C, 0x0CE6, None)
        dt = (time.perf_counter() - t0) * 1000
        print(f"CreateDualSenseDevice -> ok={ok} handle=0x{dev.value:X} ({dt:.0f} ms)")
        if not ok:
            return 1

        # 推一帧静音输入报告，验证 raw 通路
        rpt = (c_uint8 * 64)()
        rpt[0] = 0x01
        rpt[1] = 0x80
        rpt[2] = 0x80
        rpt[3] = 0x80
        rpt[4] = 0x80
        ok = dll.SetDualSenseRawInputReport(dev, rpt, 64)
        print(f"SetDualSenseRawInputReport(64B) -> ok={ok}")

        print("\n设备已创建，观察 5 秒……（此时系统里应出现 DualSense）")
        time.sleep(5)

        ok = dll.RemoveDualSenseDevice(dev)
        print(f"RemoveDualSenseDevice -> ok={ok}")
    finally:
        dll.CloseUSBServer(handle)
        print("CloseUSBServer 完成")
    return 0


if __name__ == "__main__":
    sys.exit(main())
