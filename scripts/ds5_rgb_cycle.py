"""让手柄灯条红绿蓝交替，用于目视验证输出反馈链路。

需要先运行 opends.exe bridge。
"""

import ctypes
import sys
import time
from ctypes import POINTER, byref, c_ulong, c_void_p

from ds5_probe import enumerate_hid, probe, kernel32

kernel32.WriteFile.argtypes = [c_void_p, c_void_p, c_ulong, POINTER(c_ulong), c_void_p]

COLORS = [
    ("红", (255, 0, 0)),
    ("绿", (0, 255, 0)),
    ("蓝", (0, 0, 255)),
]


def find_virtual():
    for p in enumerate_hid():
        r = probe(p)
        if not r:
            continue
        if r["vid"] == 0x054C and r["in_len"] == 64:
            return r
        kernel32.CloseHandle(r["handle"])
    return None


def write_color(dev, rgb):
    p = bytearray(47)
    p[1] = 1 << 2   # ValidFlag1: ALLOW_LED_COLOR
    p[38] = 1 << 0  # ValidFlag2: ALLOW_BRIGHTNESS_CHANGE
    p[42] = 0       # 亮度：0 高 / 1 中 / 2 低
    p[44], p[45], p[46] = rgb
    rpt = bytearray(48)
    rpt[0] = 0x02
    rpt[1:48] = p
    written = c_ulong(0)
    return bool(kernel32.WriteFile(dev["handle"], bytes(rpt), 48, byref(written), None))


def main():
    seconds = int(sys.argv[1]) if len(sys.argv) > 1 else 30
    dev = find_virtual()
    if not dev:
        print("未找到虚拟设备（IN=64），请先运行 opends.exe bridge")
        return 1

    print(f"灯条循环 {seconds} 秒（红→绿→蓝，每种 2 秒）")
    start = time.time()
    i = 0
    while time.time() - start < seconds:
        name, rgb = COLORS[i % 3]
        # 每种颜色持续 2 秒，期间每 200ms 重发一次，避免手柄超时回落
        t_end = time.time() + 2.0
        while time.time() < t_end and time.time() - start < seconds:
            write_color(dev, rgb)
            time.sleep(0.2)
        print(f"  {name}  {rgb}")
        i += 1

    # 收尾：关灯
    p = bytearray(47)
    p[1] = (1 << 2) | (1 << 3)
    p[44] = p[45] = p[46] = 0
    rpt = bytearray(48)
    rpt[0] = 0x02
    rpt[1:48] = p
    written = c_ulong(0)
    kernel32.WriteFile(dev["handle"], bytes(rpt), 48, byref(written), None)
    print("已关灯")
    kernel32.CloseHandle(dev["handle"])
    return 0


if __name__ == "__main__":
    sys.exit(main())
