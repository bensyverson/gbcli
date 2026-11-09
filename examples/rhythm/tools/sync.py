#!/usr/bin/env python3
"""Checks that the music the game played lines up with its chart.

Reads the JSON of a gbcli run with --audio-log over the whole song, on the
lead's channel (1) and the bass's (3), takes every trigger write (NR14 or
NR34 with bit 7 set) as a note onset, and checks two things:

  - the song: every lead onset the song text holds was heard, with its
    pitch, in order, at the first onset's time plus its ticks after the
    first note, times the exact tick length (a frame, 140,448 elapsed
    ticks, with the timer off), never rounded, so a per-tick error would
    grow into a visible drift; with the residuals' spread and trend;
  - the chart (assets/music/lantern.chart): every note in it was heard on
    its own channel, with its pitch, at its chart tick by the same clock.
    The chart takes only some of the lead's notes, so the first check is
    the one that shows nothing drifts between them.

Usage (from examples/rhythm; build/perfect.keys from tools/keys.py, which
leaves the title screen and plays every note, so the song runs to its end):
  gbcli run build/rhythm.gb --session sync --from boot --frames 4700 \
      --keys-file build/perfect.keys --audio-log --audio-channel 1 \
      --audio-channel 3 --json > build/sync.json
  python3 -I tools/sync.py build/sync.json --from-frame 63

--from-frame N skips onsets before frame N of the run (the title loop's):
pass the frame the game's clock starts in (tools/keys.py --start).

Exit 0 when every onset is within TOLERANCE of its time, else 1.
"""

import json
import pathlib
import re
import sys
from fractions import Fraction

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from chart import make_chart, SONG, LEAD, BASS  # noqa: E402
from songtext import Song  # noqa: E402

# The trigger register of each charted channel, and how many octaves below
# hUGETracker's name its note sounds (the wave channel's period is twice
# a square's for the same name).
TRIGGERS = {LEAD: (0xFF14, 1), BASS: (0xFF1E, 2)}
FRAME = 140448  # elapsed ticks (2^23 Hz) in a frame
# The driver's work before a channel's trigger write varies with the row
# (more effects, more channels), by a few thousand cycles at most: well
# under a frame. Anything near a frame would be a lost or extra tick.
TOLERANCE = FRAME // 8
NOTE_RE = re.compile(r"= ([A-G]#?)(-?\d+)")
MAX_REPORTED = 10


def huge_to_scientific(name, octaves_down):
    """hUGETracker's name to the pitch the channel sounds, 'C#' kept."""
    letter = name[0] + ("#" if name[1] == "#" else "")
    return f"{letter}{int(name[2]) - octaves_down}"


def onsets(log, addr, from_elapsed):
    out = []
    for w in log:
        if w["addr"] == addr and w["value"] & 0x80 and w["elapsed"] >= from_elapsed:
            m = NOTE_RE.search(w["text"])
            out.append((w["elapsed"], f"{m.group(1)}{m.group(2)}" if m else "?"))
    return out


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
        print(f"--from-frame counts from {since}, not boot", file=sys.stderr)
        return 2
    log = doc["result"]["audio"]["log"]
    heard = {ch: onsets(log, addr, from_frame * FRAME) for ch, (addr, _) in TRIGGERS.items()}
    song = Song(SONG.read_text())
    lead = song.onsets(LEAD)
    chart = make_chart(song)
    num, den = song.frames_per_tick()
    tick_len = Fraction(num, den)
    ok = True

    # The song: every lead onset, in order. The first anchors the clock.
    got = heard[LEAD]
    print(f"song lead onsets={len(lead)} heard={len(got)} "
          f"tick={float(tick_len):.1f} elapsed ({float(tick_len / FRAME):.4f} frames)")
    if len(got) != len(lead):
        print(f"MISMATCH count: song {len(lead)}, heard {len(got)}")
        ok = False
    if not got:
        print("sync FAILED: no lead onsets heard")
        return 1
    t0, tick0 = got[0][0], lead[0][0]

    def expect(tick):
        return t0 + (tick - tick0) * tick_len

    residuals, reported = [], 0
    for i, ((tick, note, order, row), (elapsed, pitch)) in enumerate(zip(lead, got)):
        r = elapsed - expect(tick)
        residuals.append(r)
        want = huge_to_scientific(note, TRIGGERS[LEAD][1])
        if pitch != want or abs(r) > TOLERANCE:
            ok = False
            reported += 1
            if reported <= MAX_REPORTED:
                print(f"MISMATCH lead onset {i} order {order:02d} row {row:02d} tick {tick}:"
                      f" song {want}, heard {pitch}, residual {float(r):+.0f}")
    n = len(residuals)
    xs = [x[0] for x in lead[:n]]
    mx, my = sum(xs) / n, sum(residuals) / n
    sxx = sum((x - mx) ** 2 for x in xs)
    # Least-squares slope of residual against tick: drift per tick.
    slope = sum((x - mx) * (y - my) for x, y in zip(xs, residuals)) / sxx if sxx else 0
    span = xs[-1] - xs[0]
    lo, hi = min(residuals), max(residuals)
    print(f"residual min={float(lo):+.0f} max={float(hi):+.0f} elapsed ticks "
          f"(spread {float(hi - lo) / FRAME:.4f} frames, tolerance {TOLERANCE / FRAME:.3f})")
    print(f"drift slope={float(slope):+.4f} elapsed/tick, "
          f"{float(slope) * span / FRAME:+.4f} frames over {span} ticks")
    last = lead[n - 1]
    print(f"last lead onset: #{n - 1} tick {last[0]} (order {last[2]:02d} row "
          f"{last[3]:02d}) residual {float(residuals[-1]):+.0f} elapsed ticks")

    # The chart: each note heard on its channel at its tick.
    by_channel = {ch: {} for ch in TRIGGERS}
    for ch, hs in heard.items():
        for elapsed, pitch in hs:
            by_channel[ch].setdefault(round((elapsed - t0) / tick_len) + tick0, []).append(
                (elapsed, pitch))
    matched = {ch: 0 for ch in TRIGGERS}
    worst = 0
    for i, x in enumerate(chart):
        want = huge_to_scientific(x.note, TRIGGERS[x.channel][1])
        found = [(e, p) for e, p in by_channel[x.channel].get(x.tick, [])
                 if p == want and abs(e - expect(x.tick)) <= TOLERANCE]
        if found:
            matched[x.channel] += 1
            worst = max(worst, abs(found[0][0] - expect(x.tick)))
        else:
            ok = False
            reported += 1
            if reported <= MAX_REPORTED:
                print(f"MISMATCH chart note {i} tick {x.tick} channel {x.channel + 1}:"
                      f" no {want} heard at its tick")
    if reported > MAX_REPORTED:
        print(f"... {reported - MAX_REPORTED} more mismatches")
    print(f"chart notes={len(chart)} heard at their ticks={sum(matched.values())} "
          f"(lead {matched[LEAD]}, bass {matched[BASS]}); worst residual "
          f"{float(worst):.0f} elapsed ticks")
    print(f"first onset at elapsed {t0} (frame {t0 / FRAME:.2f} since boot)")
    print("sync ok" if ok else "sync FAILED")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
