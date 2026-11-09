"""Reads the parts of gbcli song text the chart needs: ticks per row, the
timer, the orders, and each pattern's rows per channel.

Only what `gbcli help music` documents is assumed. Effects that change the
song's flow (Bxx position jump, Dxx pattern break, Fxx set speed) would move
every later onset, so they are refused rather than half-handled.
"""

import re

ROWS_PER_PATTERN = 64
FLOW_EFFECTS = ("B", "D", "F")
NOTE_RE = re.compile(r"^([A-G])([-#])([3-8])$")
SEMITONE = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}


class SongError(Exception):
    pass


def note_number(name):
    """hUGETracker's note name as a semitone count from C-3 (0)."""
    m = NOTE_RE.match(name)
    if not m:
        raise SongError(f"not a note: {name!r}")
    letter, sharp, octave = m.groups()
    return (int(octave) - 3) * 12 + SEMITONE[letter] + (1 if sharp == "#" else 0)


class Song:
    def __init__(self, text):
        self.ticks_per_row = 6
        self.timer = None  # None for off, else the TMA value D
        self.orders = []   # [(P0, P1, P2, P3)] in play order
        self.patterns = {}  # name -> {row: (note, inst, effect)}
        self._parse(text)

    def _parse(self, text):
        section = None
        header = None
        for lineno, raw in enumerate(text.splitlines(), 1):
            line = raw.strip()
            if not line or line.startswith("#"):
                continue
            words = line.split()
            head = words[0]
            if head == "ticks_per_row":
                self.ticks_per_row = int(words[1])
            elif head == "timer":
                self.timer = None if words[1] == "off" else int(words[2])
            elif head == "orders":
                section, header = "orders", None
            elif head == "pattern":
                section, header = "pattern", words[1:5]
                for name in header:
                    self.patterns.setdefault(name, {})
            elif head in ("instrument", "wave", "routine", "storage", "format",
                          "name", "artist", "comment"):
                section, header = None, None
            elif re.fullmatch(r"\d\d", head) and section == "orders":
                self.orders.append(tuple(words[1:5]))
            elif re.fullmatch(r"\d\d", head) and section == "pattern":
                row = int(head)
                cells = [c.split() for c in line[2:].split("|")]
                if len(cells) != 4:
                    raise SongError(f"line {lineno}: want 4 cells")
                for name, cell in zip(header, cells):
                    note, inst, _vol, effect = cell
                    self.patterns[name][row] = (note, inst, effect)

    def frames_per_tick(self):
        """Exact frames per driver tick, as a (numerator, denominator) pair
        of 2^23 Hz ticks: a frame is 140,448 of them."""
        if self.timer is None:
            return (140448, 1)
        # 4096 / (256 - D) ticks a second, 2^23 elapsed ticks a second.
        return (2 ** 23 * (256 - self.timer), 4096)

    def onsets(self, channel):
        """Every note the channel starts: (tick, note, order, row), in order.

        tick counts driver ticks from the first (0); a 7xx note delay starts
        the note xx ticks into its row.
        """
        out = []
        for order, names in enumerate(self.orders):
            pattern = self.patterns.get(names[channel], {})
            for other in names:
                for row, (_n, _i, eff) in self.patterns.get(other, {}).items():
                    if eff != "..." and eff[0] in FLOW_EFFECTS:
                        raise SongError(
                            f"order {order:02d} pattern {other} row {row:02d}:"
                            f" effect {eff} changes the song's flow")
            for row in sorted(pattern):
                note, _inst, eff = pattern[row]
                if note == "...":
                    continue
                delay = int(eff[1:], 16) if eff.startswith("7") else 0
                tick = (order * ROWS_PER_PATTERN + row) * self.ticks_per_row + delay
                out.append((tick, note, order, row))
        return out

    def length_ticks(self):
        return len(self.orders) * ROWS_PER_PATTERN * self.ticks_per_row
