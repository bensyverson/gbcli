"""Writes Switchback's two songs as gbcli song text (gbcli help music):

  assets/music/title.song   the title theme: all four channels
  assets/music/race.song    the race theme: channels 1, 3 and 4 only, so
                            channel 2 is free for the sound effects
                            (src/sfx.asm mutes it in the driver)

The grids are generated from short phrase lists here, so the songs can be
rearranged without hand-editing 64-row patterns; the .song files are the
assets the build reads.

Run: python3 -I tools/songs.py (from anywhere).
"""

import os

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.normpath(os.path.join(HERE, "..", "assets", "music"))
EMPTY = "... .. ... ..."

INSTRUMENTS = """
instrument square 1 "Lead"
  envelope 11 down 2
  duty 50%

instrument square 2 "Chord"
  envelope 7 down 1
  duty 25%

instrument square 3 "Long"
  envelope 10 down 4
  duty 50%

instrument wave 1 "Bass"
  level 100%
  waveform 0

instrument noise 1 "Kick"
  envelope 15 down 1

instrument noise 2 "Snare"
  envelope 12 down 1
  mode 15-bit

instrument noise 3 "Hat"
  length 2
  envelope 6 down 1
  mode 7-bit
"""

# A soft square-ish bass wave: 32 four-bit samples.
WAVES = "wave 0 FFFFEEDDCCBA98765432110000112345"


def cell(note=None, inst=None, effect=None):
    if note is None:
        return EMPTY if effect is None else f"... .. ... {effect}"
    return f"{note} {inst:02d} ... {effect or '...'}"


def pattern(name, ch1, ch2, ch3, ch4):
    """Each chN maps row -> cell text."""
    lines = [f"pattern {name}"]
    for row in range(64):
        cells = [c.get(row, EMPTY) for c in (ch1, ch2, ch3, ch4)]
        if all(x == EMPTY for x in cells):
            continue
        lines.append(f"{row:02d} " + " | ".join(cells))
    return "\n".join(lines)


def notes_every(seq, step, inst, start=0, effect=None):
    """seq: notes (or None for a rest), one every `step` rows."""
    out = {}
    for k, n in enumerate(seq):
        if n:
            out[start + k * step] = cell(n, inst, effect)
    return out


def drums(kick, snare, hat):
    out = {}
    for r in hat:
        out[r] = cell("C-6", 3)
    for r in snare:
        out[r] = cell("C-5", 2)
    for r in kick:
        out[r] = cell("C-4", 1)
    return out


def bass_bars(roots):
    """Eighth notes per bar: root, root, octave, root, as 16-row bars."""
    out = {}
    for bar, (low, high) in enumerate(roots):
        for k, n in enumerate([low, low, high, low, low, high, low, high]):
            out[bar * 16 + k * 2] = cell(n, 1)
    return out


def song(name, ticks, orders, patterns):
    head = f'format gbcli-song 1\nname "{name}"\nartist "Switchback"\nticks_per_row {ticks}\n\n{WAVES}\n'
    body = "orders\n" + "\n".join(f"{k:02d} " + " ".join(o) for k, o in enumerate(orders))
    return head + INSTRUMENTS + "\n" + body + "\n\n" + "\n\n".join(patterns) + "\n"


def race():
    bass_a = bass_bars([("A-3", "A-4"), ("G-3", "G-4"), ("F-3", "F-4"), ("E-3", "E-4")])
    bass_b = bass_bars([("F-3", "F-4"), ("G-3", "G-4"), ("A-3", "A-4"), ("E-3", "E-4")])
    beat = drums(kick=[0, 8, 16, 24, 32, 40, 48, 56], snare=[4, 12, 20, 28, 36, 44, 52, 60],
                 hat=list(range(2, 64, 4)))
    beat_fill = dict(beat)
    for r in (56, 58, 60, 62):
        beat_fill[r] = cell("C-5", 2)
    lead_a = notes_every(["E-5", None, "A-5", "B-5", "C-6", None, "B-5", "A-5",
                          "G-5", None, "D-5", "G-5", "E-5", None, None, None], 4, 1)
    lead_b = notes_every(["F-5", "A-5", "C-6", "A-5", "G-5", "B-5", "D-6", "B-5",
                          "A-5", "C-6", "E-6", "C-6", "B-5", None, "G#5", None], 4, 1)
    lead_b[60] = cell("G#5", 1, "437")
    rest = {}
    return song("Switchback: Race", 4,
                [("P0", "P9", "P1", "P2"), ("P3", "P9", "P1", "P2"),
                 ("P4", "P9", "P5", "P6"), ("P3", "P9", "P1", "P6")],
                [pattern("P0 P9 P1 P2", {}, rest, bass_a, beat),
                 pattern("P3 P9 P1 P2", lead_a, rest, bass_a, beat),
                 pattern("P4 P9 P5 P6", lead_b, rest, bass_b, beat_fill),
                 pattern("P3 P9 P1 P6", lead_a, rest, bass_a, beat_fill)])


def title():
    chords = {}
    # Arpeggiated chords on channel 2: A minor, F, C, G (0xy arpeggio).
    for bar, (root, arp) in enumerate([("A-4", "037"), ("F-4", "047"), ("C-5", "047"), ("G-4", "047")]):
        for k in range(4):
            chords[bar * 16 + k * 4] = cell(root, 2, arp)
    bass = {}
    for bar, n in enumerate(["A-3", "F-3", "C-4", "G-3"]):
        bass[bar * 16] = cell(n, 1)
        bass[bar * 16 + 8] = cell(n, 1)
        bass[bar * 16 + 12] = cell(n, 1)
    lead = notes_every(["A-5", None, None, "C-6", "E-6", None, "D-6", "C-6",
                        "G-5", None, None, "C-6", "D-6", None, "B-5", None], 4, 3)
    lead[60] = cell("B-5", 3, "E06")
    beat = drums(kick=[0, 16, 24, 32, 48, 56], snare=[8, 24, 40, 56], hat=list(range(4, 64, 8)))
    return song("Switchback: Title", 6,
                [("P0", "P1", "P2", "P3"), ("P4", "P1", "P2", "P3")],
                [pattern("P0 P1 P2 P3", {}, chords, bass, beat),
                 pattern("P4 P1 P2 P3", lead, chords, bass, beat)])


def main():
    os.makedirs(OUT, exist_ok=True)
    for name, text in (("title", title()), ("race", race())):
        path = os.path.join(OUT, f"{name}.song")
        with open(path, "w") as f:
            f.write(text)
        print(f"wrote {path}")


if __name__ == "__main__":
    main()
