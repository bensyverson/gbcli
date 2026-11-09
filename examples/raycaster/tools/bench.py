#!/usr/bin/env python3
"""Measures the renderer in one or more sessions over the scripted walk.

  python3 tools/bench.py --gbcli PATH SESSION...

For each session (made with `gbcli run ROM --session S --frames 120
--checkpoint start`), from checkpoint start with keys/walk.keys held:
  cast    gbcli measure MainLoop..MainLoop.waitCopy --iterations 25
          (casting and drawing one view into a buffer)
  period  gbcli measure ShowView..ShowView --iterations 25
          (one view on screen to the next)
and prints the mean of each in CPU T-cycles and in frames (gbcli's
frame_pct, which allows for double speed), and the max in T-cycles.
"""

import argparse
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
PROJECT = os.path.normpath(os.path.join(HERE, ".."))
SPANS = [("cast", "MainLoop..MainLoop.waitCopy"), ("period", "ShowView..ShowView")]


def measure(opts, session, span):
    cmd = [opts.gbcli, "measure", span, "--session", session, "--dir", opts.dir,
           "--at", "start", "--iterations", str(opts.iterations), "--frames", "2000",
           "--keys-file", "keys/walk.keys"]
    r = subprocess.run(cmd, capture_output=True, text=True)
    m = re.search(r"^t min=(\d+) max=(\d+) mean=([\d.]+)", r.stdout, re.M)
    f = re.search(r"^frame_pct min=[\d.]+ max=[\d.]+ mean=([\d.]+)", r.stdout, re.M)
    if r.returncode != 0 or not m or not f:
        sys.stderr.write(r.stdout + r.stderr)
        raise SystemExit("measure failed: %s" % " ".join(cmd))
    return int(m.group(2)), float(m.group(3)), float(f.group(1)) / 100


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--gbcli", default="gbcli")
    ap.add_argument("--dir", default=PROJECT)
    ap.add_argument("--iterations", type=int, default=25)
    ap.add_argument("sessions", nargs="+")
    opts = ap.parse_args()
    for s in opts.sessions:
        parts = []
        for name, span in SPANS:
            hi, mean, frames = measure(opts, s, span)
            parts.append("%s_mean=%d (%.2f frames) %s_max=%d" % (name, mean, frames, name, hi))
        print("%s %s" % (s, " ".join(parts)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
