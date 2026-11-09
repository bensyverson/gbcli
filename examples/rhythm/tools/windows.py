#!/usr/bin/env python3
"""Plays the whole chart at several press offsets and prints the judgments,
so the windows (perfect within 2 ticks, good within 6) can be seen at their
edges. Each offset is one gbcli run from boot with a key file from keys.py.

Usage (from examples/rhythm, after gbcli build):
  python3 -I tools/windows.py [GBCLI] [--start 63] [--shifts -7,-6,-3,-2,0,2,3,6,7]
GBCLI defaults to `gbcli` on PATH.
"""

import argparse
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
FRAMES = "4700"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("gbcli", nargs="?", default="gbcli")
    ap.add_argument("--start", default="63")
    ap.add_argument("--shifts", default="-7,-6,-3,-2,0,2,3,6,7")
    args = ap.parse_args()
    keys = ROOT / "build" / "window.keys"
    print("shift perfect good miss")
    for shift in args.shifts.split(","):
        text = subprocess.run(
            [sys.executable, "-I", str(ROOT / "tools" / "keys.py"),
             "--start", args.start, "--shift", shift],
            check=True, capture_output=True, text=True).stdout
        keys.write_text(text)
        out = subprocess.run(
            [args.gbcli, "run", "build/rhythm.gb", "--session", "windows",
             "--from", "boot", "--frames", FRAMES, "--keys-file", str(keys),
             "--read", "[wPerfect]", "--read", "[wGood]", "--read", "[wMiss]"],
            cwd=ROOT, check=True, capture_output=True, text=True).stdout
        vals = [line.split("value=")[1].split()[0]
                for line in out.splitlines() if line.startswith("read ")]
        print(f"{int(shift):+5d} {vals[0]:>7} {vals[1]:>4} {vals[2]:>4}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
