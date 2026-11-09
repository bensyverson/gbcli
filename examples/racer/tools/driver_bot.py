"""Drives a lap through gbcli and writes the inputs as a keys file.

A closed loop: every CHUNK frames it reads the car's offset from the road's
center (wPlayerX) and steers back toward the center line, where neither
rivals (lanes at +/-0.45 of the road) nor trees (off the road) can be hit.
Held keys persist across gbcli runs, so the file records only changes, and
replaying it in one run repeats the lap exactly (the emulator is
deterministic):

  gbcli run build/racer.gb --session win --keys-file tools/win.keys --until '[wState] == 2'

Run: python3 -I tools/driver_bot.py [GBCLI] (from anywhere; GBCLI defaults
to `gbcli` on PATH). It replaces the session `bot`.
"""

import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
PROJECT = os.path.normpath(os.path.join(HERE, ".."))
OUT = os.path.join(HERE, "win.keys")
EXE = sys.argv[1] if len(sys.argv) > 1 else "gbcli"
CHUNK = 6            # frames between looks
DEAD_BAND = 6        # pixels off center before steering
STATE_WON, STATE_LOST = 2, 3
MAX_FRAMES = 4200
TITLE_FRAMES = 40     # the first run: the title screen, through START


def run(keys, first, frames=CHUNK):
    args = [EXE, "run"] + (["build/racer.gb"] if first else []) + [
        "--session", "bot", "--frames", str(frames),
        "--read", "[wPlayerX]", "--read", "[wState]", "--json"]
    if keys:
        args += ["--keys", " ".join(keys)]
    out = subprocess.run(args, cwd=PROJECT, capture_output=True, text=True)
    if out.returncode != 0:
        raise SystemExit(f"{' '.join(args)} failed ({out.returncode}): {out.stderr.strip()}")
    reads = {r["expr"]: r["value"] for r in json.loads(out.stdout)["result"]["reads"]}
    px = reads["[wPlayerX]"]
    return (px - 256 if px > 127 else px), reads["[wState]"]


def main():
    subprocess.run([EXE, "sessions", "rm", "bot"], cwd=PROJECT, capture_output=True)
    # START on the title screen, then hold A for the whole race.
    tokens = ["A@0", "START@30", "-START@32"]
    keys = list(tokens)  # applied at the start of the next chunk
    held = None
    frame = 0
    state = 0
    while frame < MAX_FRAMES:
        chunk = TITLE_FRAMES if frame == 0 else CHUNK
        px, state = run(keys, frame == 0, chunk)
        frame += chunk
        if state in (STATE_WON, STATE_LOST):
            break
        want = "LEFT" if px > DEAD_BAND else "RIGHT" if px < -DEAD_BAND else None
        keys = []
        if want != held:
            if held:
                keys.append(f"-{held}@0")
                tokens.append(f"-{held}@{frame}")
            if want:
                keys.append(f"{want}@0")
                tokens.append(f"{want}@{frame}")
            held = want
    with open(OUT, "w") as f:
        f.write("# A winning lap, recorded by tools/driver_bot.py: hold A, keep to the center line.\n")
        f.write("# gbcli run build/racer.gb --session win --keys-file tools/win.keys --until '[wState] == 2'\n")
        for t in tokens:
            f.write(t + "\n")
    print(f"state {state} after {frame} frames; wrote {OUT}: {len(tokens)} tokens")


if __name__ == "__main__":
    main()
