#!/usr/bin/env python3
"""Writes Lantern Drum's two songs as gbcli song text:

  assets/music/lantern.song   the song you play: channel 1 (the lead) is the
                              chart, which tools/chart.py reads from it
  assets/music/title.song     the title screen's loop

Both leave channel 2 silent: it belongs to the hit and miss sounds, so an
effect never cuts a note of the music, whenever it lands.

The song text is what the build reads; this script exists because a pattern
several orders share must be spelled out in every block that plays it.

Run: python3 -I tools/compose.py [--check]   (from examples/rhythm)
"""

import pathlib
import sys

MUSIC = pathlib.Path(__file__).resolve().parent.parent / "assets" / "music"

NAMES = ["C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-"]
ROOT = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}


def note(n):
    """Semitones from C-3 to hUGETracker's name (C-3 sounds as C2)."""
    return f"{NAMES[n % 12]}{3 + n // 12}"


def chord_root(name, octave=0):
    """'Am' or 'E' to the root's semitone number in octave 3 (+octave)."""
    return ROOT[name[0]] + 12 * octave


# ---------------------------------------------------------------------------
# The game song: A minor, 7 ticks a row (one tick a frame: 128 BPM).

LEADS = {
    "A": {0: "E-6", 4: "A-5", 8: "C-6", 10: "B-5", 12: "A-5",
          16: "F-5", 20: "A-5", 24: "C-6", 28: "A-5",
          32: "G-5", 36: "C-6", 40: "E-6", 44: "D-6", 46: "C-6",
          48: "B-5", 52: "G-5", 56: "D-6"},
    "B": {0: "A-5", 2: "C-6", 4: "E-6", 6: "C-6", 8: "A-6", 12: "G-6", 14: "E-6",
          16: "F-6", 18: "E-6", 20: "C-6", 24: "A-5", 26: "C-6", 28: "F-6",
          32: "E-6", 34: "D-6", 36: "C-6", 38: "D-6", 40: "E-6", 44: "G-6",
          48: "D-6", 50: "B-5", 52: "G-5", 56: "B-5", 58: "D-6", 60: "G-6"},
    "C": {0: "A-6", 3: "G-6", 6: "F-6", 8: "C-6", 12: "F-6",
          16: "G-6", 19: "F-6", 22: "D-6", 24: "B-5", 28: "D-6",
          32: "E-6", 35: "G-6", 38: "B-6", 40: "G-6", 44: "E-6",
          48: "A-6", 52: "E-6", 54: "C-6", 56: "A-5", 60: "C-6"},
    "D": {0: "D-6", 8: "F-6", 16: "E-6", 24: "C-6",
          32: "D-6", 36: "F-6", 40: "A-6", 44: "F-6",
          48: "G#6", 52: "E-6", 56: "B-5", 60: "G#5"},
    "O": {0: "A-5", 8: "C-6", 16: "E-6", 32: "A-6"},
    "-": {},
}
LEADS["C2"] = {**LEADS["C"], 10: "D-6", 26: "C-6", 42: "F-6", 58: "B-5", 62: "E-6"}

PROGRESSIONS = {
    "verse": ["A", "F", "C", "G"],
    "chorus": ["F", "G", "E", "A"],
    "bridge": ["D", "A", "D", "E"],
    "end": ["A", "A", "A", "A"],
}

# Each order: lead pattern, chords, drum style.
SONG = [
    ("-", "verse", "intro"),
    ("A", "verse", "groove"),
    ("B", "verse", "groove"),
    ("C", "chorus", "drive"),
    ("A", "verse", "groove"),
    ("B", "verse", "groove"),
    ("D", "bridge", "sparse"),
    ("C", "chorus", "drive"),
    ("C2", "chorus", "drive"),
    ("O", "end", "end"),
]


def bass(chords):
    rows = {}
    for bar, name in enumerate(chords):
        r = chord_root(name)
        fifth = r + 7
        for off, n in ((0, r), (4, r + 12), (8, r), (12, fifth)):
            rows[bar * 16 + off] = (note(n), 1)
    return rows


KICK, SNARE, HAT = ("C-5", 2), ("A-5", 3), ("F-6", 1)


