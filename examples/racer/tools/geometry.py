"""The road's geometry: one place for the numbers road.py and tables.py share.

The background map holds the sky, then ROWS road strips in each of two phases
(see road.py). The scanline engine (src/road.asm) picks, for every screen
line, a strip by depth (SCY) and shifts it for curves and steering (SCX).
"""

SKY_ROWS = 96          # BG rows of sky; row SKY_ROWS-1 sits on the horizon
ROWS = 80              # road strips per phase; strip 0 is the horizon
MAP_ROWS = SKY_ROWS + 2 * ROWS

CENTER = 128           # BG x of the road's center line
SCREEN_W = 160
ROAD_SCX0 = CENTER - SCREEN_W // 2  # SCX that centers the road on screen

MIN_HALF = 2           # half-width of strip 0, in pixels
MAX_HALF = 88          # half-width of strip ROWS-1

FIRST_LINE = 16        # the highest line the road may reach (steepest climb)
LAST_LINE = 143        # the bottom of the screen
PROFILE_LINES = 128    # lines FIRST_LINE..LAST_LINE, one byte each

LEVELS = 32            # hill and curve levels; level 16 is flat / straight
FLAT = 16

FLAT_HORIZON = 56      # the first road line on flat ground

Z_NUM = 2048           # depth of strip i: Z_NUM / (i + Z_OFF), clamped to 255
Z_OFF = 8
STRIPE = 16            # depth units per rumble stripe (two stripes a period)
PHASES = 2 * STRIPE    # the stripe pattern repeats every 32 depth units

CURVE_MAX = 110        # pixels the road's far end shifts at curve level 0 or 31


# Objects: lanes as fractions of the road's half-width (roadside left, left
# lane, right lane, roadside right), and the first strip of each pre-scaled
# size (frames 0-3 of the 32x32 sheets); nearer than strip 8 is drawn,
# farther isn't.
LANES = (-1.3, -0.45, 0.45, 1.3)
SIZE_FROM = (8, 26, 44, 62)


def half_width(i):
    return MIN_HALF + (i * (MAX_HALF - MIN_HALF) + (ROWS - 1) // 2) // (ROWS - 1)


def depth(i):
    return min(255, Z_NUM // (i + Z_OFF))
