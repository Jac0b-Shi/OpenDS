"""Generate the small OpenDS controller icon using only the Python standard library."""
import struct
from pathlib import Path

def rounded(x, y, left, top, right, bottom, radius):
    dx = max(left + radius - x, 0, x - (right - radius))
    dy = max(top + radius - y, 0, y - (bottom - radius))
    return dx * dx + dy * dy <= radius * radius

def bitmap(size):
    pixels = bytearray()
    for row in range(size - 1, -1, -1):
        for col in range(size):
            sums = [0, 0, 0, 0]
            for ay in range(4):
                for ax in range(4):
                    x, y = (col + (ax + .5) / 4) * 32 / size, (row + (ay + .5) / 4) * 32 / size
                    if not rounded(x, y, 0, 0, 32, 32, 8):
                        continue
                    color = (225, 95, 42, 255)
                    if rounded(x, y, 5, 10, 27, 23, 5):
                        color = (255, 255, 255, 255)
                    if (9 <= x <= 15 and 15 <= y <= 17) or (11 <= x <= 13 and 13 <= y <= 19):
                        color = (225, 95, 42, 255)
                    if (x - 21) ** 2 + (y - 14.5) ** 2 < 1.6 or (x - 23) ** 2 + (y - 18) ** 2 < 1.6:
                        color = (225, 95, 42, 255)
                    for i in range(4):
                        sums[i] += color[i]
            alpha = sums[3] / 16
            pixels.extend(round(v / (alpha / 255 * 16)) if alpha and i < 3 else round(v / 16) for i, v in enumerate(sums))
    mask = bytes(((size + 31) // 32) * 4 * size)
    header = struct.pack('<IIIHHIIIIII', 40, size, size * 2, 1, 32, 0, len(pixels), 0, 0, 0, 0)
    return header + pixels + mask

if __name__ == '__main__':
    sizes = (32, 64)
    blocks = [bitmap(size) for size in sizes]
    offset = 6 + 16 * len(sizes)
    entries = []
    for size, block in zip(sizes, blocks):
        entries.append(struct.pack('<BBBBHHII', size, size, 0, 0, 1, 32, len(block), offset))
        offset += len(block)
    dest = Path(__file__).resolve().parents[1] / 'assets' / 'opends.ico'
    dest.write_bytes(struct.pack('<HHH', 0, 1, len(sizes)) + b''.join(entries) + b''.join(blocks))
