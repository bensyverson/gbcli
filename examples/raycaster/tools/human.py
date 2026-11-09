#!/usr/bin/env python3
"""Plays Lantern the way a lost human does, through gbcli, and checks the lamp lasts.

  python3 tools/human.py --gbcli PATH --session S [--at start] [--out keys/human.keys]
  python3 tools/human.py --plan     (the route and its cost, from tools/gen_maze.py; no gbcli)

The bot (tools/bot.py) knows the map and walks the shortest path. This
player doesn't: it explores by the left-hand rule (keep a hand on the left
wall), so it walks into every dead end on its way and backtracks out, and
it takes a key only when the wall leads it through the key's cell. Once it
holds every key it keeps following the wall until it reaches the door.
It plays at a person's pace, not a machine's:
  - a turn overshoots by OVERSHOOT frames and is corrected back after a
    REACT-frame pause;
  - it stops to look for LOOK frames at every cell where the way bends or
    splits, and for DEAD_END frames at a dead end before turning round.
Each step is one `gbcli run --frames N --keys ... --read ...`, from
checkpoint --at; it writes every key change to --out, so one run replays
the game (`gbcli run --at start --frames N --keys-file keys/human.keys
--expect '[wState] == 1'`). Exit 0 if it wins; it prints the frames it took,
the oil left (wOilPx of OIL_PIXELS), the least it had at any step, and
gbcli's calls.
"""

import argparse
import math
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
PROJECT = os.path.normpath(os.path.join(HERE, ".."))
sys.path.insert(0, HERE)

TURN_SPEED = 6      # angle units a frame (defs.inc)
WALK_SPEED = 16     # 1/256 cells a frame
STATE_WON = 1
OIL_PIXELS = 128

OVERSHOOT = 4       # frames a turn runs past its mark
REACT = 12          # frames before the correction
LOOK = 20           # frames looking at a bend or a fork
DEAD_END = 30       # frames at a dead end

# Headings: east, south, west, north (angle 0, 256, 512, 768).
DIRS = [(1, 0), (0, 1), (-1, 0), (0, -1)]
READS = ["u16[wPosX]", "u16[wPosY]", "u16[wAngle]", "[wKeys]", "[wState]", "[wOilPx]"]


def route(cells, start, keys_total):
    """The left-hand walk from start, facing east: a list of (cell, heading)
    moves, and the door's cell once every key is held (None if it never
    gets there). cells: 16x16 values, 0 floor, 1 stone, 2 key, 3 door."""
    pos, heading = start, 0
    keys = set()
    moves = []
    for _ in range(2000):
        if cells[pos[1] * 16 + pos[0]] == 2:
            keys.add(pos)
        if len(keys) == keys_total:
            for d, (dx, dy) in enumerate(DIRS):
                n = (pos[0] + dx, pos[1] + dy)
                if cells[n[1] * 16 + n[0]] == 3:
                    return moves, (n, d)
        for turn in (3, 0, 1, 2):           # left, ahead, right, back
            d = (heading + turn) % 4
            n = (pos[0] + DIRS[d][0], pos[1] + DIRS[d][1])
            if cells[n[1] * 16 + n[0]] in (0, 2):
                heading, pos = d, n
                moves.append((pos, d))
                break
    return moves, None


def open_ways(cells, c):
    return sum(1 for dx, dy in DIRS if cells[(c[1] + dy) * 16 + c[0] + dx] in (0, 2))


def plan_cost(cells, start, keys_total):
    """Estimated frames for the route at this player's pace."""
    moves, door = route(cells, start, keys_total)
    frames = 0
    heading = 0
    prev = start
    for cell, d in moves:
        turn = (d - heading) % 4
        if turn:
            ways = open_ways(cells, prev)
            frames += DEAD_END if ways == 1 else LOOK
            frames += (256 * (2 if turn == 2 else 1)) // TURN_SPEED + 2 * OVERSHOOT + REACT
        frames += 256 // WALK_SPEED
        heading, prev = d, cell
    return len(moves), door, frames


class Player:
    def __init__(self, opts):
        self.opts = opts
        self.calls = 0
        self.frame = 0
        self.events = []
        self.first = True
        self.low = OIL_PIXELS   # the least oil seen at any step

    def gbcli(self, args):
        cmd = [self.opts.gbcli] + args + ["--session", self.opts.session, "--dir", self.opts.dir]
        self.calls += 1
        r = subprocess.run(cmd, capture_output=True, text=True)
        if r.returncode != 0:
            sys.stderr.write(r.stdout + r.stderr)
            raise SystemExit("gbcli failed: %s" % " ".join(cmd))
        return r.stdout

    def run(self, frames, key=None):
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
        st = dict(zip(("x", "y", "a", "keys", "state", "oil"), vals))
        self.low = min(self.low, st["oil"])
        return st

    def read_cells(self):
        """The maze as the player meets it: wMap, with the keys still on wKeyCell."""
        path = os.path.join(self.opts.dir, "build", "human-map.bin")
        self.gbcli(["view", "memory", "wMap", "256", "--out", path])
        with open(path, "rb") as f:
            cells = list(f.read(256))
        out = self.gbcli(["read", "[wKeyCell]", "[wKeyCell+1]", "[wKeyCell+2]"])
        for v in re.findall(r"^read expr=\S+ value=(\d+)", out, re.M):
            if int(v) != 0xFF:
                cells[int(v)] = 2
        return cells


