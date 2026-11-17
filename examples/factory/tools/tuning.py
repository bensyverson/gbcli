"""Reads include/tuning.inc, so tools agree with the game on every number.

Only the `DEF NAME EQU value` lines are read; a value may be a number
(decimal, $hex or %binary) or a name defined earlier in the file.
"""

import re
from pathlib import Path

PROJECT = Path(__file__).resolve().parent.parent
TUNING = PROJECT / "include" / "tuning.inc"

_DEF = re.compile(r"^\s*DEF\s+(\w+)\s+EQU\s+([^;]+)", re.IGNORECASE)


def _number(text, known):
    text = text.strip()
    if text.startswith("$"):
        return int(text[1:], 16)
    if text.startswith("%"):
        return int(text[1:].replace("_", ""), 2)
    if text in known:
        return known[text]
    return int(text)


def load(path=TUNING):
    """Every constant in the tuning file, by name."""
    known = {}
    for line in Path(path).read_text().splitlines():
        m = _DEF.match(line)
        if m:
            known[m.group(1)] = _number(m.group(2), known)
    return known
