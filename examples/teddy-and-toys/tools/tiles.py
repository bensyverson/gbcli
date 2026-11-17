"""Shared helpers: cut an image into 8x8 tiles and encode them as 2bpp.

An image here is a list of rows, each a list of color numbers 0-3 (the
game's, below: tiles.indexed turns shades into them).
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


# The game's color numbers. Pieces are both background tiles (on the belts
# and in the key) and objects (in Elle's hands), from the same tile data. An
# object's color 0 is transparent, so a piece's clear pixels are 0, and the
# background's color 0 is the belts' dark gray, which shows through them:
#     BGP  %11_01_00_10: 0 dark gray, 1 white, 2 light gray, 3 black
#     OBP0 %11_01_00_00: 0 clear,     1 white, 2 light gray, 3 black
# On a GBC each palette keeps that order: 0 the belt (or clear), 1 the
# lightest, 2 the middle, 3 the ink.
WHITE, LIGHT, DARK, BLACK = 0, 1, 2, 3          # shades, as tools/pixels.py reads them
CLEAR = -1                                      # pixels.read(path, clear=CLEAR)
INDEX = {DARK: 0, WHITE: 1, LIGHT: 2, BLACK: 3, CLEAR: 0}


def indexed(img, allow_dark=True, where="image"):
    """Shades (and CLEAR) to the game's color numbers."""
    out = []
    for y, row in enumerate(img):
        for x, s in enumerate(row):
            if s == DARK and not allow_dark:
                raise SystemExit(f"{where}: pixel {x},{y} is dark gray, which an object can't show; "
                                 "use white, light gray, black or clear")
        out.append([INDEX[s] for s in row])
    return out