def turn_to(p, st, target, sloppy):
    """Turns to `target`, overshooting and correcting back when sloppy."""
    diff = (target - st["a"] + 512) % 1024 - 512
    frames = int(round(abs(diff) / TURN_SPEED))
    if not frames:
        return st
    key, back = ("RIGHT", "LEFT") if diff > 0 else ("LEFT", "RIGHT")
    if sloppy:
        st = p.run(frames + OVERSHOOT, key)
        st = p.run(REACT)
        st = p.run(OVERSHOOT, back)
    else:
        st = p.run(frames, key)
    return st


def angle_to(sx, sy, tx, ty):
    return int(round(math.atan2(ty - sy, tx - sx) * 1024 / (2 * math.pi))) % 1024


def go_to(p, st, cell, sloppy):
    tx, ty = cell[0] * 256 + 128, cell[1] * 256 + 128
    st = turn_to(p, st, angle_to(st["x"], st["y"], tx, ty), sloppy)
    frames = int(round(math.hypot(tx - st["x"], ty - st["y"]) / WALK_SPEED))
    if frames:
        st = p.run(frames, "UP")
    return st


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--gbcli", default="gbcli")
    ap.add_argument("--session")
    ap.add_argument("--at", default="start")
    ap.add_argument("--dir", default=PROJECT)
    ap.add_argument("--out", default=os.path.join(PROJECT, "keys", "human.keys"))
    ap.add_argument("--plan", action="store_true", help="print the route's cost from gen_maze.py's maze")
    opts = ap.parse_args()

    if opts.plan:
        import gen_maze
        rows, start = gen_maze.parse()
        cells = [gen_maze.CELLS[c] for row in rows for c in row]
        n, door, frames = plan_cost(cells, start, sum(1 for v in cells if v == 2))
        print("route cells=%d door=%s frames~%d" % (n, door and door[0], frames))
        return 0 if door else 1
    if not opts.session:
        ap.error("--session is required (or --plan)")

    p = Player(opts)
    st = p.run(1)
    cells = p.read_cells()
    start = (st["x"] >> 8, st["y"] >> 8)
    moves, door = route(cells, start, sum(1 for v in cells if v == 2))
    if door is None:
        raise SystemExit("the left-hand walk never reaches the door with every key")
    heading = 0
    prev = start
    i = 0
    while i < len(moves):
        cell, d = moves[i]
        if d != heading:
            # A bend, a fork or a dead end: stop and look, then turn.
            p.run(DEAD_END if open_ways(cells, prev) == 1 else LOOK)
        # Walk the straight run as one press, to its last cell.
        j = i
        while j + 1 < len(moves) and moves[j + 1][1] == d:
            j += 1
        st = go_to(p, st, moves[j][0], d != heading)
        heading, prev = d, moves[j][0]
        i = j + 1
    # Every key held: face the door and walk into it.
    door_cell, d = door
    p.run(LOOK)
    st = turn_to(p, st, d * 256, True)
    for _ in range(6):
        st = p.run(8, "UP")
        if st["state"] == STATE_WON:
            break
    won = st["state"] == STATE_WON

    with open(opts.out, "w") as f:
        f.write("# Written by tools/human.py: a lost human's game from checkpoint %s, %d frames:\n"
                % (opts.at, p.frame))
        f.write("# the left-hand rule, with pauses and sloppy turns. Oil left: %d of %d.\n"
                % (st["oil"], OIL_PIXELS))
        f.write("# gbcli run --at %s --frames %d --keys-file %s --expect '[wState] == 1'\n"
                % (opts.at, p.frame + 1, os.path.relpath(opts.out, opts.dir)))
        for frame, token in p.events:
            f.write("%s@%d\n" % (token, frame))
    print("%s at frame %d; keys=%d oil=%d/%d (%d%%) lowest=%d cells=%d gbcli calls=%d keys=%s"
          % ("won" if won else "NOT WON", p.frame, st["keys"], st["oil"], OIL_PIXELS,
             100 * st["oil"] // OIL_PIXELS, p.low, len(moves), p.calls, os.path.relpath(opts.out, opts.dir)))
    return 0 if won else 1


if __name__ == "__main__":
    sys.exit(main())
