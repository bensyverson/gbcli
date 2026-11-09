#!/usr/bin/env python3
"""Writes Lantern Drum's pictures as gbcli pixel text:

  assets/tiles/play.pixels   the playfield's tiles: blank, lane, drum (the
                             hit marker), lantern (a note), burst (a hit),
                             lattice (the results screen's border)
  assets/bg/title.pixels     the title screen, 160x144: the name over a
                             night sky, and a lantern; row 15 is left clear
                             for PRESS START, printed in the font

Every image uses the four shades of palette `ui` (assets/palettes.txt):
. lightest, l, d, k darkest. On a GBC the program colors them.

Run: python3 -I tools/art.py [--check]   (from examples/rhythm)
"""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from font import G  # noqa: E402

ROOT = pathlib.Path(__file__).resolve().parent.parent
PLAY = ROOT / "assets" / "tiles" / "play.pixels"
TITLE = ROOT / "assets" / "bg" / "title.pixels"

LEGEND = ["color . #ffffff", "color l #aaaaaa", "color d #555555", "color k #000000"]

# In tile order: the program's TILE_* constants (include/game.inc) follow it.
PLAY_TILES = [
    ("blank", ["........"] * 8),
    ("lane", ["...l....", "........"] * 4),
    ("drum", ["..kkkk..",
              ".kddddk.",
              "kd....dk",
              "kd....dk",
              "kd....dk",
              "kd....dk",
              ".kddddk.",
              "..kkkk.."]),
    ("lantern", ["...kk...",
                 ".kkkkkk.",
                 "kldlldlk",
                 "kldlldlk",
                 "kldlldlk",
                 "kldlldlk",
                 ".kkkkkk.",
                 "...kk..."]),
    ("burst", ["...k....",
               ".k.k.k..",
               "..kkk...",
               "kkk.kkk.",
               "..kkk...",
               ".k.k.k..",
               "...k....",
               "........"]),
    ("lattice", ["dddddddd",
                 "d..dd..d",
                 "d.d..d.d",
                 "dd....dd",
                 "dd....dd",
                 "d.d..d.d",
                 "d..dd..d",
                 "dddddddd"]),
]


def play_text():
    out = ["format gbcli-pixels 1",
           "# Lantern Drum's playfield tiles: made by tools/art.py.",
           f"size {8 * len(PLAY_TILES)} 8"] + LEGEND
    for i, (name, rows) in enumerate(PLAY_TILES):
        out += ["", f"rows {8 * i} 0   # {name}"] + rows
    return "\n".join(out) + "\n"


def title_pixels():
    w, h = 160, 144
    px = [["."] * w for _ in range(h)]

    def put(x, y, c):
        if 0 <= x < w and 0 <= y < h and not 120 <= y < 128:
            px[y][x] = c

    # Stars: a fixed little generator, so the picture never changes.
    seed = 12345
    for _ in range(18):
        seed = (seed * 1103515245 + 12345) % 2 ** 31
        x = seed % w
        seed = (seed * 1103515245 + 12345) % 2 ** 31
        y = seed % 112
        put(x, y, "l")

    def text(s, x0, y0, scale):
        for i, ch in enumerate(s):
            for gy, row in enumerate(G[ch]):
                for gx, bit in enumerate(row):
                    if bit != "X":
                        continue
                    for sy in range(scale):
                        for sx in range(scale):
                            x = x0 + i * 6 * scale + gx * scale + sx
                            y = y0 + gy * scale + sy
                            put(x + 1, y + 1, "d")
                            put(x, y, "k")

    text("LANTERN", (w - 7 * 12) // 2, 14, 2)
    text("DRUM", (w - 4 * 12) // 2, 34, 2)

    # The lantern: a paper body with ribs, caps and a handle, and a glow.
    cx, top, bottom, half = 80, 66, 106, 16
    for y in range(top, bottom):
        t = (y - top) / (bottom - top - 1)
        width = int(round(half * (1 - (2 * t - 1) ** 2) ** 0.5)) + 3
        for x in range(cx - width, cx + width):
            edge = x in (cx - width, cx + width - 1)
            rib = (x - cx) % 6 == 0
            put(x, y, "k" if edge else ("d" if rib else "l"))
    for y in (top - 2, top - 1, bottom, bottom + 1):
        for x in range(cx - 6, cx + 6):
            put(x, y, "k")
    for y in range(top - 8, top - 2):
        put(cx, y, "k")
    for x in range(cx - 4, cx + 5):
        put(x, top - 8, "k")
    for i in range(24):
        # glow: dots on a ring around the body
        import math
        a = 2 * math.pi * i / 24
        put(int(round(cx + 30 * math.cos(a))), int(round(86 + 26 * math.sin(a))), "l")
    # Drum sticks, crossed below the lantern.
    for i in range(14):
        put(cx - 20 + i, 112 + i // 3, "d")
        put(cx + 19 - i, 112 + i // 3, "d")
    return px


def title_text():
    px = title_pixels()
    out = ["format gbcli-pixels 1",
           "# Lantern Drum's title screen: made by tools/art.py.",
           "size 160 144"] + LEGEND
    for band in range(0, 144, 8):
        out += ["", f"rows 0 {band}"] + ["".join(r) for r in px[band:band + 8]]
    return "\n".join(out) + "\n"


def main():
    files = {PLAY: play_text(), TITLE: title_text()}
    if "--check" in sys.argv[1:]:
        stale = [p for p, t in files.items() if not p.exists() or p.read_text() != t]
        for p in stale:
            print(f"stale: {p.relative_to(ROOT)} (run tools/art.py)", file=sys.stderr)
        return 1 if stale else 0
    for path, text in files.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
        print(f"wrote {path.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
