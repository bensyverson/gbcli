"""Draws assets/bg/title.png: Switchback's title screen, 160x144.

A setting sun behind the mountains, the road running off to the horizon,
the name in large letters and PRESS START on a band across the road. The
letters are tools/font.py's glyphs, doubled.

Run: python3 -I tools/title.py (from anywhere).
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from png import Canvas  # noqa: E402
from font import FONT  # noqa: E402
from colors import SHADE  # noqa: E402

# Indexed art in the DMG shades; the title palette in assets/palettes.txt
# colors it on a GBC (cream, sunset orange, purple, near black).
SKY = WHITE = SHADE[0]
SUNSET = GRASS = SHADE[1]
MOUNTAIN = ROAD = CAR_BODY = SHADE[2]
OUTLINE = RED = SHADE[3]

W, H = 160, 144
HORIZON = 64


def text(c, s, x, y, scale, color, shadow=None):
    for k, ch in enumerate(s):
        for row, bits in enumerate(FONT[ch]):
            for col, bit in enumerate(bits):
                if bit != "#":
                    continue
                px = x + k * 6 * scale + col * scale
                py = y + row * scale
                if shadow:
                    c.rect(px + 1, py + 1, scale, scale, shadow)
                c.rect(px, py, scale, scale, color)


def main():
    c = Canvas(W, H, SKY)
    # The sun: a half disc sitting on the horizon.
    cx, r = 80, 22
    for y in range(HORIZON - r, HORIZON):
        for x in range(cx - r, cx + r + 1):
            if (x - cx) ** 2 + (y - HORIZON) ** 2 <= r * r:
                c.set(x, y, SUNSET)
    # Mountains in front of it, two peaks a side.
    for x in range(W):
        h = max(0, 14 - abs((x % 80) - 20) // 2, 10 - abs(((x + 30) % 80) - 40) // 3)
        if abs(x - cx) < 18:
            h = min(h, 3)
        for y in range(HORIZON - h, HORIZON):
            c.set(x, y, MOUNTAIN)
    # Grass and the road, widening toward the bottom, with rumble stripes.
    for y in range(HORIZON, H):
        d = y - HORIZON + 1
        half = 2 + d * 70 // (H - HORIZON)
        rumble = max(1, half // 7)
        z = 1200 // (d + 6)
        stripe = (z // 6) % 2
        for x in range(W):
            off = abs(x - 80 + 0.5)
            if off > half:
                col = GRASS
            elif off > half - rumble:
                col = RED if stripe else WHITE
            elif stripe and off < max(1, half // 24):
                col = WHITE
            else:
                col = ROAD
            c.set(x, y, col)
    # The name, and PRESS START on a band across the road.
    name = "SWITCHBACK"
    scale = 2
    tw = len(name) * 6 * scale - scale
    text(c, name, (W - tw) // 2, 14, scale, CAR_BODY, OUTLINE)
    c.rect(0, 104, W, 16, ROAD)
    msg = "PRESS START"
    text(c, msg, (W - (len(msg) * 6 - 1)) // 2, 108, 1, WHITE)
    out = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets", "bg", "title.png"))
    c.save(out)
    seen = {tuple(tuple(c.px[ty * 8 + r][tx * 8: tx * 8 + 8]) for r in range(8))
            for ty in range(H // 8) for tx in range(W // 8)}
    print(f"wrote {out}: {len(seen)} unique tiles")


if __name__ == "__main__":
    main()
