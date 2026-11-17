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


def _chunk(kind, body):
    return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))


def write(path, rows, scale=1):
    """Writes rows of (r, g, b) as an RGB PNG, each pixel scale x scale."""
    height, width = len(rows) * scale, len(rows[0]) * scale
    raw = bytearray()
    for row in rows:
        line = bytearray(b"\x00")
        for px in row:
            line += bytes(px) * scale
        raw += line * scale
    data = (b"\x89PNG\r\n\x1a\n"
            + _chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
            + _chunk(b"IDAT", zlib.compress(bytes(raw), 9))
            + _chunk(b"IEND", b""))
    Path(path).parent.mkdir(parents=True, exist_ok=True)
    Path(path).write_bytes(data)


def hstack(images, gap=2, background=(96, 96, 96)):
    """Images (rows of (r, g, b)) side by side, top-aligned, gap pixels apart."""
    height = max(len(img) for img in images)
    out = [[] for _ in range(height)]
    for n, img in enumerate(images):
        if n:
            for row in out:
                row.extend([background] * gap)
        width = len(img[0])
        for y in range(height):
            out[y].extend(img[y] if y < len(img) else [background] * width)
    return out


def vstack(images, gap=2, background=(96, 96, 96)):
    """Images one above another, left-aligned, gap pixels apart."""
    width = max(len(img[0]) for img in images)
    out = []
    for n, img in enumerate(images):
        if n:
            out.extend([[background] * width for _ in range(gap)])
        out.extend(row + [background] * (width - len(row)) for row in img)
    return out
