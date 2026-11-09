"""Draws the object sheets in assets/sprites/ .

  car.png             the player's car, 24x16 (three 8x16 objects)
  rival.24x16.png     a rival car at four pre-scaled sizes, far to near
  tree.8x32.png       a roadside tree at four sizes, far to near

Each frame is drawn bottom-centered, so the program anchors every size at
the same point: the frame's bottom center on the object's line. The frames
are narrow on purpose: on the car's lines, the car (3 objects), a near
rival (3) and a near tree (1) stay at 7 objects, which keeps HBlank long
enough for the road's scroll writes (src/road.asm).

Run: python3 -I tools/sprites.py (from anywhere).
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from png import Canvas  # noqa: E402
from colors import (CAR_BODY, CAR_LIGHT, LEAVES, LEAVES_LIGHT, OUTLINE,  # noqa: E402
                    RIVAL_BODY, RIVAL_LIGHT, TRUNK)

CLEAR = (0, 0, 0, 0)

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.normpath(os.path.join(HERE, "..", "assets", "sprites"))


def car(c, ox, oy, w, h, body, light):
    """A car seen from behind, w x h, top-left at (ox, oy): cabin and rear
    window on top, the body with two tail lights, tires under the corners."""
    tire_h = max(1, h // 5)
    cab_h = max(1, (h * 2) // 5)
    cab_w = max(2, (w * 3) // 5)
    cx = ox + (w - cab_w) // 2
    # cabin: outline, then the rear window
    c.rect(cx, oy, cab_w, cab_h, OUTLINE)
    if cab_w > 3 and cab_h > 2:
        c.rect(cx + 1, oy + 1, cab_w - 2, cab_h - 1, light)
    # body
    body_y = oy + cab_h
    body_h = h - cab_h - tire_h
    c.rect(ox, body_y, w, body_h, OUTLINE)
    if w > 2 and body_h > 2:
        c.rect(ox + 1, body_y, w - 2, body_h - 1, body)
        lamp = max(1, w // 6)
        c.rect(ox + 1, body_y + 1, lamp, 1, light)
        c.rect(ox + w - 1 - lamp, body_y + 1, lamp, 1, light)
    # tires
    tire_w = max(1, w // 4)
    c.rect(ox, oy + h - tire_h, tire_w, tire_h, OUTLINE)
    c.rect(ox + w - tire_w, oy + h - tire_h, tire_w, tire_h, OUTLINE)


def tree(c, ox, oy, w, h):
    """A poplar: a pointed crown, lit on its left, on a short trunk."""
    trunk_h = max(1, h // 5)
    trunk_w = max(1, w // 4)
    c.rect(ox + (w - trunk_w) // 2, oy + h - trunk_h, trunk_w, trunk_h, TRUNK)
    crown_h = h - trunk_h
    cx2 = 2 * ox + w  # twice the center, to stay in whole pixels
    for y in range(crown_h):
        # widest two thirds of the way down, rounded at the bottom
        t = (y + 1) / crown_h
        width = w * min(1.0, t * 1.5) if t < 0.85 else w * (1 - (t - 0.85) * 2)
        half = max(0.5, width / 2)
        x0 = int(round((cx2 / 2) - half))
        x1 = int(round((cx2 / 2) + half))
        for x in range(x0, max(x0 + 1, x1)):
            lit = x < (cx2 / 2) - half / 3 and (x + y) % 3 != 0
            c.set(x, oy + y, LEAVES_LIGHT if lit else LEAVES)


def frames(name, fw, fh, sizes, draw):
    c = Canvas(fw * len(sizes), fh, CLEAR)
    for k, (w, h) in enumerate(sizes):
        draw(c, k * fw + (fw - w) // 2, fh - h, w, h)
    c.save(os.path.join(OUT, name))
    print(f"wrote {name}")


def main():
    os.makedirs(OUT, exist_ok=True)
    c = Canvas(24, 16, CLEAR)
    car(c, 0, 0, 24, 16, CAR_BODY, CAR_LIGHT)
    c.save(os.path.join(OUT, "car.png"))
    print("wrote car.png")
    frames("rival.24x16.png", 24, 16, [(6, 4), (10, 7), (16, 11), (24, 16)],
           lambda c, x, y, w, h: car(c, x, y, w, h, RIVAL_BODY, RIVAL_LIGHT))
    frames("tree.8x32.png", 8, 32, [(4, 8), (6, 14), (8, 22), (8, 32)], tree)


if __name__ == "__main__":
    main()
