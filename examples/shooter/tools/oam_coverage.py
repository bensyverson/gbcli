#!/usr/bin/env python3
"""Shows the sprite multiplexer at work: steps a gbcli session one frame at a
time and lists, per frame, which objects OAM actually drew.

An object counts as drawn in a frame when `gbcli view oam` puts it on screen
and not dropped by the 10-per-line limit. Objects are told apart by tile
number, as the test scene gives each its own (tiles FIRST..FIRST+COUNT-1).
Exits 1 if some object was never drawn in the frames stepped.

    python3 -I tools/oam_coverage.py --session mux --frames 6

Run from the project folder, on a session past start-up. Each frame stops at
VBlank before the DMA, so OAM then holds what the frame just showed.
"""

import argparse
import json
import subprocess
import sys


def gbcli(exe, *args):
    out = subprocess.run([exe, *args, "--json"], capture_output=True, text=True)
    if out.returncode != 0:
        sys.exit(f"gbcli {' '.join(args)} failed ({out.returncode}): {out.stdout}{out.stderr}")
    return json.loads(out.stdout.splitlines()[-1])


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--session", required=True)
    ap.add_argument("--frames", type=int, default=6)
    ap.add_argument("--first", type=lambda s: int(s, 0), default=0x10, help="first object tile")
    ap.add_argument("--count", type=int, default=48, help="objects in the scene")
    ap.add_argument("--verbose", action="store_true", help="a row per frame")
    ap.add_argument("--gbcli", default="gbcli", help="the gbcli binary")
    a = ap.parse_args()

    wanted = set(range(a.first, a.first + a.count))
    seen = set()
    run = dict.fromkeys(wanted, 0)  # frames each object has gone undrawn
    worst = 0
    per_frame = []
    for _ in range(a.frames):
        stop = gbcli(a.gbcli, "run", "--session", a.session, "--frames", "1")
        frame = stop["result"]["stop"]["time"]["frame"]
        objs = gbcli(a.gbcli, "view", "oam", "--session", a.session)["result"]["view"]["data"]["objects"]
        drawn = {o["tile"] for o in objs if o["on_screen"] and not o["dropped_by_line_limit"]}
        in_oam = {o["tile"] for o in objs if o["y"] != 0}
        drawn &= wanted
        seen |= drawn
        per_frame.append(len(drawn))
        missing = sorted(wanted - drawn)
        for t in wanted:
            run[t] = 0 if t in drawn else run[t] + 1
            worst = max(worst, run[t])
        if a.verbose:
            print(f"frame={frame} in_oam={len(in_oam & wanted)} drawn={len(drawn)} "
                  f"line_dropped={len((in_oam & wanted) - drawn)} not_drawn={' '.join(f'${t:02X}' for t in missing)}")
    never = sorted(wanted - seen)
    print(f"objects={a.count} frames={a.frames} drawn_some_frame={len(seen & wanted)} "
          f"drawn_per_frame_min={min(per_frame)} drawn_per_frame_mean={sum(per_frame) / len(per_frame):.1f} "
          f"longest_undrawn_run={worst} never_drawn={' '.join(f'${t:02X}' for t in never) or 'none'}")
    sys.exit(1 if never else 0)


if __name__ == "__main__":
    main()
