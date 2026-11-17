"""Reads gbcli pixel text (`gbcli help pixels`) into shades 0-3.

Supports the parts the toy art uses: size, color (#rgb, #rrggbb, clear),
fill and rows blocks. A color takes the DMG shade nearest its lightness
(white 0 to black 3); clear is shade 0, the belt's color behind a piece.
"""

from pathlib import Path


def _shade(value, clear=0):
    if value == "clear":
        return clear
    if not value.startswith("#"):
        raise ValueError(f"unsupported color {value!r} (use #rrggbb or clear)")
    h = value[1:]
    if len(h) == 3:
        h = "".join(c * 2 for c in h)
    r, g, b = (int(h[i:i + 2], 16) for i in (0, 2, 4))
    light = (r * 299 + g * 587 + b * 114) / 1000
    return min(3, max(0, round((255 - light) / 85)))


def _strip_comment(line):
    """The line up to a '#' that is followed by a space, a tab or the end."""
    for i, ch in enumerate(line):
        if ch == "#" and (i + 1 == len(line) or line[i + 1] in " \t"):
            return line[:i]
    return line


def read(path, clear=0):
    """The image as rows of shades; clear pixels read as `clear`."""
    width = height = None
    legend = {}
    img = None
    block = None  # (x, y, row index)
    for number, raw in enumerate(Path(path).read_text().splitlines(), 1):
        words = _strip_comment(raw).split()
        if block is not None:
            if not words:
                block = None
                continue
            x, y, i = block
            for dx, ch in enumerate(words[0]):
                if ch not in legend:
                    raise ValueError(f"{path}:{number}: {ch!r} is not in the legend")
                img[y + i][x + dx] = legend[ch]
            block = (x, y, i + 1)
            continue
        if not words or words[0].startswith("#"):
            continue
        kind = words[0]
        if kind == "format":
            continue
        if kind == "size":
            width, height = int(words[1]), int(words[2])
            img = [[None] * width for _ in range(height)]
        elif kind == "color":
            legend[words[1]] = _shade(words[2], clear)
        elif kind == "fill":
            img = [[legend[words[1]]] * width for _ in range(height)]
        elif kind == "rows":
            x, y = (int(words[1]), int(words[2])) if len(words) > 1 else (0, 0)
            block = (x, y, 0)
        else:
            raise ValueError(f"{path}:{number}: {kind!r} blocks aren't supported here")
    for y, row in enumerate(img):
        if None in row:
            raise ValueError(f"{path}: pixel ({row.index(None)}, {y}) is never painted")
    return img
