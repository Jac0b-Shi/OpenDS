"""枚举 HID 设备并读取 DualSense 输入报告，用于验证逆向结论。

验证点：
1. 蓝牙模式报告 ID 是否为 0x31、长度 78
2. buf[1] 的 bit1 是否为"带音频"标志
3. buf[2..66] 是否就是 64 字节 USB 报告负载
"""

import ctypes
from ctypes import wintypes

DIGCF_PRESENT = 0x02
DIGCF_DEVICEINTERFACE = 0x10
GENERIC_READ = 0x80000000
GENERIC_WRITE = 0x40000000
FILE_SHARE_READ = 0x01
FILE_SHARE_WRITE = 0x02
OPEN_EXISTING = 3
INVALID_HANDLE_VALUE = ctypes.c_void_p(-1).value


class GUID(ctypes.Structure):
    _fields_ = [("Data1", wintypes.DWORD), ("Data2", wintypes.WORD),
                ("Data3", wintypes.WORD), ("Data4", ctypes.c_ubyte * 8)]


class SP_DEVICE_INTERFACE_DATA(ctypes.Structure):
    _fields_ = [("cbSize", wintypes.DWORD), ("InterfaceClassGuid", GUID),
                ("Flags", wintypes.DWORD), ("Reserved", ctypes.c_size_t)]


class SP_DEVICE_INTERFACE_DETAIL_DATA_W(ctypes.Structure):
    _fields_ = [("cbSize", wintypes.DWORD), ("DevicePath", ctypes.c_wchar * 512)]


class HIDD_ATTRIBUTES(ctypes.Structure):
    _fields_ = [("Size", wintypes.ULONG), ("VendorID", wintypes.USHORT),
                ("ProductID", wintypes.USHORT), ("VersionNumber", wintypes.USHORT)]


class HIDP_CAPS(ctypes.Structure):
    _fields_ = [("Usage", wintypes.USHORT), ("UsagePage", wintypes.USHORT),
                ("InputReportByteLength", wintypes.USHORT),
                ("OutputReportByteLength", wintypes.USHORT),
                ("FeatureReportByteLength", wintypes.USHORT),
                ("Reserved", wintypes.USHORT * 17),
                ("NumberLinkCollectionNodes", wintypes.USHORT),
                ("NumberInputButtonCaps", wintypes.USHORT),
                ("NumberInputValueCaps", wintypes.USHORT),
                ("NumberInputDataIndices", wintypes.USHORT),
                ("NumberOutputButtonCaps", wintypes.USHORT),
                ("NumberOutputValueCaps", wintypes.USHORT),
                ("NumberOutputDataIndices", wintypes.USHORT),
                ("NumberFeatureButtonCaps", wintypes.USHORT),
                ("NumberFeatureValueCaps", wintypes.USHORT),
                ("NumberFeatureDataIndices", wintypes.USHORT)]


hid = ctypes.WinDLL("hid")
setupapi = ctypes.WinDLL("setupapi")
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)

hid.HidD_GetHidGuid.argtypes = [ctypes.POINTER(GUID)]
hid.HidD_GetAttributes.argtypes = [wintypes.HANDLE, ctypes.POINTER(HIDD_ATTRIBUTES)]
hid.HidD_GetPreparsedData.argtypes = [wintypes.HANDLE, ctypes.POINTER(ctypes.c_void_p)]
hid.HidD_FreePreparsedData.argtypes = [ctypes.c_void_p]
hid.HidP_GetCaps.argtypes = [ctypes.c_void_p, ctypes.POINTER(HIDP_CAPS)]

setupapi.SetupDiGetClassDevsW.restype = wintypes.HANDLE
setupapi.SetupDiGetClassDevsW.argtypes = [ctypes.POINTER(GUID), wintypes.LPCWSTR,
                                          wintypes.HWND, wintypes.DWORD]
setupapi.SetupDiEnumDeviceInterfaces.restype = wintypes.BOOL
setupapi.SetupDiEnumDeviceInterfaces.argtypes = [wintypes.HANDLE, ctypes.c_void_p,
                                                 ctypes.POINTER(GUID), wintypes.DWORD,
                                                 ctypes.POINTER(SP_DEVICE_INTERFACE_DATA)]
setupapi.SetupDiGetDeviceInterfaceDetailW.restype = wintypes.BOOL
setupapi.SetupDiGetDeviceInterfaceDetailW.argtypes = [wintypes.HANDLE,
                                                      ctypes.POINTER(SP_DEVICE_INTERFACE_DATA),
                                                      ctypes.c_void_p, wintypes.DWORD,
                                                      ctypes.POINTER(wintypes.DWORD),
                                                      ctypes.c_void_p]
setupapi.SetupDiDestroyDeviceInfoList.argtypes = [wintypes.HANDLE]

kernel32.CreateFileW.restype = wintypes.HANDLE
kernel32.CreateFileW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD,
                                 ctypes.c_void_p, wintypes.DWORD, wintypes.DWORD,
                                 wintypes.HANDLE]
