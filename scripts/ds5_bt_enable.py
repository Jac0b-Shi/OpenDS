"""向蓝牙 DualSense 发送 0x31 输出报告，切换到手柄的完整报告模式。

依据逆向结论（FUN_140006c20）：
    rpt[0] = 0x31, rpt[1] = 0x10, 总长 78，末尾 4 字节 CRC32
    CRC 覆盖前 74 字节（0..73），小端放在 74..77

============================ 重要警告 ============================
本脚本会**永久改变手柄的工作模式**（直到手柄断电或重新配对）：

  默认模式 0x01（78 字节，DS4 兼容）  ←→  完整模式 0x31（78 字节）

切到 0x31 后，**游戏和 Steam 会认不出这份报告**，表现为：
  - 手柄输入完全进不去游戏
  - 系统持续收到无法解析的高频输入，导致鼠标被锁在手柄上

已知的恢复方式（按可靠度排序）：
  1. 关闭手柄电源（长按 PS 约 10 秒）再开机   ← 最可靠
  2. Windows 蓝牙设置里删除设备后重新配对
  3. 电脑重启

因此：**执行前请确认当前没有在玩游戏**。

用 --read-only 可以只读查看当前模式，不发送任何数据：

    python ds5_bt_enable.py --read-only
==================================================================
"""

import ctypes
import sys
import time
import zlib

from ds5_probe import enumerate_hid, probe, kernel32, GENERIC_READ, GENERIC_WRITE, \
    FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, INVALID_HANDLE_VALUE

kernel32.WriteFile.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_ulong,
                               ctypes.POINTER(ctypes.c_ulong), ctypes.c_void_p]


def build_enable_report():
    rpt = bytearray(78)
    rpt[0] = 0x31
    rpt[1] = 0x10          # 逆向得到的标志值
    crc = zlib.crc32(bytes(rpt[0:74])) & 0xFFFFFFFF
    rpt[74:78] = crc.to_bytes(4, "little")
    return bytes(rpt)


def find_bt():
    for p in enumerate_hid():
        r = probe(p)
        if not r:
            continue
        if r["vid"] == 0x054C and r["in_len"] >= 70:
            return r
        kernel32.CloseHandle(r["handle"])
    return None


def main():
    dev = find_bt()
    if not dev:
        print("未找到蓝牙 DualSense（IN>=70）")
        return 1
    print(f"蓝牙设备: VID={dev['vid']:04X} PID={dev['pid']:04X} IN={dev['in_len']} OUT={dev['out_len']}")

    # 先看当前报告 ID
    size = dev["in_len"]
    buf = (ctypes.c_ubyte * size)()
    n = ctypes.c_ulong(0)
    kernel32.ReadFile(dev["handle"], buf, size, ctypes.byref(n), None)
    current_id = buf[0]
    print(f"当前: len={n.value} ID=0x{current_id:02X}")

    # 只读模式：仅报告状态，不写设备
    if "--read-only" in sys.argv:
        mode = "默认模式（游戏可用）" if current_id == 0x01 else "完整模式（游戏会异常）"
        print(f"状态: {mode}")
        if current_id != 0x01:
            print("恢复方法: 关闭手柄电源再开机，或在蓝牙设置里重新配对。")
        kernel32.CloseHandle(dev["handle"])
        return 0

    # 写入前二次确认
    print("\n!! 即将把报告模式改为 0x31 —— 游戏会认不出输入，鼠标可能被锁在手柄上")
    print("!! 恢复方式：关闭手柄电源再开机")
    print("!! 如只想查看状态，请改用 --read-only\n")

    # 发送启用报告
    rpt = build_enable_report()
    written = ctypes.c_ulong(0)
    ok = kernel32.WriteFile(dev["handle"], rpt, len(rpt), ctypes.byref(written), None)
    err = ctypes.get_last_error()
    print(f"发送 0x31 报告: ok={bool(ok)} written={written.value} err={err}")

    time.sleep(0.3)

    # 再读，看 ID 是否变成 0x31
    for i in range(5):
        if not kernel32.ReadFile(dev["handle"], buf, size, ctypes.byref(n), None):
            print(f"  读取失败 err={ctypes.get_last_error()}")
            break
        data = bytes(buf[:n.value])
        print(f"  第{i+1}帧: len={len(data)} ID=0x{data[0]:02X} buf[1]=0x{data[1]:02X} "
              f"前12={(' '.join(f'{b:02X}' for b in data[:12]))}")

    print("\n提醒: 手柄现在处于完整模式，游戏会认不出输入。")
    print("     用完请关闭手柄电源再开机，恢复默认模式。")

    kernel32.CloseHandle(dev["handle"])
    return 0


if __name__ == "__main__":
    sys.exit(main())
