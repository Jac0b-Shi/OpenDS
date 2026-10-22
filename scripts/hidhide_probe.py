"""探测 HidHide 驱动的 IOCTL 通道：确认哪些功能在当前驱动版本可用。

只读为主；唯一会写的是 SET_ACTIVE（会立即还原）。
"""

import ctypes
from ctypes import c_uint32, c_void_p, byref

GENERIC_READ = 0x80000000
FILE_SHARE_READ = 0x01
FILE_SHARE_WRITE = 0x02
FILE_SHARE_DELETE = 0x04
OPEN_EXISTING = 3


def ctl(n):
    return (0x8001 << 16) | (1 << 14) | (n << 2)


IOCTL_GET_WHITELIST = ctl(2048)
IOCTL_SET_WHITELIST = ctl(2049)
IOCTL_GET_BLACKLIST = ctl(2050)
IOCTL_SET_BLACKLIST = ctl(2051)
IOCTL_GET_ACTIVE = ctl(2052)
IOCTL_SET_ACTIVE = ctl(2053)
IOCTL_GET_WLINVERSE = ctl(2054)
IOCTL_SET_WLINVERSE = ctl(2055)
IOCTL_ADD_SESSION_BLACKLIST = ctl(2056)
IOCTL_CLR_SESSION_BLACKLIST = ctl(2057)

k32 = ctypes.WinDLL("kernel32", use_last_error=True)
k32.CreateFileW.restype = c_void_p
k32.CreateFileW.argtypes = [ctypes.c_wchar_p, c_uint32, c_uint32, c_void_p,
                            c_uint32, c_uint32, c_void_p]
k32.DeviceIoControl.argtypes = [c_void_p, c_uint32, c_void_p, c_uint32,
                                c_void_p, c_uint32, ctypes.POINTER(c_uint32), c_void_p]
k32.DeviceIoControl.restype = ctypes.c_int
k32.CloseHandle.argtypes = [c_void_p]


def open_device():
    h = k32.CreateFileW("\\\\.\\HidHide", GENERIC_READ,
                        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                        None, OPEN_EXISTING, 0, None)
    if h == c_void_p(-1).value or not h:
        raise OSError(f"打开 HidHide 失败: {ctypes.get_last_error()}")
    return h


def ioctl(h, code, in_bytes=None, out_size=0):
    in_buf = None
    in_size = 0
    if in_bytes:
        in_buf = ctypes.create_string_buffer(in_bytes, len(in_bytes))
        in_size = len(in_bytes)
    out_buf = ctypes.create_string_buffer(out_size) if out_size else None
    returned = c_uint32(0)
    ok = k32.DeviceIoControl(h, code, in_buf, in_size, out_buf, out_size,
                             byref(returned), None)
    err = ctypes.get_last_error()
    return ok, returned.value, (out_buf.raw if out_buf else None), err


def main():
    h = open_device()
    print("HidHide 设备已打开\n")
    names = {
        "GET_WHITELIST": IOCTL_GET_WHITELIST,
        "GET_BLACKLIST": IOCTL_GET_BLACKLIST,
        "GET_ACTIVE": IOCTL_GET_ACTIVE,
        "GET_WLINVERSE": IOCTL_GET_WLINVERSE,
        "CLEAR_SESSION": IOCTL_CLR_SESSION_BLACKLIST,
    }
    print("=== 只读探测 ===")
    for name, code in names.items():
        size = 1 if "ACTIVE" in name or "INVERSE" in name else 4096
        ok, n, data, err = ioctl(h, code, None, size)
        detail = ""
        if ok and data:
            if size == 1:
                detail = f"value={data[0]}"
            else:
                txt = data[:n].decode("utf-16-le", errors="ignore").strip("\x00")
                detail = f"chars={n//2} items={[x for x in txt.split(chr(0)) if x]}"
        print(f"  {name:16s} code=0x{code:08X} ok={ok} ret={n} err={err} {detail}")

    print("\n=== SET_ACTIVE 往返 ===")
    ok, n, data, err = ioctl(h, IOCTL_GET_ACTIVE, None, 1)
    orig = data[0] if ok and data else None
    print(f"  原始值: {orig}")
    for v in (1, 0):
        ok2, _, _, err2 = ioctl(h, IOCTL_SET_ACTIVE, bytes([v]), 0)
        print(f"  SET_ACTIVE({v}) -> ok={ok2} err={err2}")
        ok3, _, d3, _ = ioctl(h, IOCTL_GET_ACTIVE, None, 1)
        print(f"    回读: {d3[0] if ok3 and d3 else '?'}")
    if orig is not None:
        ioctl(h, IOCTL_SET_ACTIVE, bytes([orig]), 0)
        print(f"  已还原为 {orig}")

    print("\n=== ADD_SESSION_BLACKLIST 探测 ===")
    payload = "HID\\VID_1234&PID_5678\\TEST\0\0".encode("utf-16-le")
    ok, n, _, err = ioctl(h, IOCTL_ADD_SESSION_BLACKLIST, payload, 0)
    print(f"  code=0x{IOCTL_ADD_SESSION_BLACKLIST:08X} ok={ok} err={err}")
    if not ok:
        print(f"  -> 该驱动版本可能不支持 session blacklist（Win32 错误 {err}）")
    ioctl(h, IOCTL_CLR_SESSION_BLACKLIST, None, 0)

    k32.CloseHandle(h)


if __name__ == "__main__":
    main()
