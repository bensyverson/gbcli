"""Shared helpers: cut an image into 8x8 tiles and encode them as 2bpp.

An image here is a list of rows, each a list of shades 0 (white) to 3
(black), as the Game Boy draws them with BGP %11_10_01_00.
"""


def split(img, cols, rows):
    """The image's tiles, row by row: cols x rows tiles of 8x8."""
    out = []
    for ty in range(rows):
        for tx in range(cols):
            out.append([img[ty * 8 + y][tx * 8:tx * 8 + 8] for y in range(8)])
    return out


def encode(tile):
    """16 bytes: per row, the low bit plane then the high one."""
    data = []
    for row in tile:
        lo = hi = 0
        for x, shade in enumerate(row):
            bit = 7 - x
            lo |= (shade & 1) << bit
            hi |= (shade >> 1) << bit
        data += [lo, hi]
    return data
