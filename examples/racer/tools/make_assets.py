"""Regenerates every generated file in the project, in dependency order:

  palettes.py  assets/palettes.txt
  road.py      assets/bg/road.png
  title.py     assets/bg/title.png
  sprites.py   assets/sprites/*.png
  font.py      assets/tiles/font.png
  songs.py     assets/music/*.song
  tables.py    include/road_geometry.inc, src/road_tables.asm

Each is deterministic, so a rerun rewrites the same bytes.

Run: python3 -I tools/make_assets.py (from anywhere).
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import font  # noqa: E402
import palettes  # noqa: E402
import road  # noqa: E402
import songs  # noqa: E402
import sprites  # noqa: E402
import tables  # noqa: E402
import title  # noqa: E402

for module in (palettes, road, title, sprites, font, songs, tables):
    module.main()
