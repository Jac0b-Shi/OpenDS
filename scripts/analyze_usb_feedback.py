"""Read a classic USBPcap pcap locally; summarize DualSense HID and 4ch PCM.

No device I/O. ISO OUT payloads occur on submissions, not completions:
https://desowin.org/usbpcap/captureformat.html
"""
import argparse
import array
from collections import Counter
import hashlib
import json
import math
from pathlib import Path
import struct
import sys


def analyze(path, device):
    raw = path.read_bytes()
    if raw[:4] != b'\xd4\xc3\xb2\xa1' or len(raw) < 24:
        raise ValueError('Expected little-endian, microsecond classic pcap')
    if struct.unpack_from('<I', raw, 20)[0] != 249:
        raise ValueError('Expected LINKTYPE_USBPCAP (249)')
    endpoints, lengths = Counter(), Counter()
    hid, features = [], []
    pcm_packets = pcm_frames = nonzero_packets = 0
    squares, peaks, active = [0] * 4, [0] * 4, [0] * 4
    seconds = {}
    first = last = None
    off = 24
    truncated = False
    while off + 16 <= len(raw):
        ts, us, size, original = struct.unpack_from('<IIII', raw, off)
        off += 16
        if off + size > len(raw):
            truncated = True
            break
        packet = memoryview(raw)[off:off + size]
        off += size
        if len(packet) < 27:
            continue
        hlen = struct.unpack_from('<H', packet)[0]
        dev = struct.unpack_from('<H', packet, 19)[0]
        if dev != device or not 27 <= hlen <= len(packet):
            continue
        ep, kind, info = packet[21], packet[22], packet[16]
        length = struct.unpack_from('<I', packet, 23)[0]
        if hlen + length > len(packet):
            truncated = True
            continue
        payload = packet[hlen:hlen + length]
        now = ts + us / 1e6
        first = now if first is None else first
        last = now
        endpoints[(ep, kind, bool(info & 1))] += 1
        if ep == 3 and kind == 1 and length:
            hid.append({'time': now, 'length': length, 'hex': payload.hex()})
        if ep & 0x7f == 0 and kind == 2 and length and not info & 1:
            features.append({'time': now, 'hex': payload[:80].hex()})
        if ep != 1 or kind != 0 or info & 1 or not length:
            continue
        lengths[length] += 1
        if length % 8:
            continue
        samples = array.array('h')
        samples.frombytes(payload)
        if sys.byteorder != 'little':
            samples.byteswap()
        frames = len(samples) // 4
        pcm_frames += frames
        pcm_packets += 1
        nonzero_packets += any(samples)
        second = int(now - first)
        row = seconds.setdefault(second, [0] * 4)
        for ch in range(4):
            values = samples[ch::4]
            peak = max(map(abs, values), default=0)
            peaks[ch] = max(peaks[ch], peak)
            squares[ch] += sum(v * v for v in values)
            active[ch] += peak != 0
            row[ch] = max(row[ch], peak)
    return {
        'file': str(path.resolve()), 'bytes': len(raw),
        'sha256': hashlib.sha256(raw).hexdigest(), 'device': device,
        'duration_seconds': round(last - first, 3) if first is not None else 0,
        'truncated': truncated,
        'endpoints': [{'ep': ep, 'transfer': kind, 'completion': completion, 'packets': n}
                      for (ep, kind, completion), n in endpoints.items()],
        'hid_output': hid, 'control_setups': features,
        'pcm_packets': pcm_packets, 'pcm_frames': pcm_frames,
        'pcm_nonzero_packets': nonzero_packets,
        'pcm_batch_lengths': dict(lengths), 'channel_peak': peaks,
        'channel_rms': [round(math.sqrt(v / pcm_frames), 3) if pcm_frames else 0 for v in squares],
        'channel_active_packets': active, 'second_channel_peaks': seconds,
    }


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('pcap', type=Path)
    parser.add_argument('--device', type=int, required=True)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    result = analyze(args.pcap, args.device)
    report = json.dumps(result, indent=2)
    if args.output:
        args.output.write_text(report, encoding='utf-8')
    print(json.dumps({k: v for k, v in result.items()
                      if k not in ('control_setups', 'second_channel_peaks')}, indent=2))
