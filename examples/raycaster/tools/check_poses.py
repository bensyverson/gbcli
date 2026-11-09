#!/usr/bin/env python3
"""Walks the session to several poses and checks each view with reference.py.

  python3 tools/check_poses.py --gbcli PATH --session ray [--at start] [--dir PROJECT]

Each walk starts from checkpoint --at (a state in the maze with the view
drawn), presses its keys, then waits SETTLE frames with no key held so the
view on screen is the one cast from the final pose. Exit 1 if any differs.
"""

import argparse
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
PROJECT = os.path.normpath(os.path.join(HERE, ".."))

SETTLE = 60

# (keys, frames the keys take): every quadrant of the circle, walks into
# walls, and a long walk across the map.
WALKS = [
    ("", 0),
    ("RIGHT@0 -RIGHT@13 UP@13 -UP@40", 40),
    ("LEFT@0 -LEFT@40", 40),
    ("RIGHT@0 -RIGHT@50 UP@50 -UP@90", 90),
    ("LEFT@0 -LEFT@85 UP@85 -UP@140", 140),
    ("UP@0 -UP@60", 60),
    ("RIGHT@0 -RIGHT@21 UP@21 -UP@150 LEFT@150 -LEFT@170 DOWN@170 -DOWN@200", 200),
]


# Poses set directly with --poke (x, y in 8.8 cells, angle): each faces a key
# or the exit door, so the door's shades and every frame of the key are
# checked too (the first walk's view has a key hidden behind a wall). The
# fourth number is extra frames to settle, which moves the keys' bob.
POKES = [
    (0x0D80, 0x0180, 0, 0),       # the key at (14, 1), ahead: frame 0
    (0x0380, 0x0A80, 768, 20),    # the key at (3, 9), ahead, bobbing up
    (0x0180, 0x0E80, 800, 44),    # the key at (1, 13), ahead and to the right, bobbing down
    (0x0A40, 0x0180, 928, 0),     # the key at (14, 1), to the right: frame 1
    (0x0740, 0x0180, 928, 0),     # farther: frame 2
    (0x0740, 0x0140, 0, 0),       # dead ahead: frame 3
    (0x0780, 0x0140, 960, 0),     # one of its two objects behind a wall
    (0x0E80, 0x0E40, 40, 0),      # the exit door at (15, 14), close
    (0x0B80, 0x0D80, 60, 0),      # the door from farther off
]


def poke_args(x, y, a):
    return ["--poke", "wPosX=%d,%d" % (x & 0xFF, x >> 8),
            "--poke", "wPosY=%d,%d" % (y & 0xFF, y >> 8),
            "--poke", "wAngle=%d,%d" % (a & 0xFF, a >> 8)]


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--gbcli", default="gbcli")
    ap.add_argument("--session", required=True)
    ap.add_argument("--at", default="start")
    ap.add_argument("--dir", default=PROJECT)
    opts = ap.parse_args()

    failed = 0
    runs = [(keys, frames, []) for keys, frames in WALKS]
    runs += [("", wait, poke_args(x, y, a)) for x, y, a, wait in POKES]
    for keys, frames, pokes in runs:
        cmd = [opts.gbcli, "run", "--session", opts.session, "--dir", opts.dir,
               "--at", opts.at, "--frames", str(frames + SETTLE)] + pokes
        if keys:
            cmd += ["--keys", keys]
        r = subprocess.run(cmd, capture_output=True, text=True)
        if r.returncode != 0:
            sys.stderr.write(r.stdout + r.stderr)
            return 2
        r = subprocess.run([sys.executable, "-I", os.path.join(HERE, "reference.py"),
                            "--gbcli", opts.gbcli, "--session", opts.session, "--dir", opts.dir,
                            "--current"],
                           capture_output=True, text=True)
        line = (r.stdout + r.stderr).strip()
        print("%s %s" % (line, "keys='%s'" % keys if not pokes else "poked"))
        if r.returncode != 0:
            failed += 1
    print("%s %d of %d views" % ("ok" if not failed else "FAILED", len(runs) - failed, len(runs)))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
