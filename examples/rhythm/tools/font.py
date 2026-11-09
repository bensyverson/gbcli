#!/usr/bin/env python3
"""Writes assets/tiles/font.pixels, Lantern Drum's text tiles, from 5x7 glyphs.

One 8x8 tile per character, in CHARS order, so the game's charmap
(src/text.asm) is tile = FONT_BASE + CHARS.index(c). '[' and ']' are the
life meter's empty and full cells.

Run: python3 -I tools/font.py [--check]   (from examples/rhythm)
"""

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "assets" / "tiles" / "font.pixels"

CHARS = " 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ!-:[]"

G = {
    " ": [".....", ".....", ".....", ".....", ".....", ".....", "....."],
    "0": [".XXX.", "X...X", "X..XX", "X.X.X", "XX..X", "X...X", ".XXX."],
    "1": ["..X..", ".XX..", "..X..", "..X..", "..X..", "..X..", ".XXX."],
    "2": [".XXX.", "X...X", "....X", "...X.", "..X..", ".X...", "XXXXX"],
    "3": ["XXXX.", "....X", "....X", ".XXX.", "....X", "....X", "XXXX."],
    "4": ["...X.", "..XX.", ".X.X.", "X..X.", "XXXXX", "...X.", "...X."],
    "5": ["XXXXX", "X....", "XXXX.", "....X", "....X", "X...X", ".XXX."],
    "6": ["..XX.", ".X...", "X....", "XXXX.", "X...X", "X...X", ".XXX."],
    "7": ["XXXXX", "....X", "...X.", "..X..", ".X...", ".X...", ".X..."],
    "8": [".XXX.", "X...X", "X...X", ".XXX.", "X...X", "X...X", ".XXX."],
    "9": [".XXX.", "X...X", "X...X", ".XXXX", "....X", "...X.", ".XX.."],
    "A": [".XXX.", "X...X", "X...X", "XXXXX", "X...X", "X...X", "X...X"],
    "B": ["XXXX.", "X...X", "X...X", "XXXX.", "X...X", "X...X", "XXXX."],
    "C": [".XXX.", "X...X", "X....", "X....", "X....", "X...X", ".XXX."],
    "D": ["XXXX.", "X...X", "X...X", "X...X", "X...X", "X...X", "XXXX."],
    "E": ["XXXXX", "X....", "X....", "XXXX.", "X....", "X....", "XXXXX"],
    "F": ["XXXXX", "X....", "X....", "XXXX.", "X....", "X....", "X...."],
    "G": [".XXX.", "X...X", "X....", "X.XXX", "X...X", "X...X", ".XXXX"],
    "H": ["X...X", "X...X", "X...X", "XXXXX", "X...X", "X...X", "X...X"],
    "I": [".XXX.", "..X..", "..X..", "..X..", "..X..", "..X..", ".XXX."],
    "J": ["..XXX", "...X.", "...X.", "...X.", "...X.", "X..X.", ".XX.."],
    "K": ["X...X", "X..X.", "X.X..", "XX...", "X.X..", "X..X.", "X...X"],
    "L": ["X....", "X....", "X....", "X....", "X....", "X....", "XXXXX"],
    "M": ["X...X", "XX.XX", "X.X.X", "X.X.X", "X...X", "X...X", "X...X"],
    "N": ["X...X", "X...X", "XX..X", "X.X.X", "X..XX", "X...X", "X...X"],
    "O": [".XXX.", "X...X", "X...X", "X...X", "X...X", "X...X", ".XXX."],
    "P": ["XXXX.", "X...X", "X...X", "XXXX.", "X....", "X....", "X...."],
    "Q": [".XXX.", "X...X", "X...X", "X...X", "X.X.X", "X..X.", ".XX.X"],
    "R": ["XXXX.", "X...X", "X...X", "XXXX.", "X.X..", "X..X.", "X...X"],
    "S": [".XXXX", "X....", "X....", ".XXX.", "....X", "....X", "XXXX."],
    "T": ["XXXXX", "..X..", "..X..", "..X..", "..X..", "..X..", "..X.."],
    "U": ["X...X", "X...X", "X...X", "X...X", "X...X", "X...X", ".XXX."],
    "V": ["X...X", "X...X", "X...X", "X...X", "X...X", ".X.X.", "..X.."],
    "W": ["X...X", "X...X", "X...X", "X.X.X", "X.X.X", "X.X.X", ".X.X."],
    "X": ["X...X", "X...X", ".X.X.", "..X..", ".X.X.", "X...X", "X...X"],
    "Y": ["X...X", "X...X", ".X.X.", "..X..", "..X..", "..X..", "..X.."],
    "Z": ["XXXXX", "....X", "...X.", "..X..", ".X...", "X....", "XXXXX"],
    "!": ["..X..", "..X..", "..X..", "..X..", "..X..", ".....", "..X.."],
    "-": [".....", ".....", ".....", "XXXXX", ".....", ".....", "....."],
    ":": [".....", "..X..", "..X..", ".....", "..X..", "..X..", "....."],
}


def cell(c):
    if c == "[":
        return ["kkkkkkkk"] + ["k......k"] * 6 + ["kkkkkkkk"]
    if c == "]":
        return ["kkkkkkkk"] + ["kggggggk"] * 6 + ["kkkkkkkk"]
    rows = ["." + r.replace("X", "k") + ".." for r in G[c]]
    return rows + ["........"]


def text():
    out = [
        "format gbcli-pixels 1",
        "# Lantern Drum's text tiles, one per character: made by tools/font.py.",
        f"# Characters, in tile order: {CHARS!r}",
        f"size {8 * len(CHARS)} 8",
        "color . #ffffff",
        "color g #aaaaaa",
        "color k #000000",
    ]
    for i, c in enumerate(CHARS):
        out.append("")
        out.append(f"rows {8 * i} 0   # {c!r}")
        out.extend(cell(c))
    return "\n".join(out) + "\n"


def main():
    t = text()
    if "--check" in sys.argv[1:]:
        if not OUT.exists() or OUT.read_text() != t:
            print(f"stale: {OUT.relative_to(ROOT)} (run tools/font.py)", file=sys.stderr)
            return 1
        return 0
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(t)
    print(f"wrote {OUT.relative_to(ROOT)} ({len(CHARS)} tiles)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
