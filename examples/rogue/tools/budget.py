#!/usr/bin/env python3
"""The frame budget over part of a run: `gbcli run … --frame-budget` for frames FROM on.

  python3 -I tools/budget.py --from 1713 -- build/rogue.gb --session S --frames 2900 --keys-file K

Runs gbcli run with the arguments after --, adds --frame-budget --json, and
sums up only the frames from FROM (counted from the run's first frame, which
is frame 0 for a run from boot). gbcli comes from PATH, or $GBCLI.
"""

import argparse
import json
import os
import subprocess
import sys


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--from", dest="start", type=int, required=True)
    ap.add_argument("run_args", nargs=argparse.REMAINDER)
    a = ap.parse_args()
    args = [x for x in a.run_args if x != "--"]
    exe = os.environ.get("GBCLI", "gbcli")
    out = subprocess.run([exe, "run", *args, "--frame-budget", "--json"], capture_output=True, text=True)
    res = json.loads(out.stdout)
    if not res.get("ok"):
        sys.exit(f"gbcli run failed: {res.get('error')}")
    frames = res["result"]["frame_budget"]["frames"][a.start:]
    lit = [f for f in frames if not f["lcd_off"] and not f.get("partial")]
    lag = [a.start + i for i, f in enumerate(frames) if f["lag"]]
    act = [f["active_t"] for f in lit]
    busiest = a.start + frames.index(max(lit, key=lambda f: f["active_t"]))
    print(f"frame_budget from={a.start} frames={len(frames)} lag={len(lag)} "
          f"lcd_off={sum(f['lcd_off'] for f in frames)} active_max={max(act)} "
          f"active_mean={sum(act) / len(act):.1f} busiest={busiest}"
          + (f" lag_frames={lag[:10]}" if lag else ""))
    sys.exit(1 if lag else 0)


if __name__ == "__main__":
    main()
