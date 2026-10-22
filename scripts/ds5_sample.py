"""采样 DualSense 输入报告，统计字段变化，用于验证报告布局。

用法：
    python ds5_sample.py [帧数]

默认采样 120 帧（约 2 秒）。输出每个字节的变化范围，便于识别静态字段与动态字段。
"""

import ctypes
import sys
import time

from ds5_probe import (GUID, SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
                       HIDD_ATTRIBUTES, HIDP_CAPS, enumerate_hid, probe, kernel32)


def open_sony():
    for p in enumerate_hid():
        r = probe(p)
        if not r:
            continue
        if r["vid"] == 0x054C:
            return r
        kernel32.CloseHandle(r["handle"])
    return None


def sample(dev, frames=120):
    size = max(dev["in_len"], 64)
    buf = (ctypes.c_ubyte * size)()
    n = ctypes.c_ulong(0)
    samples = []
    for _ in range(frames):
        ok = kernel32.ReadFile(dev["handle"], buf, size, ctypes.byref(n), None)
        if not ok:
            break
        samples.append(bytes(buf[:n.value]))
        time.sleep(0.001)
    return samples


def analyze(samples):
    if not samples:
        print("没有采到样本")
        return
    length = min(len(s) for s in samples)
    print(f"样本数={len(samples)}  报告长度={length}  ID=0x{samples[0][0]:02X}")
    print("\n偏移  最小  最大  变化次数  当前值   说明")
    print("-" * 62)
    for i in range(length):
        vals = [s[i] for s in samples]
        lo, hi = min(vals), max(vals)
        changes = sum(1 for a, b in zip(vals, vals[1:]) if a != b)
        note = ""
        if i == 0:
            note = "报告 ID"
        elif i in (1, 2):
            note = "左摇杆 X/Y"
        elif i in (3, 4):
            note = "右摇杆 X/Y"
        elif i == 5:
            note = "按键组 0"
        elif i == 6:
            note = "按键组 1"
        elif i == 7:
            note = "按键组 2"
        elif i == 8:
            note = "L2 扳机"
        elif i == 9:
            note = "R2 扳机"
        elif i in (10, 11):
            note = "序号/时间戳"
        print(f"{i:>4}  {lo:>4}  {hi:>4}  {changes:>8}  0x{vals[-1]:02X}   {note}")

    zero = sum(1 for i in range(1, length) if max(s[i] for s in samples) == 0)
    print(f"\n恒为 0 的字节数: {zero} / {length - 1}")


if __name__ == "__main__":
    frames = int(sys.argv[1]) if len(sys.argv) > 1 else 120
    dev = open_sony()
    if not dev:
        print("未找到 Sony HID 设备")
        sys.exit(1)
    mode = "USB" if dev["in_len"] <= 64 else "Bluetooth"
    print(f"设备: VID={dev['vid']:04X} PID={dev['pid']:04X}  "
          f"IN={dev['in_len']} OUT={dev['out_len']}  推断模式={mode}")
    print(f"路径: {dev['path'][:100]}")
    print("采样中，可以拨动摇杆/按按键……\n")
    samples = sample(dev, frames)
    analyze(samples)
    kernel32.CloseHandle(dev["handle"])
