#!/usr/bin/env python3
"""Draws every image of Vesper Swarm into assets/, with the standard library
only (PNG by zlib and struct). Deterministic: rerunning it rewrites the same
bytes. Every image is indexed art: assets/palettes.txt assigns each its
palette (the same colors as PALETTES here), so one set of tiles serves the
DMG and, colored by those palettes, the GBC.

    python3 -I tools/draw_art.py

assets/bg/sky0-3.png  the parallax landscape's four bands, 256 px wide,
                      each tiling every 64 px (32 for the ground) so it
                      can scroll on its own and wrap
assets/bg/title.png   the title screen, 160x144
assets/tiles/*.png    8x8 tiles loaded in a fixed order: the
                      font, the ship, the enemies, the bullet, the boss.
                      Every tile in a sheet is distinct, so none is merged
                      away and the tile numbers in include/game.inc hold.
"""

import pathlib
import struct
import zlib

ROOT = pathlib.Path(__file__).resolve().parent.parent
ASSETS = ROOT / "assets"


def png(path, rows):
    """Writes rows of (r, g, b) tuples as an 8-bit RGB PNG."""
    h, w = len(rows), len(rows[0])
    raw = b"".join(b"\x00" + bytes(c for px in row for c in px) for row in rows)

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    data = (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def rgb(s):
    s = s.lstrip("#")
    return tuple(int(s[i:i + 2], 16) for i in (0, 2, 4))


class Canvas:
    def __init__(self, w, h, fill):
        self.w, self.h = w, h
        self.px = [[fill] * w for _ in range(h)]

    def set(self, x, y, c):
        if 0 <= x < self.w and 0 <= y < self.h:
            self.px[y][x] = c

    def rect(self, x, y, w, h, c):
        for yy in range(y, y + h):
            for xx in range(x, x + w):
                self.set(xx, yy, c)

    def grid(self, x, y, lines, colors):
        """Paints a picture given as strings; '.' leaves the pixel alone."""
        for dy, line in enumerate(lines):
            for dx, ch in enumerate(line):
                if ch != ".":
                    self.set(x + dx, y + dy, colors[ch])

    def save(self, rel):
        png(ASSETS / rel, self.px)


# --- Indexed palettes, lightest first (index 0 is an object's transparent).
# They are repeated in assets/palettes.txt, which assigns them.
PALETTES = {
    "text": ["#f8f0d8", "#c8a860", "#5c3c54", "#0c1028"],
    "ship": ["#ffffff", "#80b0f0", "#2858b0", "#081838"],
    "enemy": ["#ffffff", "#ffd860", "#d04020", "#300808"],
    "shot": ["#ffffff", "#f060a0", "#a02060", "#300818"],
    "boss": ["#ffffff", "#d0f080", "#7040b0", "#180828"],
    "stars": ["#f0f0ff", "#9098d0", "#404870", "#0c1028"],
    "peaks": ["#e8ecf8", "#8a90c8", "#3a3f6a", "#0c1028"],
    "forest": ["#e0f8c8", "#80c070", "#305838", "#0c1810"],
    "ground": ["#f0e0c0", "#c09868", "#604030", "#1c1008"],
    "title": ["#fff0c0", "#f0a030", "#803060", "#0c1028"],
}


def sheet(name, tiles, palette):
    """A row of 8x8 tiles from 8-line pictures in digits 0-3."""
    cols = [rgb(c) for c in PALETTES[palette]]
    c = Canvas(8 * len(tiles), 8, cols[0])
    for i, t in enumerate(tiles):
        assert len(t) == 8 and all(len(r) == 8 for r in t), (name, i)
        c.grid(8 * i, 0, t, {str(k): cols[k] for k in range(4)})
    c.save(f"tiles/{name}.png")


def split16(rows):
    """A 16x8 picture as two 8x8 tiles, left then right."""
    return [[r[:8] for r in rows], [r[8:] for r in rows]]


def split24(rows):
    """A 24x24 picture as nine tiles, row by row."""
    return [[r[8 * tx:8 * tx + 8] for r in rows[8 * ty:8 * ty + 8]] for ty in range(3) for tx in range(3)]


# --- The font: 5x7 glyphs, light (0) on dark (3); tile 0 is the space.
GLYPHS = {
    " ": [0, 0, 0, 0, 0, 0, 0],
    "0": [14, 17, 19, 21, 25, 17, 14], "1": [4, 12, 4, 4, 4, 4, 14],
    "2": [14, 17, 1, 2, 4, 8, 31], "3": [30, 1, 1, 14, 1, 1, 30],
    "4": [2, 6, 10, 18, 31, 2, 2], "5": [31, 16, 30, 1, 1, 17, 14],
    "6": [6, 8, 16, 30, 17, 17, 14], "7": [31, 1, 2, 4, 8, 8, 8],
    "8": [14, 17, 17, 14, 17, 17, 14], "9": [14, 17, 17, 15, 1, 2, 12],
    "A": [14, 17, 17, 31, 17, 17, 17], "B": [30, 17, 17, 30, 17, 17, 30],
    "C": [14, 17, 16, 16, 16, 17, 14], "D": [28, 18, 17, 17, 17, 18, 28],
    "E": [31, 16, 16, 30, 16, 16, 31], "F": [31, 16, 16, 30, 16, 16, 16],
    "G": [14, 17, 16, 23, 17, 17, 15], "H": [17, 17, 17, 31, 17, 17, 17],
    "I": [14, 4, 4, 4, 4, 4, 14], "J": [7, 2, 2, 2, 2, 18, 12],
    "K": [17, 18, 20, 24, 20, 18, 17], "L": [16, 16, 16, 16, 16, 16, 31],
    "M": [17, 27, 21, 21, 17, 17, 17], "N": [17, 17, 25, 21, 19, 17, 17],
    "O": [14, 17, 17, 17, 17, 17, 14], "P": [30, 17, 17, 30, 16, 16, 16],
    "Q": [14, 17, 17, 17, 21, 18, 13], "R": [30, 17, 17, 30, 20, 18, 17],
    "S": [15, 16, 16, 14, 1, 1, 30], "T": [31, 4, 4, 4, 4, 4, 4],
    "U": [17, 17, 17, 17, 17, 17, 14], "V": [17, 17, 17, 17, 17, 10, 4],
    "W": [17, 17, 17, 21, 21, 21, 10], "X": [17, 17, 10, 4, 10, 17, 17],
    "Y": [17, 17, 10, 4, 4, 4, 4], "Z": [31, 1, 2, 4, 8, 16, 31],
    "x": [0, 0, 17, 10, 4, 10, 17], "-": [0, 0, 0, 31, 0, 0, 0],
    ":": [0, 12, 12, 0, 12, 12, 0], ".": [0, 0, 0, 0, 0, 12, 12],
    "!": [4, 4, 4, 4, 4, 0, 4], "^": [0, 4, 14, 31, 31, 10, 0],
}
FONT_ORDER = " 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZx-:.!^"


def font():
    tiles = []
    for ch in FONT_ORDER:
        rows = ["3" * 8]
        for bits in GLYPHS[ch]:
            # The glyph in light (0) with a mid (2) shadow to its lower right.
            rows.append("".join("0" if 1 <= x <= 5 and bits >> (5 - x) & 1 else "3" for x in range(8)))
        shadowed = [list(r) for r in rows]
        for y in range(8):
            for x in range(8):
                if rows[y][x] == "3" and y > 0 and x > 0 and rows[y - 1][x - 1] == "0":
                    shadowed[y][x] = "2"
        tiles.append(["".join(r) for r in shadowed])
    # The space tile must differ from the window's other dark tile: none
    # exists, so it stays all 3.
    sheet("font", tiles, "text")


SHIP = [
    "...333..........",
    "..31113333......",
    ".3111222223333..",
    "3322222221111133",
    "3322222222222233",
    ".3111222223333..",
    "..31113333......",
    "...333..........",
]
SHOT = ["........", "........", "..333333", ".3211113", ".3211113", "..333333", "........", "........"]
DART = [
    "..........3333..",
    "......33332223..",
    "...3332222223...",
    "33221122222333..",
    "33221122222333..",
    "...3332222223...",
    "......33332223..",
    "..........3333..",
]
WAVE = [
    "....33333333....",
    "..332222222233..",
    ".32211222222223.",
    "3221111223322223",
    "3221111223322223",
    ".32211222222223.",
    "..332222222233..",
    "....33333333....",
]
POD = [
    "..333333333333..",
    ".32222222222223.",
    "3221133333311223",
    "3221312112131223",
    "3221312112131223",
    "3221133333311223",
    ".32222222222223.",
    "..333333333333..",
]
BOOM1 = ["3..22..3", ".3.11.3.", "..2112..", "21100112", "21100112", "..2112..", ".3.11.3.", "3..22..3"]
BOOM2 = ["2......2", "...33...", "..3..3..", ".3....3.", ".3....3.", "..3..3..", "...33...", "2......2"]
BULLET = ["........", "..3333..", ".322223.", ".321123.", ".321123.", ".322223.", "..3333..", "........"]
BOSS = [
    ".......33333333.........",
    ".....332222222233.......",
    "...3322111111122233.....",
    "..322111222221112223....",
    ".32211223333322112223...",
    ".32112233111332211223...",
    "3221123311111332112233..",
    "3211223111111133211223..",
    "3211223111111113321122333333",
    "3211223111111113321122222223",
    "3211223111111133211223111123",
    "3221123311111332112233333333",
    ".32112233111332211223...",
    ".32211223333322112223...",
    "..322111222221112223....",
    "...3322111111122233.....",
    ".....332222222233.......",
    "..3333.3333333333.......",
    ".322223.................",
    "3221122333333333333.....",
    "3221122222222222223.....",
    ".322223.................",
    "..3333..................",
    "........................",
]


def light(rows):
    """Swaps the light and mid colors: bodies drawn light read over every band."""
    return [r.translate(str.maketrans("12", "21")) for r in rows]


def sprites():
    sheet("ship", split16(light(SHIP)) + [SHOT], "ship")
    sheet("enemies", split16(light(DART)) + split16(light(WAVE)) + split16(light(POD)) + [BOOM1, BOOM2], "enemy")
    sheet("bullet", [BULLET], "shot")
    boss = light([(r + "." * 24)[:24] for r in BOSS])
    sheet("boss", split24(boss), "boss")


# --- The backgrounds, indexed art too: the sky is four images, one a band,
# so on the GBC each band has its own palette; the title is one image.
# Every palette's colors are lightest first and fall one to each DMG shade.
def pal(name):
    return [rgb(c) for c in PALETTES[name]]


STAR, DIM, _, NIGHT = pal("stars")
SNOW, MTN_LIT, MTN, _ = pal("peaks")
LEAF, HILL_LIT, HILL_BACK, HILL = pal("forest")
DIRT_LIT, DIRT, DIRT_DARK, ROCK = pal("ground")


def lcg(seed):
    while True:
        seed = (seed * 1103515245 + 12345) & 0x7FFFFFFF
        yield seed >> 8


STAR_TILES = [
    [(3, 3, STAR)], [(5, 1, DIM)], [(1, 5, DIM), (6, 2, DIM)],
    [(4, 2, STAR), (3, 2, DIM), (5, 2, DIM), (4, 1, DIM), (4, 3, DIM)], [(2, 6, STAR)],
]


def starfield(c, x0, y0, cols, rows, seed, density=3):
    r = lcg(seed)
    for ty in range(rows):
        for tx in range(cols):
            v = next(r)
            if v % density == 0:
                for sx, sy, col in STAR_TILES[(v >> 4) % len(STAR_TILES)]:
                    c.set(x0 + tx * 8 + sx, y0 + ty * 8 + sy, col)


def sky():
    c = Canvas(256, 136, NIGHT)
    # Band 0, lines 0-39: stars, a pattern of 8 tiles repeated.
    starfield(c, 0, 0, 8, 5, 7)
    for x in range(64, 256):
        for y in range(40):
            c.px[y][x] = c.px[y][x - 64]
    # Band 1, lines 40-87: mountains, every 64 px.
    def ridge(x):
        x %= 64
        return 50 + abs(x - 24) * 3 // 4 if x < 48 else 50 + (abs(x - 56) + 6) * 3 // 4

    for x in range(256):
        top = ridge(x)
        for y in range(40, 88):
            if y >= top:
                lit = (x % 64) < 24 and y < top + (24 - (x % 64)) // 2 + 3
                c.set(x, y, MTN_LIT if lit else MTN)
            if y >= top and y < top + 3 and top < 58:
                c.set(x, y, SNOW)
    starfield(c, 0, 40, 8, 1, 11, density=4)
    for x in range(256):
        for y in range(40, 48):
            c.px[y][x] = {STAR: SNOW, DIM: MTN_LIT}.get(c.px[y][x], c.px[y][x])
    for x in range(64, 256):
        for y in range(40, 48):
            c.px[y][x] = c.px[y][x - 64]
    # Band 2, lines 88-119: forest hills over the mountains' feet.
    for x in range(256):
        xx = x % 64
        top = 96 + (abs(xx - 32) // 6)
        crown = top - (3 if xx % 8 in (2, 3, 4, 5) else 1 if xx % 8 in (1, 6) else 0)
        for y in range(88, 120):
            if y >= crown:
                c.set(x, y, HILL)
                if y < crown + 3 and xx % 8 in (2, 3):
                    c.set(x, y, LEAF)
                elif (y * 5 + xx * 3) % 16 == 0 and y > top + 4:
                    c.set(x, y, HILL_LIT)
            else:
                c.set(x, y, HILL_BACK)
    # Band 3, lines 120-135: the ground, every 32 px.
    for x in range(256):
        xx = x % 32
        for y in range(120, 136):
            col = DIRT
            if y == 120 or (y == 121 and xx % 4 == 0):
                col = ROCK
            elif (xx * 3 + y * 5) % 11 == 0:
                col = DIRT_LIT
            c.set(x, y, col)
        if 8 <= xx < 14:
            for y in range(127, 132):
                if abs(xx - 11) + abs(y - 129) <= 3:
                    c.set(x, y, ROCK if abs(xx - 11) + abs(y - 129) == 3 or y > 129 else DIRT_DARK)
    for i, (y0, y1) in enumerate(BANDS):
        band = Canvas(256, y1 - y0, NIGHT)
        band.px = [row[:] for row in c.px[y0:y1]]
        band.save(f"bg/sky{i}.png")


BANDS = [(0, 40), (40, 88), (88, 120), (120, 136)]


T_LIGHT, T_GOLD, T_PLUM, T_NIGHT = pal("title")
LOGO_FILL, LOGO_EDGE, LOGO_DARK = T_LIGHT, T_GOLD, T_PLUM
PLANET, PLANET_LIT, PLANET_RIM = T_PLUM, T_GOLD, T_LIGHT


def big_text(c, x0, y0, text, scale):
    for i, ch in enumerate(text):
        bits = GLYPHS[ch]
        for gy, row in enumerate(bits):
            for gx in range(5):
                if row >> (4 - gx) & 1:
                    x, y = x0 + i * 6 * scale + gx * scale, y0 + gy * scale
                    c.rect(x + 1, y + 1, scale, scale, LOGO_DARK)
    for i, ch in enumerate(text):
        bits = GLYPHS[ch]
        for gy, row in enumerate(bits):
            for gx in range(5):
                if row >> (4 - gx) & 1:
                    x, y = x0 + i * 6 * scale + gx * scale, y0 + gy * scale
                    c.rect(x, y, scale, scale, LOGO_FILL if gy < 4 else LOGO_EDGE)


def title():
    c = Canvas(160, 144, T_NIGHT)
    starfield(c, 0, 0, 20, 18, 3, density=5)
    for row in c.px:
        for x, col in enumerate(row):
            row[x] = {STAR: T_LIGHT, DIM: T_GOLD}.get(col, T_NIGHT)
    # A planet rising from the bottom right.
    cx, cy, r = 128, 150, 48
    for y in range(144):
        for x in range(160):
            d2 = (x - cx) ** 2 + (y - cy) ** 2
            if d2 <= r * r:
                col = PLANET
                if d2 >= (r - 2) ** 2 and x < cx:
                    col = PLANET_RIM
                elif (x - cx + 14) ** 2 + (y - cy + 14) ** 2 > (r - 4) ** 2 and x < cx:
                    col = PLANET_LIT
                c.set(x, y, col)
    big_text(c, 8, 16, "VESPER", 4)
    big_text(c, 32, 50, "SWARM", 4)
    # The ship, twice its size, crossing below the logo.
    cols = {"1": T_LIGHT, "2": T_GOLD, "3": T_PLUM}
    for y, row in enumerate(SHIP):
        for x, ch in enumerate(row):
            if ch != ".":
                c.rect(16 + 2 * x, 92 + 2 * y, 2, 2, cols[ch])
    c.save("bg/title.png")


def main():
    font()
    sprites()
    sky()
    title()
    print("drew " + " ".join(sorted(str(p.relative_to(ROOT)) for p in ASSETS.rglob("*.png"))))


if __name__ == "__main__":
    main()
