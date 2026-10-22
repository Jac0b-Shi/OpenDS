"""Read virtual motion; --pulse tests a brief rumble through the virtual HID output.

Never writes physical HID directly. --pulse always sends a zero-motor report on exit.
Acceleration variance is diagnostic evidence, not a substitute for a user's feel.
"""
import argparse
import ctypes
import json
import math
import statistics
import struct
import time
from ds5_probe import enumerate_hid, probe, kernel32, hid

hid.HidD_GetInputReport.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_ulong]
hid.HidD_GetInputReport.restype = ctypes.c_ubyte
kernel32.WriteFile.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_ulong,
                              ctypes.POINTER(ctypes.c_ulong), ctypes.c_void_p]


def sample(handle):
    buf = ctypes.create_string_buffer(64)
    buf[0] = 1
    if not hid.HidD_GetInputReport(handle, buf, 64):
        raise ctypes.WinError(ctypes.get_last_error())
    return {'gyro': struct.unpack_from('<hhh', buf.raw, 16),
            'accel': struct.unpack_from('<hhh', buf.raw, 22),
            'timestamp': struct.unpack_from('<I', buf.raw, 28)[0]}


def motors(handle, strength, v2):
    report = bytearray(48)
    report[0] = 2
    report[1] = 2 if v2 else 3
    report[3] = report[4] = strength
    report[39] = 4 if v2 else 0
    written = ctypes.c_ulong()
    if not kernel32.WriteFile(handle, bytes(report), len(report), ctypes.byref(written), None):
        raise ctypes.WinError(ctypes.get_last_error())
    if written.value != len(report):
        raise RuntimeError('short output report')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--pulse', action='store_true')
    parser.add_argument('--v2', action='store_true', help='send the v2 selector directly')
    args = parser.parse_args()
    device = None
    for path in enumerate_hid():
        candidate = probe(path)
        if not candidate:
            continue
        if candidate['vid'] == 0x054c and candidate['pid'] in (0x0ce6, 0x0df2) and candidate['in_len'] == 64:
            device = candidate
            break
        kernel32.CloseHandle(candidate['handle'])
    if not device:
        raise RuntimeError('No virtual USB DualSense found; start the bridge first')
    handle = device['handle']
    try:
        results = []
        for name, strength in [('baseline', 0), ('pulse', 96), ('after', 0)] if args.pulse else [('motion', 0)]:
            frames = []
            for _ in range(24):
                if args.pulse:
                    motors(handle, strength, args.v2)
                frames.append(sample(handle))
                time.sleep(.025)
            rms = math.sqrt(sum(statistics.pvariance([f['accel'][axis] for f in frames]) for axis in range(3)))
            results.append({'phase': name, 'accel_variation_rms': round(rms, 2),
                            'distinct_sensor_timestamps': len(set(f['timestamp'] for f in frames)),
                            'first': frames[0], 'last': frames[-1]})
        print(json.dumps(results, indent=2), flush=True)
    finally:
        if args.pulse:
            motors(handle, 0, args.v2)
        kernel32.CloseHandle(handle)


if __name__ == '__main__':
    main()
