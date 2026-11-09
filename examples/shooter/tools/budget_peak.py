#!/usr/bin/env python3
"""Summarizes a `gbcli run --frame-budget --json` result: every lag frame,
the busiest frame that isn't one, and the busiest stretch of WINDOW frames.

    gbcli run build/shooter.gb --session budget --frames 6000 \\
        --keys-file keys/win.keys --until '[wState] == 2' \\
        --frame-budget --json > build/budget.json
    python3 -I tools/budget_peak.py build/budget.json

Frame indexes count from the run's start (boot, for a fresh session).
"""

import argparse
import json

FRAME_T = 70224  # T-cycles in a DMG frame


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("json")
    ap.add_argument("--window", type=int, default=60)
    a = ap.parse_args()

    frames = json.load(open(a.json))["result"]["frame_budget"]["frames"]
    lag = [i for i, f in enumerate(frames) if f["lag"]]
    print(f"frames={len(frames)} lag={len(lag)} lag_frames={' '.join(map(str, lag)) or 'none'}")
    busy = [(f["active_t"], i) for i, f in enumerate(frames) if not f["lag"]]
    t, i = max(busy)
    print(f"busiest_frame index={i} active_t={t} share={100 * t / FRAME_T:.1f}%")
    best = None
    for s in range(len(frames) - a.window + 1):
        win = frames[s:s + a.window]
        if any(f["lag"] for f in win):
            continue
        total = sum(f["active_t"] for f in win)
        if best is None or total > best[0]:
            best = (total, s)
    if best:
        total, s = best
        mean = total / a.window
        print(f"busiest_window start={s} frames={a.window} mean_active_t={mean:.0f} "
              f"share={100 * mean / FRAME_T:.1f}% lag=0")


if __name__ == "__main__":
    main()