def drums(style):
    rows = {}
    for bar in range(4):
        b = bar * 16
        if style == "intro":
            hits = {2: HAT, 6: HAT, 10: HAT, 14: HAT}
            if bar >= 2:
                hits.update({0: KICK, 8: KICK})
            if bar == 3:
                hits.update({12: SNARE, 14: SNARE})
        elif style == "groove":
            hits = {0: KICK, 4: SNARE, 8: KICK, 12: SNARE,
                    2: HAT, 6: HAT, 10: HAT, 14: HAT}
        elif style == "drive":
            hits = {0: KICK, 4: SNARE, 6: KICK, 8: KICK, 12: SNARE,
                    2: HAT, 10: HAT, 14: HAT}
        elif style == "sparse":
            hits = {0: KICK, 8: SNARE}
            if bar == 3:
                hits.update({10: SNARE, 12: SNARE, 14: SNARE})
        else:  # end
            hits = {0: KICK} if bar < 2 else {}
        for off, hit in hits.items():
            rows[b + off] = hit
    return rows


# ---------------------------------------------------------------------------
# The title loop: slow (10 ticks a row), lead and bass with a soft hat.

TITLE_LEADS = {
    "T1": {0: "A-5", 6: "C-6", 8: "E-6", 16: "D-6", 22: "C-6", 24: "A-5",
           32: "G-5", 38: "C-6", 40: "E-6", 48: "D-6", 56: "B-5"},
    "T2": {0: "C-6", 6: "E-6", 8: "A-6", 16: "G-6", 22: "F-6", 24: "E-6",
           32: "D-6", 40: "C-6", 48: "B-5", 52: "C-6", 56: "E-6"},
}
TITLE_SONG = [("T1", "verse", "title"), ("T2", "verse", "title")]


def title_drums(style):
    return {bar * 16 + off: HAT for bar in range(4) for off in (4, 12)}


# ---------------------------------------------------------------------------

def cell(entry, inst_default=1):
    if entry is None:
        return "... .. ... ..."
    if isinstance(entry, tuple):
        name, inst = entry
    else:
        name, inst = entry, inst_default
    return f"{name} {inst:02d} ... ..."


INSTRUMENTS = """\
instrument square 1 "Lead"
  envelope 12 down 2
  duty 50%

instrument square 2 "Soft lead"
  envelope 9 down 3
  duty 25%

instrument wave 1 "Bass"
  level 50%
  waveform 4

instrument noise 1 "Hat"
  length 4
  envelope 6 down 1
  mode 7-bit

instrument noise 2 "Kick"
  envelope 15 down 2

instrument noise 3 "Snare"
  envelope 12 down 2
"""


def song_text(name, comment, ticks_per_row, orders, leads, drum_fn, lead_inst):
    # Pattern names: one per distinct lead, bass progression and drum style.
    lead_ids, bass_ids, drum_ids = {}, {}, {}

    def pid(table, key, base):
        if key not in table:
            table[key] = f"P{base + len(table)}"
        return table[key]

    rows = []
    for lead, prog, style in orders:
        rows.append((pid(lead_ids, lead, 0), "P90", pid(bass_ids, prog, 40),
                     pid(drum_ids, style, 60), lead, prog, style))
    out = [
        "format gbcli-song 1",
        f'name "{name}"',
        'artist "rhythm15"',
        f'comment "{comment}"',
        f"ticks_per_row {ticks_per_row}",
        "timer off",
        "",
        INSTRUMENTS,
        "orders",
    ]
    for i, r in enumerate(rows):
        out.append(f"{i:02d} {r[0]} {r[1]} {r[2]} {r[3]}")
    seen = set()
    for p0, p1, p2, p3, lead, prog, style in rows:
        if (p0, p2, p3) in seen:
            continue
        seen.add((p0, p2, p3))
        lead_rows = leads[lead]
        bass_rows = bass(PROGRESSIONS[prog])
        drum_rows = drum_fn(style)
        out.append("")
        out.append(f"pattern {p0} {p1} {p2} {p3}")
        for row in range(64):
            c = [cell(lead_rows.get(row), lead_inst), cell(None),
                 cell(bass_rows.get(row)), cell(drum_rows.get(row))]
            if any(x != "... .. ... ..." for x in c):
                out.append(f"{row:02d} " + " | ".join(c))
    return "\n".join(out) + "\n"


def songs():
    return {
        MUSIC / "lantern.song": song_text(
            "Lantern Drum", "Channel 1 is the chart (tools/chart.py); channel 2 is the effects'.",
            7, SONG, LEADS, drums, 1),
        MUSIC / "title.song": song_text(
            "Lantern Drum title", "The title loop; channel 2 is left to the effects.",
            10, TITLE_SONG, TITLE_LEADS, title_drums, 2),
    }


def main():
    files = songs()
    if "--check" in sys.argv[1:]:
        stale = [p for p, t in files.items() if not p.exists() or p.read_text() != t]
        for p in stale:
            print(f"stale: {p.name} (run tools/compose.py)", file=sys.stderr)
        return 1 if stale else 0
    for path, text in files.items():
        path.write_text(text)
        print(f"wrote {path.name}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
