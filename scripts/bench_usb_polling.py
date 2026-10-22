"""Compare DLL USB/IP input scheduling locally, without attaching a Windows device.

Feeds input at 250 Hz, then reads EP4 as quickly as the virtual USB server allows.
Does not access the physical controller or the running OpenDS bridge.
"""
import argparse
import ctypes as c
import hashlib
import json
import os
from pathlib import Path
import socket
import struct
import threading
import time
import viiper_probe as probe


def receive(sock, count):
    chunks = bytearray()
    while len(chunks) < count:
        part = sock.recv(count - len(chunks))
        if not part:
            raise RuntimeError('USB/IP connection closed')
        chunks.extend(part)
    return bytes(chunks)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dll', type=Path, required=True)
    parser.add_argument('--seconds', type=int, default=10, choices=range(1, 61))
    parser.add_argument('--port', type=int, default=3247)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    probe.DLL = str(args.dll.resolve())
    dll = probe.load()
    dll.UseDualSensePhysicalUSBProfile.argtypes = [c.c_uint64]
    dll.UseDualSensePhysicalUSBProfile.restype = c.c_uint8
    config = probe.USBServerConfig(f'127.0.0.1:{args.port}'.encode(), 30000, 5000, 1)
    server, bus, device = c.c_uint64(), c.c_uint32(), c.c_uint64()
    stop = threading.Event()
    producer = None
    assert dll.NewUSBServer(c.byref(config), c.byref(server), None)
    try:
        assert dll.CreateUSBBus(server, c.byref(bus))
        assert dll.CreateDualSenseDevice(server, c.byref(device), bus, 0, 0x054c, 0x0ce6, None)
        assert dll.UseDualSensePhysicalUSBProfile(device)
        frame = (c.c_uint8 * 64)()
        frame[0] = 1
        frame[1] = frame[2] = frame[3] = frame[4] = 128

        def feed():
            deadline = time.perf_counter()
            while not stop.is_set():
                frame[7] = (frame[7] + 1) & 255
                if not dll.SetDualSenseRawInputReport(device, frame, 64):
                    stop.set()
                    return
                deadline += .004
                stop.wait(max(0, deadline - time.perf_counter()))

        producer = threading.Thread(target=feed, daemon=True)
        producer.start()
        with socket.create_connection(('127.0.0.1', args.port), timeout=3) as sock:
            sock.sendall(struct.pack('!HHI32s', 0x111, 0x8003, 0, f'{bus.value}-1'.encode()))
            version, command, status = struct.unpack('!HHI', receive(sock, 8))
            assert command == 3 and status == 0, (version, command, status)
            receive(sock, 312)
            start, cpu_start = time.perf_counter(), sum(os.times()[:2])
            count = 0
            while time.perf_counter() - start < args.seconds:
                count += 1
                sock.sendall(struct.pack('!IIIIIiiiii8s', 1, count, 0x10001, 1, 4, 0, 64, 0, 0, 0, bytes(8)))
                header = receive(sock, 48)
                status, length = struct.unpack_from('!iI', header, 20)
                assert status == 0 and length == 64, (status, length)
                assert receive(sock, length)[0] == 1
            elapsed = time.perf_counter() - start
            result = {'dll': str(args.dll.resolve()), 'sha256': hashlib.sha256(args.dll.read_bytes()).hexdigest(),
                      'seconds': elapsed, 'input_completions': count, 'completions_per_second': count / elapsed,
                      'process_cpu_core_percent': 100 * (sum(os.times()[:2]) - cpu_start) / elapsed,
                      'windows_device_attached': False}
            if args.output:
                args.output.write_text(json.dumps(result, indent=2), encoding='utf-8')
            print(json.dumps(result, indent=2))
    finally:
        stop.set()
        if producer:
            producer.join(timeout=3)
        if device.value:
            dll.RemoveDualSenseDevice(device)
        dll.CloseUSBServer(server)


if __name__ == '__main__':
    main()
