#!/usr/bin/env python3
"""Makes Lantern Drum's chart from the song: one note per lead onset.

Reads assets/music/lantern.song, takes channel 1's note onsets in driver
ticks, gives each a lane by the melody's contour (up a step moves a lane
right, down moves left, a repeat stays), and writes:

  assets/music/lantern.chart   the chart as text: tick, lane, note
  src/chart.asm                the same as data for the game
  include/chart.inc            its constants: PREROLL, CHART_LEN, SONG_END_TICK

The chart is in driver ticks, never frames, so nothing is rounded: the game
counts the driver's ticks and judges a press against that count.

Run: python3 -I tools/chart.py [--check]   (from examples/rhythm)
"""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from songtext import Song, note_number  # noqa: E402

ROOT = pathlib.Path(__file__).resolve().parent.parent
SONG = ROOT / "assets" / "music" / "lantern.song"
CHART = ROOT / "assets" / "music" / "lantern.chart"
ASM = ROOT / "src" / "chart.asm"
INC = ROOT / "include" / "chart.inc"
CHART_CHANNEL = 0  # channel 1, the lead
LANES = 4
LANE_KEYS = ["LEFT", "RIGHT", "B", "A"]
PREROLL = 64  # ticks of silence before the song, while the first notes fall


def make_chart(song):
    notes = []
    lane = 1
    prev = None
    for tick, note, order, row in song.onsets(CHART_CHANNEL):
        n = note_number(note)
        if prev is not None:
            if n > prev:
                lane = (lane + 1) % LANES
            elif n < prev:
                lane = (lane - 1) % LANES
        prev = n
        notes.append((tick, lane, note, order, row))
    return notes


def chart_text(song, notes):
    lines = [
        "# Lantern Drum chart, made by tools/chart.py from lantern.song.",
        f"# ticks_per_row {song.ticks_per_row}; timer "
        f"{'off' if song.timer is None else 'on ' + str(song.timer)};"
        f" song length {song.length_ticks()} ticks",
        "# tick lane key note order row",
    ]
    for tick, lane, note, order, row in notes:
        lines.append(f"{tick:5d} {lane} {LANE_KEYS[lane]:5s} {note} "
                     f"{order:02d} {row:02d}")
    return "\n".join(lines) + "\n"


def chart_inc(song, notes):
    out = [
        "; Made by tools/chart.py from assets/music/lantern.song; do not edit.",
        "; The game's clock runs PREROLL ticks before the song starts, so notes",
        "; can fall into view; a note's clock time is PREROLL + its driver tick.",
        "",
        "IF !DEF(CHART_INC)",
        "DEF CHART_INC EQU 1",
        f"DEF PREROLL EQU {PREROLL}",
        f"DEF CHART_LEN EQU {len(notes)}",
        f"DEF SONG_END_TICK EQU {song.length_ticks()}",
        "ENDC",
    ]
    return "\n".join(out) + "\n"


def chart_asm(song, notes):
    if len(notes) > 255:
        raise SystemExit(f"{len(notes)} notes: the game indexes at most 255")
    out = [
        "; Made by tools/chart.py from assets/music/lantern.song; do not edit.",
        "; Each note: its clock time (PREROLL + the driver tick it sounds on),",
        "; and its lane (0-3). The constants are in include/chart.inc.",
        "",
        'include "chart.inc"',
        "",
        'SECTION "Chart", ROM0',
        "",
        "ChartTicks::",
    ]
    for i in range(0, len(notes), 8):
        out.append("\tdw " + ", ".join(f"PREROLL+{n[0]}" for n in notes[i:i + 8]))
    out.append("ChartLanes::")
    for i in range(0, len(notes), 16):
        out.append("\tdb " + ", ".join(str(n[1]) for n in notes[i:i + 16]))
    return "\n".join(out) + "\n"


def main():
    song = Song(SONG.read_text())
    notes = make_chart(song)
    files = {CHART: chart_text(song, notes), ASM: chart_asm(song, notes),
             INC: chart_inc(song, notes)}
    if "--check" in sys.argv[1:]:
        stale = [p for p, t in files.items() if not p.exists() or p.read_text() != t]
        for p in stale:
            print(f"stale: {p.relative_to(ROOT)} (run tools/chart.py)", file=sys.stderr)
        return 1 if stale else 0
    for path, text in files.items():
        path.write_text(text)
        print(f"wrote {path.relative_to(ROOT)}")
    print(f"chart notes={len(notes)} length_ticks={song.length_ticks()}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
