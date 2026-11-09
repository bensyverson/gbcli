#!/usr/bin/env python3
"""Checks that the music the game played lines up with its chart.

Reads the JSON of a gbcli run with --audio-log over the whole song, takes
every trigger write on the chart's channel (NR14 with bit 7 set) as a note
onset, and compares the onsets with assets/music/lantern.chart:

  - the same number of notes, with the same pitches, in the same order;
  - each onset at the first onset's time plus its ticks after the first note, times the exact
    tick length (a frame, 140,448 elapsed ticks, with the timer off), never
    rounded, so a per-tick error would grow into a visible drift;
  - the residuals' spread and their trend from the first note to the last.

Usage (from examples/rhythm; build/perfect.keys from tools/keys.py, which
leaves the title screen and plays every note, so the song runs to its end):
  gbcli run build/rhythm.gb --session sync --from boot --frames 4700 \
      --keys-file build/perfect.keys --audio-log --audio-channel 1 --json > build/sync.json
  python3 -I tools/sync.py build/sync.json --from-frame 63

--from-frame N skips onsets before frame N of the run (the title loop's):
pass the frame the game's clock starts in (tools/keys.py --start).

Exit 0 when every onset is within TOLERANCE of its chart time, else 1.
"""

import json
import pathlib
import re
import sys
from fractions import Fraction

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from chart import make_chart, SONG  # noqa: E402
from songtext import Song  # noqa: E402

NR14 = 0xFF14
FRAME = 140448  # elapsed ticks (2^23 Hz) in a frame
# The driver's work before a channel's trigger write varies with the row
# (more effects, more channels), by a few thousand cycles at most: well
# under a frame. Anything near a frame would be a lost or extra tick.
TOLERANCE = FRAME // 8
NOTE_RE = re.compile(r"= ([A-G]#?)(-?\d+)")


def huge_to_scientific(name):
    """hUGETracker's C-5 sounds as C4: one octave down, 'C#' kept."""
    letter = name[0] + ("#" if name[1] == "#" else "")
    return f"{letter}{int(name[2]) - 1}"


def onsets(log):
    out = []
    for w in log:
        if w["addr"] == NR14 and w["value"] & 0x80:
            m = NOTE_RE.search(w["text"])
            out.append((w["elapsed"], f"{m.group(1)}{m.group(2)}" if m else "?"))
    return out


MAX_REPORTED = 10


def main(argv):
    from_frame = 0
    if "--from-frame" in argv:
        i = argv.index("--from-frame")
        from_frame = int(argv[i + 1])
        argv = argv[:i] + argv[i + 2:]
    if len(argv) != 2:
        print(__doc__, file=sys.stderr)
        return 2
    doc = json.loads(pathlib.Path(argv[1]).read_text())
    if not doc.get("ok"):
        print(f"gbcli run failed: {doc.get('error')}", file=sys.stderr)
        return 2
    since = doc["result"]["stop"]["time"]["since"]["kind"]
    if from_frame and since != "boot":
        print(f"--from-frame counts from boot, but the run counts from {since}", file=sys.stderr)
        return 2
    heard = [o for o in onsets(doc["result"]["audio"]["log"]) if o[0] >= from_frame * FRAME]
    song = Song(SONG.read_text())
    chart = make_chart(song)
    num, den = song.frames_per_tick()
    tick_len = Fraction(num, den)

    print(f"chart notes={len(chart)} heard onsets={len(heard)} "
          f"tick={float(tick_len):.1f} elapsed ({float(tick_len / FRAME):.4f} frames)")
    ok = True
    if len(heard) != len(chart):
        print(f"MISMATCH count: chart {len(chart)}, heard {len(heard)}")
        ok = False
    t0 = heard[0][0] if heard else 0
    residuals = []
    reported = 0
    for i, ((tick, lane, note, order, row), (elapsed, pitch)) in enumerate(zip(chart, heard)):
        expect = t0 + (tick - chart[0][0]) * tick_len  # the first note anchors
        r = elapsed - expect
        residuals.append(r)
        want = huge_to_scientific(note)
        bad_pitch = pitch != want
        bad_time = abs(r) > TOLERANCE
        if bad_pitch or bad_time:
            ok = False
            reported += 1
            if reported > MAX_REPORTED:
                continue
            print(f"MISMATCH note {i} order {order:02d} row {row:02d} tick {tick}: "
                  f"chart {want}, heard {pitch}, residual {float(r):+.0f}")
    if reported > MAX_REPORTED:
        print(f"... {reported - MAX_REPORTED} more mismatches")
    if residuals:
        lo, hi = min(residuals), max(residuals)
        n = len(residuals)
        # Least-squares slope of residual against chart tick: drift per tick.
        xs = [c[0] for c in chart[:n]]
        mx = sum(xs) / n
        my = sum(residuals) / n
        sxx = sum((x - mx) ** 2 for x in xs)
        slope = sum((x - mx) * (y - my) for x, y in zip(xs, residuals)) / sxx if sxx else 0
        span = xs[-1] - xs[0]
        last = chart[n - 1]
        print(f"residual min={float(lo):+.0f} max={float(hi):+.0f} elapsed ticks "
              f"(spread {float(hi - lo) / FRAME:.4f} frames, tolerance "
              f"{TOLERANCE / FRAME:.3f})")
        print(f"drift slope={float(slope):+.4f} elapsed/tick, "
              f"{float(slope) * span / FRAME:+.4f} frames over {span} ticks")
        print(f"last note: #{n - 1} tick {last[0]} (order {last[3]:02d} row "
              f"{last[4]:02d}) residual {float(residuals[-1]):+.0f} elapsed ticks")
        print(f"first onset at elapsed {t0} (frame {t0 / FRAME:.2f} since boot)")
    print("sync ok" if ok else "sync FAILED")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
