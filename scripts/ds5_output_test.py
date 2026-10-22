"""向虚拟 DualSense 写输出报告，验证「游戏 → 虚拟设备 → 真实手柄」链路。

用法：
    python ds5_output_test.py red      # 灯条设红
    python ds5_output_test.py off      # 关灯
    python ds5_output_test.py rumble   # 震动 1 秒

原理：虚拟设备的 USB 输出报告是 0x02 + 47 字节载荷；libVIIPER 收到后触发
SetDualSenseOutputCallback，桥接程序再转成蓝牙 0x31 写给真实手柄。
"""

import ctypes
import sys
import time
from ctypes import POINTER, byref, c_ubyte, c_ulong, c_void_p

from ds5_probe import enumerate_hid, probe, kernel32

kernel32.WriteFile.argtypes = [c_void_p, c_void_p, c_ulong, POINTER(c_ulong), c_void_p]


def find_virtual():
    """虚拟设备的特征是 IN=64（USB 模式），路径含 vid_054c&pid_0ce6。"""
    for p in enumerate_hid():
        r = probe(p)
        if not r:
            continue
        if r["vid"] == 0x054C and r["in_len"] == 64:
            return r
        kernel32.CloseHandle(r["handle"])
    return None


def write_output(dev, payload: bytearray):
    rpt = bytearray(48)
    rpt[0] = 0x02
    rpt[1:48] = payload
    written = c_ulong(0)
    ok = kernel32.WriteFile(dev["handle"], bytes(rpt), 48, byref(written), None)
    return bool(ok), written.value, ctypes.get_last_error()


def main():
    action = sys.argv[1] if len(sys.argv) > 1 else "red"
    dev = find_virtual()
    if not dev:
        print("未找到虚拟设备（IN=64）。请先运行 opends.exe bridge")
        return 1
    print(f"虚拟设备: IN={dev['in_len']} OUT={dev['out_len']}")

    p = bytearray(47)
    if action == "red":
        p[1] = 1 << 2          # ValidFlag1: ALLOW_LED_COLOR
        p[44], p[45], p[46] = 255, 0, 0
    elif action == "green":
        p[1] = 1 << 2
        p[44], p[45], p[46] = 0, 255, 0
    elif action == "off":
        p[1] = (1 << 2) | (1 << 3)   # ALLOW_LED_COLOR | RESET_LIGHTS
        p[44] = p[45] = p[46] = 0
        p[43] = 0
    elif action == "rumble":
        p[0] = 1      # ValidFlag0: ENABLE_RUMBLE_EMULATION
        p[2] = 200    # 右马达
        p[3] = 200    # 左马达
    elif action == "trigger":
        # 右扳机：恒定阻力（模式 0x01，参数 [0]=力度）
        p[0] = 1 << 2          # ALLOW_RIGHT_TRIGGER_FFB
        p[10] = 0x01           # Resistance
        p[11] = 0x60
    else:
        print(f"未知动作: {action}")
        return 1

    ok, n, err = write_output(dev, p)
    print(f"写入 0x02 报告: ok={ok} written={n} err={err}")

    if action == "rumble":
        print("保持震动 1 秒…")
        for _ in range(20):
            write_output(dev, p)
            time.sleep(0.05)
        p[2] = p[3] = 0
        write_output(dev, p)
        print("已停止")

    kernel32.CloseHandle(dev["handle"])
    return 0


if __name__ == "__main__":
    sys.exit(main())
