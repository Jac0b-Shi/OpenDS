"""Summarize Sony USBPcap startup control transfers without device I/O."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import struct


def packets(raw):
    if len(raw) < 24 or raw[:4] != b'\xd4\xc3\xb2\xa1' or struct.unpack_from('<I', raw, 20)[0] != 249:
        raise ValueError('Expected classic little-endian USBPcap pcap')
    off = 24
    while off + 16 <= len(raw):
        sec, micro, size, _ = struct.unpack_from('<IIII', raw, off)
        off += 16
        if off + size > len(raw):
            break
        p = memoryview(raw)[off:off + size]
        off += size
        if len(p) < 27:
            continue
        hlen = struct.unpack_from('<H', p)[0]
        length = struct.unpack_from('<I', p, 23)[0]
        if not 27 <= hlen <= len(p) or hlen + length > len(p):
            continue
        yield sec + micro / 1e6, p, p[hlen:hlen + length]


def analyze(path):
    raw = path.read_bytes()
    devices = {}
    for _, p, d in packets(raw):
        if p[22] == 2 and p[16] & 1 and len(d) >= 18 and bytes(d[:2]) == b'\x12\x01':
            vid, pid = struct.unpack_from('<HH', d, 8)
            if vid == 0x054c:
                devices[(struct.unpack_from('<H', p, 17)[0], struct.unpack_from('<H', p, 19)[0])] = [vid, pid]
    pending, queries, features, descriptors, writes = {}, Counter(), {}, {}, []
    statuses, endpoints = Counter(), Counter()
    times = []
    for now, p, d in packets(raw):
        key = (struct.unpack_from('<H', p, 17)[0], struct.unpack_from('<H', p, 19)[0])
        if key not in devices:
            continue
        times.append(now)
        name = '%d:%d' % key
        ep, kind, completion = p[21], p[22], bool(p[16] & 1)
        endpoints[(name, ep, kind, completion)] += 1
        if kind != 2:
            continue
        irp = (key, struct.unpack_from('<Q', p, 2)[0])
        if not completion and len(d) >= 8:
            req = struct.unpack_from('<BBHHH', d)
            pending[irp] = (req, now)
            queries[(name, req)] += 1
            if req[0] == 0x21 and req[1] == 9:
                writes.append({'device': name, 'time': now, 'setup': list(req), 'hex': bytes(d[8:]).hex()})
        elif completion and irp in pending:
            req, start = pending.pop(irp)
            status = struct.unpack_from('<I', p, 10)[0]
            statuses[(name, req, status)] += 1
            if req[0] == 0xa1 and req[1] == 1:
                features.setdefault(name + ':%04x' % req[2], []).append({
                    'time': start, 'length': len(d), 'hex': bytes(d).hex(), 'status': status})
            if req[1] == 6:
                descriptors[name + ':%02x-%04x-%04x' % (req[0], req[2], req[3])] = bytes(d).hex()
    return {'file': str(path.resolve()), 'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest(),
            'devices': {'%d:%d' % k: v for k, v in devices.items()},
            'start': min(times) if times else None, 'end': max(times) if times else None,
            'queries': [{'device': name, 'setup': list(req), 'count': n} for (name, req), n in queries.items()],
            'statuses': [{'device': name, 'setup': list(req), 'status': status, 'count': n}
                         for (name, req, status), n in statuses.items()],
            'features': features, 'descriptors': descriptors, 'feature_writes': writes,
            'endpoints': [{'device': name, 'ep': ep, 'type': kind, 'completion': completion, 'count': n}
                          for (name, ep, kind, completion), n in endpoints.items()]}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('pcap', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    report = analyze(args.pcap)
    args.output.write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps({k: report[k] for k in ('file', 'bytes', 'devices')}, indent=2))
    print('Feature response counts:', {k: len(v) for k, v in report['features'].items()})
    print('Failed controls:', [s for s in report['statuses'] if s['status']])
