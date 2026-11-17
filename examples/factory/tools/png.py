"""Reads the PNGs gbcli writes (palette-indexed, 1-8 bits) into RGB rows.

Only the pixels count: two shots of the same picture compare equal even if
their files carry different metadata.
"""

import struct
import zlib
from pathlib import Path


def _paeth(a, b, c):
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    return b if pb <= pc else c


def read(path):
    """The image as a list of rows of (r, g, b)."""
    data = Path(path).read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", f"{path}: not a PNG"
    pos, idat, palette = 8, b"", None
    while pos < len(data):
        length, kind = struct.unpack(">I4s", data[pos:pos + 8])
        body = data[pos + 8:pos + 8 + length]
        pos += 12 + length
        if kind == b"IHDR":
            width, height, depth, color = struct.unpack(">IIBB", body[:10])
        elif kind == b"PLTE":
            palette = [tuple(body[i:i + 3]) for i in range(0, len(body), 3)]
        elif kind == b"IDAT":
            idat += body
    if color != 3 or palette is None:
        raise ValueError(f"{path}: want a palette PNG (color type 3), got {color}")
    raw = zlib.decompress(idat)
    stride = (width * depth + 7) // 8
    rows, prev, i = [], bytearray(stride), 0
    for _ in range(height):
        kind, line = raw[i], bytearray(raw[i + 1:i + 1 + stride])
        i += 1 + stride
        for x in range(stride):
            a = line[x - 1] if x else 0
            b, c = prev[x], prev[x - 1] if x else 0
            line[x] = (line[x] + (0, a, b, (a + b) // 2, _paeth(a, b, c))[kind]) & 0xFF
        prev = line
        per = 8 // depth
        row = []
        for x in range(width):
            byte = line[x // per]
            shift = 8 - depth * (x % per + 1)
            row.append(palette[(byte >> shift) & ((1 << depth) - 1)])
        rows.append(row)
    return rows
