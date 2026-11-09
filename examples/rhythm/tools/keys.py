#!/usr/bin/env python3
"""Writes a gbcli key file that plays Lantern Drum's chart.

Each note's button goes down on the frame the game reads the note's own
clock tick, plus --shift frames (positive is late), and comes up HOLD frames
later. The game opens on its title screen, so the file first presses
START on frame --title (default 60). Frame offsets count from the run's
start, so --start is the frame (from that start) in which the game's clock
reads 0; find it with

  gbcli run build/rhythm.gb --session cal --from boot --keys 'START@60 -START@63' --until 'u16[wNow] == 0'

whose stop names frame=N: the main loop of frame N reads wNow = 0.

Usage (from examples/rhythm):
  python3 -I tools/keys.py --start N --shift 0 > build/perfect.keys
  python3 -I tools/keys.py --start N --shift 4 > build/late.keys
  [--skip K]   leave every Kth note unpressed (a run with misses)
"""

import argparse
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from chart import make_chart, LANE_KEYS, PREROLL, SONG  # noqa: E402
from songtext import Song  # noqa: E402

HOLD = 3


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--start", type=int, required=True)
    ap.add_argument("--shift", type=int, default=0)
    ap.add_argument("--skip", type=int, default=0)
    ap.add_argument("--title", type=int, default=60)
    args = ap.parse_args()
    chart = make_chart(Song(SONG.read_text()))
    print(f"# Lantern Drum: start={args.start} shift={args.shift} skip={args.skip}, "
          f"{len(chart)} notes")
    print(f"START@{args.title} -START@{args.title + HOLD}  # leave the title screen")
    for i, (tick, lane, note, order, row) in enumerate(chart):
        if args.skip and i % args.skip == args.skip - 1:
            continue
        frame = args.start + PREROLL + tick + args.shift
        key = LANE_KEYS[lane]
        print(f"{key}@{frame} -{key}@{frame + HOLD}  # note {i} tick {tick}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
