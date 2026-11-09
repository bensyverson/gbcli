"""Draws assets/bg/road.png: the sky and the road strips the scanline engine picks from.

The background map is 256x256 pixels, read one pixel row at a time: each screen
line shows whichever BG row its SCY points at, shifted by its SCX.

  rows 0 .. SKY_ROWS-1        the sky; its bottom row sits on the horizon
  SKY_ROWS + i                road strip i in phase A (light rumble, center dash)
  SKY_ROWS + ROWS + i         road strip i in phase B (dark rumble, no dash)

Strip i is the road at one depth: i = 0 is the horizon (narrowest), i =
ROWS-1 the bottom of the screen (widest). The road is centered on x = 128.
src/road.asm assumes these constants (it asserts the map's height).

Run from examples/racer: python3 -I tools/road.py
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from png import Canvas  # noqa: E402
from geometry import CENTER, ROWS, SKY_ROWS, half_width  # noqa: E402

WIDTH = 256

# Indexed art, drawn in the DMG shades (tools/colors.py): the GBC colors
# shade i with entry i of the road palette in assets/palettes.txt.
from colors import GRASS, MOUNTAIN, RED, ROAD, SKY, WHITE  # noqa: E402


def draw_strip(c, y, i, phase):
    hw = half_width(i)
    rumble = max(1, hw // 7)
    dash = hw // 24
    for x in range(WIDTH):
        d = abs(x - CENTER + 0.5)
        if d > hw:
            col = GRASS
        elif d > hw - rumble:
            col = WHITE if phase == 0 else RED
        elif phase == 0 and dash > 0 and d < dash:
            col = WHITE
        else:
            col = ROAD
        c.set(x, y, col)


def mountain_height(x):
    # Two overlapping peaks, repeating every 64 pixels so the map wraps cleanly.
    a = 14 - abs((x % 64) - 22) // 2
    b = 10 - abs(((x + 37) % 64) - 32) // 3
    return max(a, b, 3)


def draw_sky(c):
    c.rect(0, 0, WIDTH, SKY_ROWS, SKY)
    for x in range(WIDTH):
        h = mountain_height(x)
        for yy in range(SKY_ROWS - h, SKY_ROWS):
            c.set(x, yy, MOUNTAIN)


def unique_tiles(c):
    seen = set()
    for ty in range(c.height // 8):
        for tx in range(c.width // 8):
            seen.add(tuple(tuple(c.px[ty * 8 + r][tx * 8: tx * 8 + 8]) for r in range(8)))
    return len(seen)


def main():
    c = Canvas(WIDTH, SKY_ROWS + 2 * ROWS)
    draw_sky(c)
    for phase in (0, 1):
        for i in range(ROWS):
            draw_strip(c, SKY_ROWS + phase * ROWS + i, i, phase)
    out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets", "bg", "road.png")
    out = os.path.normpath(out)
    os.makedirs(os.path.dirname(out), exist_ok=True)
    c.save(out)
    print(f"wrote {out}: {c.width}x{c.height}, {unique_tiles(c)} unique tiles")


if __name__ == "__main__":
    main()
