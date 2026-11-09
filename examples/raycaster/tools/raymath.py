"""The raycaster's fixed-point math and tables, shared by the ROM and the host.

gen_tables.py writes these tables into src/tables.asm, and reference.py renders
a view with the same integer steps the ROM takes, so a `gbcli shot` of the
ROM's view and this module's picture must match pixel for pixel.

Units: a map cell is 256 (8.8 fixed point; high byte = cell, low byte =
fraction). A full turn is ANG = 1024 angle units. Angle 0 looks along +x
(east); angles grow clockwise on the map, since map y grows downward.
"""

import math

ANG = 1024
MAP_W = 16
MAP_H = 16

RAYS = 64  # one ray per 2-pixel column
VIEW_W = RAYS * 2  # 128 pixels
VIEW_H = 64
HALF_H = VIEW_H // 2

FOV_DEG = 66.0
TAN_HALF_FOV = math.tan(math.radians(FOV_DEG / 2))
# Pixels from the eye to the projection plane: half the view's width over
# tan(FOV/2). A one-cell-tall wall at perpendicular distance d (cells) is
# FOCAL / d pixels tall, so its half height is FOCAL / 2 / d.
FOCAL = (VIEW_W / 2) / TAN_HALF_FOV

DELTA_CAP = 0x2000  # 32 cells: farther than any ray can travel in the map
HH_ENTRIES = 1024  # half-height table, indexed by perpendicular distance >> 3

# Shades (0 lightest .. 3 darkest), BGP = %11_10_01_00.
CEILING = 0
FLOOR = 1
WALL_X = 3  # a wall face crossed by a vertical grid line (east/west face)
WALL_Y = 2  # a wall face crossed by a horizontal grid line (north/south face)

# The shade of each map cell's faces, (x side, y side), by cell value:
# 1 stone, 2 a key block (glowing white), 3 the exit door (light gray).
CELL_SHADES = {1: (WALL_X, WALL_Y), 2: (0, 0), 3: (1, 1)}


def delta_table():
    """|1/cos(a)| in 8.8, capped: the ray length per cell crossed along x."""
    out = []
    for a in range(ANG):
        c = abs(math.cos(2 * math.pi * a / ANG))
        if c * DELTA_CAP <= 256:
            out.append(DELTA_CAP)
        else:
            out.append(min(DELTA_CAP, int(256 / c + 0.5)))
    return out


def col_offsets():
    """Each ray's angle relative to the view direction, for a flat projection."""
    out = []
    for c in range(RAYS):
        x = ((c + 0.5) - RAYS / 2) / (RAYS / 2) * TAN_HALF_FOV
        out.append(int(round(math.atan(x) * ANG / (2 * math.pi))))
    return out


def cos_offsets(offs):
    """cos of each ray's offset in 0.8, capped at 255: undoes the fisheye."""
    return [min(255, int(round(256 * math.cos(2 * math.pi * o / ANG)))) for o in offs]


def half_heights():
    """Wall half height in pixels by perpendicular distance >> 3 (1/32 cell)."""
    out = []
    for i in range(HH_ENTRIES):
        d = (i * 8 + 4) / 256
        out.append(min(HALF_H, int(FOCAL / 2 / d + 0.5)))
    return out


MOVE_SPEED = 16  # 1/16 cell a frame at full stride


def move_table():
    """cos(a) * MOVE_SPEED as a signed byte; sin(a) is entry (a - 256)."""
    return [int(round(MOVE_SPEED * math.cos(2 * math.pi * a / ANG))) for a in range(ANG)]


def wall_patterns():
    """The 16 low-plane bytes of a tile row whose four rays are all wall:
    each 2-bit pair is %11 (x side, shade 3) or %00 (y side, shade 2)."""
    out = []
    for n in range(16):
        v = 0
        for r in range(4):
            if n & (8 >> r):
                v |= 0xC0 >> (2 * r)
        out.append(v)
    return out


DELTA = delta_table()
COL_OFF = col_offsets()
COS_OFF = cos_offsets(COL_OFF)
HH = half_heights()
MOVE = move_table()


def step_x(a):
    return 1 if ((a + 256) & (ANG - 1)) < 512 else -1


def step_y(a):
    return 1 if a < 512 else -1


def mul16x8(v, m):
    """The ROM's Mul16x8: (v * m) >> 8, v < 65536, m < 256."""
    return (v * m) >> 8


def cast(px, py, a, cells):
    """One ray from (px, py) at angle a. Returns (side, euclidean distance 8.8,
    the cell hit).

    cells is the 16x16 map as a flat list, row-major; nonzero is a wall.
    """
    dx = DELTA[a]
    dy = DELTA[(a - 256) & (ANG - 1)]
    sx = step_x(a)
    sy = step_y(a)
    fx = px & 0xFF
    fy = py & 0xFF
    if sx > 0:
        side_x = dx if fx == 0 else mul16x8(dx, 256 - fx)
    else:
        side_x = mul16x8(dx, fx)
    if sy > 0:
        side_y = dy if fy == 0 else mul16x8(dy, 256 - fy)
    else:
        side_y = mul16x8(dy, fy)
    idx = (py >> 8) * MAP_W + (px >> 8)
    while True:
        if side_x < side_y:
            side_x += dx
            idx += sx
            side = 0
        else:
            side_y += dy
            idx += sy * MAP_W
            side = 1
        if cells[idx]:
            break
    dist = side_x - dx if side == 0 else side_y - dy
    return side, dist, cells[idx]


def ray_columns(px, py, pa, cells):
    """(half height, wall shade) for each of the RAYS columns."""
    cols = []
    for c in range(RAYS):
        a = (pa + COL_OFF[c]) & (ANG - 1)
        side, dist, cell = cast(px, py, a, cells)
        perp = mul16x8(dist, COS_OFF[c])
        hh = HH[min(perp >> 3, HH_ENTRIES - 1)]
        cols.append((hh, CELL_SHADES[cell][side]))
    return cols


def render(px, py, pa, cells):
    """The view as rows of shades, VIEW_H rows of VIEW_W pixels."""
    cols = ray_columns(px, py, pa, cells)
    rows = []
    for y in range(VIEW_H):
        row = []
        for hh, shade in cols:
            if y < HALF_H - hh:
                v = CEILING
            elif y < HALF_H + hh:
                v = shade
            else:
                v = FLOOR
            row.append(v)
            row.append(v)
        rows.append(row)
    return rows
