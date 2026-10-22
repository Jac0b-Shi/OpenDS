"""Play a brief quiet 4ch test tone to the virtual DualSense only; never changes defaults."""
import ctypes as c
import argparse
import math
from pathlib import Path
import struct
import time
from ds5_probe import GUID


class Caps(c.Structure):
    _fields_ = [('mid', c.c_ushort), ('pid', c.c_ushort), ('version', c.c_uint),
                ('name', c.c_wchar * 32), ('formats', c.c_uint),
                ('channels', c.c_ushort), ('reserved', c.c_ushort), ('support', c.c_uint)]


class Format(c.Structure):
    _fields_ = [('tag', c.c_ushort), ('channels', c.c_ushort), ('rate', c.c_uint),
                ('average', c.c_uint), ('align', c.c_ushort), ('bits', c.c_ushort),
                ('cb', c.c_ushort), ('valid', c.c_ushort), ('mask', c.c_uint), ('guid', GUID)]


class Header(c.Structure):
    _fields_ = [('data', c.c_void_p), ('length', c.c_uint), ('recorded', c.c_uint),
                ('user', c.c_size_t), ('flags', c.c_uint), ('loops', c.c_uint),
                ('next', c.c_void_p), ('reserved', c.c_size_t)]


winmm = c.WinDLL('winmm')
winmm.waveOutGetDevCapsW.argtypes = [c.c_size_t, c.POINTER(Caps), c.c_uint]
winmm.waveOutOpen.argtypes = [c.POINTER(c.c_void_p), c.c_uint, c.POINTER(Format), c.c_size_t, c.c_size_t, c.c_uint]
for name in ['waveOutPrepareHeader', 'waveOutUnprepareHeader', 'waveOutWrite']:
    getattr(winmm, name).argtypes = [c.c_void_p, c.POINTER(Header), c.c_uint]
winmm.waveOutReset.argtypes = winmm.waveOutClose.argtypes = [c.c_void_p]


def check(code):
    if code:
        raise RuntimeError('WinMM error %d' % code)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--pcm', type=Path, help='Replay up to 10s of raw 4ch S16LE 48kHz PCM')
    args = parser.parse_args()
    if args.pcm:
        data = args.pcm.read_bytes()
        if not data or len(data) % 8 or len(data) > 48000 * 8 * 10:
            raise ValueError('PCM must contain whole 4ch S16LE frames, between 1 frame and 10 seconds')
    else:
        data = b''.join(struct.pack('<hhhh', v, v, 0, 0) for v in
                       (int(1000 * math.sin(2 * math.pi * 660 * i / 48000)) for i in range(12000)))
    devices = []
    for i in range(winmm.waveOutGetNumDevs()):
        caps = Caps()
        if winmm.waveOutGetDevCapsW(i, c.byref(caps), c.sizeof(caps)):
            continue
        if 'DualSense' in caps.name:
            devices.append((i, caps.name))
    if len(devices) != 1:
        raise RuntimeError('Expected exactly one active DualSense audio endpoint, got %r' % devices)
    device, name = devices[0]
    guid = GUID(1, 0, 0x10, (c.c_ubyte * 8)(0x80, 0, 0, 0xaa, 0, 0x38, 0x9b, 0x71))
    fmt = Format(0xfffe, 4, 48000, 48000 * 8, 8, 16, 22, 16, 0x33, guid)
    handle = c.c_void_p()
    check(winmm.waveOutOpen(c.byref(handle), device, c.byref(fmt), 0, 0, 0))
    buffer = c.create_string_buffer(data)
    header = Header(c.cast(buffer, c.c_void_p), len(data), 0, 0, 0, 0, None, 0)
    prepared = False
    try:
        check(winmm.waveOutPrepareHeader(handle, c.byref(header), c.sizeof(header)))
        prepared = True
        check(winmm.waveOutWrite(handle, c.byref(header), c.sizeof(header)))
        deadline = time.monotonic() + len(data) / (48000 * 8) + 3
        while not header.flags & 1:
            if time.monotonic() > deadline:
                raise RuntimeError('Audio write timeout')
            time.sleep(.01)
        print('Played %.0fms / 4ch / S16LE / 48kHz to:' % (len(data) / 384), name)
    finally:
        winmm.waveOutReset(handle)
        if prepared:
            winmm.waveOutUnprepareHeader(handle, c.byref(header), c.sizeof(header))
        winmm.waveOutClose(handle)


if __name__ == '__main__':
    main()
