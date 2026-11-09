"""A minimal PNG writer (Python standard library only): 8-bit RGBA rows.

Every art script in tools/ draws into a Canvas and saves it here, so the
PNGs regenerate byte-identical: no timestamps, fixed zlib level.
"""

import struct
import zlib


def _chunk(kind, data):
    body = kind + data
    return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)


class Canvas:
    """A width x height grid of (r, g, b, a) pixels, transparent to start."""

    def __init__(self, width, height, fill=(0, 0, 0, 0)):
        self.width = width
        self.height = height
        self.px = [[fill] * width for _ in range(height)]

    def set(self, x, y, color):
        if 0 <= x < self.width and 0 <= y < self.height:
            self.px[y][x] = color if len(color) == 4 else (*color, 255)

    def rect(self, x, y, w, h, color):
        for yy in range(y, y + h):
            for xx in range(x, x + w):
                self.set(xx, yy, color)

    def save(self, path):
        raw = bytearray()
        for row in self.px:
            raw.append(0)  # filter: none
            for r, g, b, a in row:
                raw += bytes((r, g, b, a))
        png = b"\x89PNG\r\n\x1a\n"
        png += _chunk(b"IHDR", struct.pack(">IIBBBBB", self.width, self.height, 8, 6, 0, 0, 0))
        png += _chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        png += _chunk(b"IEND", b"")
        with open(path, "wb") as f:
            f.write(png)


def hexcolor(s):
    s = s.lstrip("#")
    return (int(s[0:2], 16), int(s[2:4], 16), int(s[4:6], 16), 255)
