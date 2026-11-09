"""Minimal PNG reading and writing (8-bit gray, RGB, RGBA or palette), stdlib only."""

import struct
import zlib


def read_png(path):
    """Returns (width, height, rows) with each pixel as an (r, g, b) tuple."""
    with open(path, "rb") as f:
        data = f.read()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("%s: not a PNG" % path)
    pos = 8
    idat = b""
    palette = None
    while pos < len(data):
        length, kind = struct.unpack(">I4s", data[pos:pos + 8])
        body = data[pos + 8:pos + 8 + length]
        pos += 12 + length
        if kind == b"IHDR":
            width, height, depth, ctype, _, _, interlace = struct.unpack(">IIBBBBB", body)
        elif kind == b"PLTE":
            palette = [tuple(body[i:i + 3]) for i in range(0, len(body), 3)]
        elif kind == b"IDAT":
            idat += body
        elif kind == b"IEND":
            break
    channels = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[ctype]
    if interlace or (depth != 8 and not (depth in (1, 2, 4) and channels == 1)):
        raise ValueError("%s: only non-interlaced 8-bit, or 1/2/4-bit gray or palette" % path)
    raw = zlib.decompress(idat)
    stride = (width * channels * depth + 7) // 8
    bpp = max(1, channels * depth // 8)
    rows = []
    prev = bytearray(stride)
    p = 0
    for _ in range(height):
        ftype = raw[p]
        line = bytearray(raw[p + 1:p + 1 + stride])
        p += 1 + stride
        for i in range(stride):
            a = line[i - bpp] if i >= bpp else 0
            b = prev[i]
            c = prev[i - bpp] if i >= bpp else 0
            if ftype == 1:
                line[i] = (line[i] + a) & 0xFF
            elif ftype == 2:
                line[i] = (line[i] + b) & 0xFF
            elif ftype == 3:
                line[i] = (line[i] + ((a + b) >> 1)) & 0xFF
            elif ftype == 4:
                pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                pr = a if pa <= pb and pa <= pc else (b if pb <= pc else c)
                line[i] = (line[i] + pr) & 0xFF
        prev = line
        px = []
        if depth < 8:
            per = 8 // depth
            line = bytearray(
                (line[x // per] >> (8 - depth * (x % per + 1))) & ((1 << depth) - 1)
                for x in range(width))
            if ctype == 0:
                line = bytearray(v * 255 // ((1 << depth) - 1) for v in line)
        for x in range(width):
            v = line[x * channels:(x + 1) * channels]
            if ctype == 0 or ctype == 4:
                px.append((v[0], v[0], v[0]))
            elif ctype == 3:
                px.append(palette[v[0]])
            else:
                px.append((v[0], v[1], v[2]))
        rows.append(px)
    return width, height, rows


def write_png(path, rows):
    """Writes rows of (r, g, b) tuples as an 8-bit RGB PNG."""
    height = len(rows)
    width = len(rows[0])
    raw = bytearray()
    for row in rows:
        raw.append(0)
        for r, g, b in row:
            raw += bytes((r, g, b))

    def chunk(kind, body):
        c = struct.pack(">I", len(body)) + kind + body
        return c + struct.pack(">I", zlib.crc32(kind + body) & 0xFFFFFFFF)

    out = b"\x89PNG\r\n\x1a\n"
    out += chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
    out += chunk(b"IDAT", zlib.compress(bytes(raw), 9))
    out += chunk(b"IEND", b"")
    with open(path, "wb") as f:
        f.write(out)
