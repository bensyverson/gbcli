#!/usr/bin/env python3
"""A closed-loop bot that plays Lantern through gbcli, and records its keys.

  python3 tools/bot.py --gbcli PATH --session S [--at start] [--out keys/win.keys]

From checkpoint --at, it reads the maze (wMap) and then repeats: plan a path
by breadth-first search to the nearest key block (the door once every key is
held), turn to face the next stretch, walk it, read the player back. Each
step is one `gbcli run --frames N --keys ... --read ...`; the session's head
moves forward step by step. It writes every key change, at its frame counted
from --at, to --out, so one run replays the whole game:
  gbcli run --at start --frames N --keys-file keys/win.keys --expect '[wState] == 1'
It prints how many gbcli calls it made and its wall time.
"""

import argparse
import math
import os
import re
import subprocess
import sys
import time
from collections import deque

HERE = os.path.dirname(os.path.abspath(__file__))
PROJECT = os.path.normpath(os.path.join(HERE, ".."))

TURN_SPEED = 6      # angle units a frame (defs.inc)
WALK_SPEED = 16     # 1/256 cells a frame at full stride (MOVE_SPEED)
STATE_WON = 1
READS = ["u16[wPosX]", "u16[wPosY]", "u16[wAngle]", "[wKeys]", "[wState]"]


class Bot:
    def __init__(self, opts):
        self.opts = opts
        self.calls = 0
        self.frame = 0          # frames since --at
        self.events = []        # (frame, token)
        self.first = True

    def gbcli(self, args):
        cmd = [self.opts.gbcli] + args + ["--session", self.opts.session, "--dir", self.opts.dir]
        self.calls += 1
        r = subprocess.run(cmd, capture_output=True, text=True)
        if r.returncode not in (0,):
            sys.stderr.write(r.stdout + r.stderr)
            raise SystemExit("gbcli failed: %s" % " ".join(cmd))
        return r.stdout

    def run(self, frames, key=None):
        """Runs `frames` frames, holding `key` for all of them; returns the state."""
        args = ["run", "--frames", str(frames)]
        if self.first:
            args += ["--at", self.opts.at]
            self.first = False
        if key:
            args += ["--keys", "%s@0 -%s@%d" % (key, key, frames)]
            self.events.append((self.frame, key))
            self.events.append((self.frame + frames, "-" + key))
        for r in READS:
            args += ["--read", r]
        out = self.gbcli(args)
        self.frame += frames
        vals = [int(v) for v in re.findall(r"^read expr=\S+ value=(\d+)", out, re.M)]
        if len(vals) != len(READS):
            raise SystemExit("unexpected output:\n" + out)
        return dict(zip(("x", "y", "a", "keys", "state"), vals))

    def read_map(self):
        path = os.path.join(self.opts.dir, "build", "bot-map.bin")
        self.gbcli(["view", "memory", "wMap", "256", "--out", path])
        with open(path, "rb") as f:
            return list(f.read(256))


def plan(cells, start, want):
    """BFS over floor cells from start to a floor cell next to a cell valued
    `want`; returns (path of cells, the target cell)."""
    prev = {start: None}
    todo = deque([start])
    while todo:
        c = todo.popleft()
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            n = (c[0] + dx, c[1] + dy)
            v = cells[n[1] * 16 + n[0]]
            if v == want:
                path = [c]
                while prev[path[-1]] is not None:
                    path.append(prev[path[-1]])
                return path[::-1], n
            if v == 0 and n not in prev:
                prev[n] = c
                todo.append(n)
    raise SystemExit("no path to a cell of value %d" % want)


def angle_to(sx, sy, tx, ty):
    return int(round(math.atan2(ty - sy, tx - sx) * 1024 / (2 * math.pi))) % 1024


def face(bot, st, target_angle):
    diff = (target_angle - st["a"] + 512) % 1024 - 512
    frames = int(round(abs(diff) / TURN_SPEED))
    if frames:
        st = bot.run(frames, "RIGHT" if diff > 0 else "LEFT")
    return st


def walk_to(bot, st, cx, cy):
    """Turns toward the centre of cell (cx, cy) and walks there."""
    tx, ty = cx * 256 + 128, cy * 256 + 128
    st = face(bot, st, angle_to(st["x"], st["y"], tx, ty))
    dist = math.hypot(tx - st["x"], ty - st["y"])
    frames = int(round(dist / WALK_SPEED))
    if frames:
        st = bot.run(frames, "UP")
    return st


def straight_runs(path):
    """The path's corners: the last cell of each straight stretch."""
    out = []
    for i in range(1, len(path)):
        last = i == len(path) - 1
        if last or (path[i + 1][0] - path[i][0], path[i + 1][1] - path[i][1]) != \
                (path[i][0] - path[i - 1][0], path[i][1] - path[i - 1][1]):
            out.append(path[i])
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--gbcli", default="gbcli")
    ap.add_argument("--session", required=True)
    ap.add_argument("--at", default="start")
    ap.add_argument("--dir", default=PROJECT)
    ap.add_argument("--out", default=os.path.join(PROJECT, "keys", "win.keys"))
    opts = ap.parse_args()

    t0 = time.time()
    bot = Bot(opts)
    st = bot.run(1)
    cells = bot.read_map()
    keys_total = sum(1 for v in cells if v == 2)
    while st["state"] != STATE_WON:
        here = (st["x"] >> 8, st["y"] >> 8)
        want = 2 if st["keys"] < keys_total else 3
        path, target = plan(cells, here, want)
        for cx, cy in straight_runs(path):
            st = walk_to(bot, st, cx, cy)
        # Face the key or door and walk into it until it counts.
        st = face(bot, st, angle_to(st["x"], st["y"], target[0] * 256 + 128, target[1] * 256 + 128))
        before = st["keys"]
        for _ in range(4):
            st = bot.run(8, "UP")
            if st["keys"] != before or st["state"] == STATE_WON:
                break
        else:
            raise SystemExit("walked into %s and nothing happened: %s" % (target, st))
        if want == 2:
            cells[target[1] * 16 + target[0]] = 0
        print("frame %d: %s at %s, keys=%d state=%d" % (
            bot.frame, "key" if want == 2 else "door", target, st["keys"], st["state"]))

    with open(opts.out, "w") as f:
        f.write("# Written by tools/bot.py: a winning game from checkpoint %s, %d frames.\n"
                % (opts.at, bot.frame))
        f.write("# gbcli run --at %s --frames %d --keys-file %s --expect '[wState] == 1'\n"
                % (opts.at, bot.frame + 1, os.path.relpath(opts.out, opts.dir)))
        for frame, token in bot.events:
            f.write("%s@%d\n" % (token, frame))
    print("won at frame %d; gbcli calls=%d wall_s=%.1f keys=%s"
          % (bot.frame, bot.calls, time.time() - t0, os.path.relpath(opts.out, opts.dir)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
