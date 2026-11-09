"""Switchback's colors, shared by every art script, and its DMG shades.

The two backgrounds are indexed art: drawn in the four DMG shades (SHADE),
and colored on a GBC by the palette tools/palettes.py assigns them in
assets/palettes.txt (ROAD_PALETTE, TITLE_PALETTE: entry i colors shade i).
A plain `gbcli build` takes at most 4 colors per background, so color art
with more would build only under --gbc.

The object sheets are color art, 3 colors each: a --gbc build shows these
colors, and the DMG shows each in the shade of its nearest entry on the dmg
line (DMG_LINE). The comment on each is the shade it lands on.
"""

from png import hexcolor

# The dmg line: reference colors for shades 0-3. Indexed art draws in
# exactly these.
DMG_LINE = ("#ffffff", "#b0c890", "#707078", "#200810")
SHADE = tuple(hexcolor(h) for h in DMG_LINE)

# The road: sky and white paint (0), grass (1), road and mountains (2), the
# red rumble (3).
ROAD_PALETTE = ("#d8ecff", "#78c048", "#686878", "#a01818")
SKY = WHITE = SHADE[0]
GRASS = SHADE[1]
ROAD = MOUNTAIN = SHADE[2]
RED = SHADE[3]

# The title, at sunset: cream sky and lettering (0), the sun and the glowing
# grass (1), purple mountains, road and name (2), shadows and rumble (3).
TITLE_PALETTE = ("#fff4d8", "#f09048", "#7048a0", "#200818")

CAR_BODY = hexcolor("#d03030")   # 2
CAR_LIGHT = hexcolor("#ffffff")  # 0
OUTLINE = hexcolor("#200810")    # 3
RIVAL_BODY = hexcolor("#3858c8")  # 2
RIVAL_LIGHT = hexcolor("#d0e0ff")  # 0
LEAVES = hexcolor("#308830")     # 2
LEAVES_LIGHT = hexcolor("#90d060")  # 1
TRUNK = hexcolor("#503018")      # 3
INK = hexcolor("#000000")        # 3

EXPECTED = {
    "CAR_BODY": 2, "CAR_LIGHT": 0, "OUTLINE": 3, "RIVAL_BODY": 2, "RIVAL_LIGHT": 0,
    "LEAVES": 2, "LEAVES_LIGHT": 1, "TRUNK": 3, "INK": 3,
}


def shade(color):
    return min(range(4), key=lambda k: sum((a - b) ** 2 for a, b in zip(color[:3], SHADE[k][:3])))
