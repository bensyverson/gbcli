#!/usr/bin/env python3
"""Makes Lantern Drum's chart from the song, ramping up across it.

Reads assets/music/lantern.song and takes notes the song really plays, in
driver ticks: the lead's (channel 1) onsets, and for a chord the bass's
(channel 3) onset on the same tick. Each order of the song has a STAGE that
says which lead onsets to chart and where to add chords, so the song opens
with single notes two beats apart, a lane at a time, and gets denser:

  order  lead onsets charted        chords
  0      every 2 beats, lane by lane  -
  1      every 2 beats                -
  2      every beat                   -
  3      every beat                   each bar's first beat
  4-5    all (adds off-beat eighths)  each bar's first beat
  6      all                          every 2 beats
  7      all (adds the 3-row syncopation)  each bar's first beat
  8      all                          every 2 beats
  9      all                          every note

A lead note's lane follows the melody between charted notes (up a step
moves a lane right, down moves left, a repeat stays); the opening order
instead walks the lanes, two notes each, to teach the buttons. A chord's
second note is its bass onset, in the lane two over, so a chord is always
one d-pad lane and one button lane: a thumb each.

Writes:
  assets/music/lantern.chart   the chart as text: tick, lane, key, note, order, row, channel
  src/chart.asm                the same as data for the game
  include/chart.inc            its constants: PREROLL, CHART_LEN, SONG_END_TICK,
                               LESSON_NOTES (the opening's, whose misses are free)

The chart is in driver ticks, never frames, so nothing is rounded: the game
counts the driver's ticks and judges a press against that count.

Run: python3 -I tools/chart.py [--check | --density]   (from examples/rhythm)
--density writes nothing and prints each order's notes, chords and closest
two press times: the ramp, as numbers.
"""

import pathlib
import sys
from typing import NamedTuple

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from songtext import Song, note_number  # noqa: E402

ROOT = pathlib.Path(__file__).resolve().parent.parent
SONG = ROOT / "assets" / "music" / "lantern.song"
CHART = ROOT / "assets" / "music" / "lantern.chart"
ASM = ROOT / "src" / "chart.asm"
INC = ROOT / "include" / "chart.inc"
LEAD = 0  # channel 1
BASS = 2  # channel 3
LANES = 4
LANE_KEYS = ["LEFT", "RIGHT", "B", "A"]
PREROLL = 64  # ticks of silence before the song, while the first notes fall
# The judgment's good window (include/game.inc GOOD_WIN) must not reach the
# next note of the same lane: keep same-lane notes at least this far apart.
MIN_LANE_GAP = 14


class Stage(NamedTuple):
    lead_every: int    # chart lead onsets on rows that are a multiple of this (1: all)
    chord_every: int   # add the bass on rows that are a multiple of this (0: none)
    walk: bool = False  # lanes walk left to right, two notes each, not by melody


# By order of the song (tools/compose.py SONG).
STAGES = [
    Stage(8, 0, walk=True),
    Stage(8, 0),
    Stage(4, 0),
    Stage(4, 16),
    Stage(1, 16),
    Stage(1, 16),
    Stage(1, 8),
    Stage(1, 16),
    Stage(1, 8),
    Stage(1, 1),
]


class Note(NamedTuple):
    tick: int     # driver tick it sounds on, from the song's first (0)
    lane: int     # 0-3, LANE_KEYS
    note: str     # hUGETracker's name for its pitch
    order: int
    row: int
    channel: int  # 0 (lead) or 2 (bass): the channel that plays it


def make_chart(song):
    if len(song.orders) != len(STAGES):
        raise SystemExit(f"the song has {len(song.orders)} orders, STAGES {len(STAGES)}")
    bass = {tick: (note, order, row) for tick, note, order, row in song.onsets(BASS)}
    notes = []
    lane, prev, walked = 1, None, 0
    for tick, note, order, row in song.onsets(LEAD):
        stage = STAGES[order]
        if row % stage.lead_every:
            continue
        n = note_number(note)
        if stage.walk:
            lane = (walked // 2) % LANES
            walked += 1
        elif prev is not None:
            if n > prev:
                lane = (lane + 1) % LANES
            elif n < prev:
                lane = (lane - 1) % LANES
        prev = n
        notes.append(Note(tick, lane, note, order, row, LEAD))
        if stage.chord_every and row % stage.chord_every == 0 and tick in bass:
            bnote, border, brow = bass[tick]
            notes.append(Note(tick, (lane + 2) % LANES, bnote, border, brow, BASS))
    notes.sort(key=lambda x: (x.tick, x.lane))
    last = {}
    for x in notes:
        if x.lane in last and x.tick - last[x.lane] < MIN_LANE_GAP:
            raise SystemExit(f"lane {x.lane}: notes at ticks {last[x.lane]} and {x.tick}"
                             f" are closer than {MIN_LANE_GAP}")
        last[x.lane] = x.tick
    return notes


def chart_text(song, notes):
    lines = [
        "# Lantern Drum chart, made by tools/chart.py from lantern.song.",
        f"# ticks_per_row {song.ticks_per_row}; timer "
        f"{'off' if song.timer is None else 'on ' + str(song.timer)};"
        f" song length {song.length_ticks()} ticks",
        "# tick lane key note order row channel",
    ]
    for x in notes:
        lines.append(f"{x.tick:5d} {x.lane} {LANE_KEYS[x.lane]:5s} {x.note} "
                     f"{x.order:02d} {x.row:02d} {x.channel + 1}")
    return "\n".join(lines) + "\n"


def lesson_notes(notes):
    """How many notes open the chart in walking stages: the lesson."""
    count = 0
    for x in notes:
        if not STAGES[x.order].walk:
            break
        count += 1
    return count


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
        "; The opening lesson (the lanes walked a pair at a time): its misses",
        "; cost no life.",
        f"DEF LESSON_NOTES EQU {lesson_notes(notes)}",
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
        out.append("\tdw " + ", ".join(f"PREROLL+{n.tick}" for n in notes[i:i + 8]))
    out.append("ChartLanes::")
    for i in range(0, len(notes), 16):
        out.append("\tdb " + ", ".join(str(n.lane) for n in notes[i:i + 16]))
    return "\n".join(out) + "\n"


def density(notes, song):
    """Per order: notes, chords, and the shortest gap between press times."""
    rows = []
    for order in range(len(song.orders)):
        mine = [x for x in notes if x.order == order]
        ticks = sorted({x.tick for x in mine})
        chords = len(mine) - len(ticks)
        gap = min((b - a for a, b in zip(ticks, ticks[1:])), default=0)
        rows.append((order, len(mine), chords, gap))
    return rows


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
    if "--density" not in sys.argv[1:]:
        for path, text in files.items():
            path.write_text(text)
            print(f"wrote {path.relative_to(ROOT)}")
    print(f"chart notes={len(notes)} length_ticks={song.length_ticks()}")
    if "--density" in sys.argv[1:]:
        print("order notes chords shortest_gap_ticks")
        for order, n, chords, gap in density(notes, song):
            print(f"{order:5d} {n:5d} {chords:6d} {gap:18d}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