kernel32.ReadFile.argtypes = [wintypes.HANDLE, ctypes.c_void_p, wintypes.DWORD,
                              ctypes.POINTER(wintypes.DWORD), ctypes.c_void_p]
kernel32.CloseHandle.argtypes = [wintypes.HANDLE]


def enumerate_hid():
    guid = GUID()
    hid.HidD_GetHidGuid(ctypes.byref(guid))
    hdev = setupapi.SetupDiGetClassDevsW(ctypes.byref(guid), None, None,
                                         DIGCF_PRESENT | DIGCF_DEVICEINTERFACE)
    if hdev == INVALID_HANDLE_VALUE:
        raise ctypes.WinError(ctypes.get_last_error())

    out = []
    idx = 0
    while True:
        ifd = SP_DEVICE_INTERFACE_DATA()
        ifd.cbSize = ctypes.sizeof(ifd)
        ok = setupapi.SetupDiEnumDeviceInterfaces(hdev, None, ctypes.byref(guid), idx,
                                                  ctypes.byref(ifd))
        if not ok:
            break
        need = wintypes.DWORD(0)
        setupapi.SetupDiGetDeviceInterfaceDetailW(hdev, ctypes.byref(ifd), None, 0,
                                                  ctypes.byref(need), None)
        if need.value == 0:
            idx += 1
            continue
        buf = ctypes.create_string_buffer(need.value)
        detail = ctypes.cast(buf, ctypes.POINTER(SP_DEVICE_INTERFACE_DETAIL_DATA_W))
        detail.contents.cbSize = 8 if ctypes.sizeof(ctypes.c_void_p) == 8 else 6
        ok = setupapi.SetupDiGetDeviceInterfaceDetailW(hdev, ctypes.byref(ifd), detail,
                                                       need, ctypes.byref(need), None)
        if ok:
            out.append(detail.contents.DevicePath)
        idx += 1
    setupapi.SetupDiDestroyDeviceInfoList(hdev)
    return out


def probe(path):
    h = kernel32.CreateFileW(path, GENERIC_READ | GENERIC_WRITE,
                             FILE_SHARE_READ | FILE_SHARE_WRITE, None,
                             OPEN_EXISTING, 0, None)
    if h == INVALID_HANDLE_VALUE or h is None:
        h = kernel32.CreateFileW(path, 0, FILE_SHARE_READ | FILE_SHARE_WRITE, None,
                                 OPEN_EXISTING, 0, None)
        if h == INVALID_HANDLE_VALUE or h is None:
            return None
    try:
        attr = HIDD_ATTRIBUTES()
        attr.Size = ctypes.sizeof(attr)
        if not hid.HidD_GetAttributes(h, ctypes.byref(attr)):
            return None
        caps = HIDP_CAPS()
        pp = ctypes.c_void_p()
        if hid.HidD_GetPreparsedData(h, ctypes.byref(pp)):
            hid.HidP_GetCaps(pp, ctypes.byref(caps))
            hid.HidD_FreePreparsedData(pp)
        return {"vid": attr.VendorID, "pid": attr.ProductID, "ver": attr.VersionNumber,
                "in_len": caps.InputReportByteLength, "out_len": caps.OutputReportByteLength,
                "feat_len": caps.FeatureReportByteLength, "handle": h, "path": path}
    except Exception:
        kernel32.CloseHandle(h)
        raise


def read_report(h, size, timeout_ms=800):
    buf = (ctypes.c_ubyte * size)()
    n = wintypes.DWORD(0)
    ok = kernel32.ReadFile(h, buf, size, ctypes.byref(n), None)
    if not ok:
        return None, ctypes.get_last_error()
    return bytes(buf[:n.value]), 0


if __name__ == "__main__":
    devs = enumerate_hid()
    print(f"HID devices: {len(devs)}")
    hits = []
    for p in devs:
        r = probe(p)
        if not r:
            continue
        if r["vid"] == 0x054C:
            hits.append(r)
            print(f"\n[SONY] VID={r['vid']:04X} PID={r['pid']:04X} ver={r['ver']:04X}")
            print(f"  IN={r['in_len']} OUT={r['out_len']} FEAT={r['feat_len']}")
            print(f"  {r['path'][:110]}")
        else:
            kernel32.CloseHandle(r["handle"])

    if not hits:
        print("\n未找到 Sony 设备（VID 0x054C）。请确认手柄已连接且未被 HidHide 隐藏。")
    else:
        for r in hits:
            data, err = read_report(r["handle"], max(r["in_len"], 78))
            if data:
                print(f"\n报告长度={len(data)}  ID=0x{data[0]:02X}  buf[1]=0x{data[1]:02X}"
                      f"  audio_flag={(data[1] >> 1) & 1}")
                print("前 16 字节:", " ".join(f"{b:02X}" for b in data[:16]))
                print("buf[2..10] :", " ".join(f"{b:02X}" for b in data[2:10]))
            else:
                print(f"\n读取失败 err={err}（可能被其他程序占用）")
            kernel32.CloseHandle(r["handle"])
