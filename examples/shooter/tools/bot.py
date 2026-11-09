#!/usr/bin/env python3
"""An autopilot for Vesper Swarm that records what it presses.

It drives a fresh gbcli session from boot a few frames at a time: reads the
ship, the enemies, the boss and every bullet from RAM, picks the move whose
next 20 frames come closest to nothing while lining up a shot, holds A
throughout, and writes every key change, at its frame since boot, to a
keys file. Replaying that file with `gbcli run --keys-file` plays the same
game, since the emulator is deterministic:

    python3 -I tools/bot.py --out keys/win.keys
    gbcli run build/shooter.gb --session win --frames 6000 \\
        --keys-file keys/win.keys --until '[wState] == 2'

Run from the project folder after `gbcli build`. Exits 1 unless the run
reached the stage-clear screen.
"""

import argparse
import json
import subprocess
import sys

STEP = 4            # frames per decision
HORIZON = 20        # frames each move is judged over
HOLD = 6            # frames a move is assumed held before the ship stops
TITLE_FRAMES = 40   # boot to the title screen
STATE_PLAY, STATE_WIN, STATE_OVER = 1, 2, 3
KIND_DART, KIND_WAVE, KIND_POD, KIND_BOOM = 1, 2, 3, 4
BOSS_ENTER, BOSS_FIGHT = 1, 2
ENEMIES, BULLETS = 8, 31
SHIP_SPEED = 2
MOVES = {
    "": (0, 0), "UP": (0, -1), "DOWN": (0, 1), "LEFT": (-1, 0), "RIGHT": (1, 0),
    "UP LEFT": (-1, -1), "UP RIGHT": (1, -1), "DOWN LEFT": (-1, 1), "DOWN RIGHT": (1, 1),
}
SINE = [round(20 * __import__("math").sin(i * 2 * 3.141592653589793 / 64)) for i in range(64)]

READS = ["[wState]", "[wPlayerX]", "[wPlayerY]", "[wInvuln]", "[wLives]",
         "[wBossState]", "[wBossX]", "[wBossY]"]
for i in range(ENEMIES):
    base = f"wEnemies+{16 * i}"
    READS += [f"[{base}]", f"[{base}+1]", f"[{base}+2]", f"[{base}+5]", f"[{base}+6]"]
for i in range(BULLETS):
    base = f"wBullets+{8 * i}"
    READS += [f"u16[{base}]", f"u16[{base}+2]", f"u16[{base}+4]", f"u16[{base}+6]"]


def signed(v):
    return (v - 65536 if v >= 32768 else v) / 256


class Game:
    def __init__(self, exe, session, rom):
        self.exe, self.session, self.rom = exe, session, rom
        self.frame = 0
        self.held = set()
        self.log = []

    def run(self, frames, keys=""):
        args = [self.exe, "run", "--session", self.session, "--frames", str(frames),
                "--allow-uninit", "--json"]
        if self.frame == 0:
            args[2:2] = [self.rom]
        if keys:
            args += ["--keys", keys]
        for r in READS:
            args += ["--read", r]
        out = subprocess.run(args, capture_output=True, text=True)
        if out.returncode != 0:
            sys.exit(f"gbcli failed ({out.returncode}): {out.stderr}")
        res = json.loads(out.stdout.splitlines()[-1])["result"]
        self.frame += frames
        return [r["value"] for r in res["reads"]]

    def press(self, want):
        """Key tokens that turn the held set into `want`, logged at this frame."""
        tokens = [f"-{k}@0" for k in sorted(self.held - want)] + [f"{k}@0" for k in sorted(want - self.held)]
        for t in tokens:
            name, _ = t.split("@")
            self.log.append(f"{name}@{self.frame}")
        self.held = set(want)
        return " ".join(tokens)


def parse(v):
    s = dict(zip(["state", "px", "py", "invuln", "lives", "boss", "bx", "by"], v[:8]))
    i = 8
    s["enemies"] = []
    for _ in range(ENEMIES):
        kind, x, y, t, base = v[i:i + 5]
        i += 5
        if kind and kind != KIND_BOOM:
            s["enemies"].append((kind, x, y, t, base))
    s["bullets"] = []
    for _ in range(BULLETS):
        x, y, dx, dy = v[i:i + 4]
        i += 4
        if x >> 8:
            s["bullets"].append((x / 256 + 4, y / 256 + 3, signed(dx), signed(dy)))
    return s


def enemy_at(kind, x, y, t, base, f):
    """The enemy's box (x, y, w, h) f frames ahead, as its kind moves."""
    t2 = t + f
    if kind == KIND_DART:
        x -= 1.5 * f
    elif kind == KIND_WAVE:
        x -= f
        y = base + SINE[(t2 * 4) & 63]
    elif kind == KIND_POD:
        if t2 < 48:
            x -= f
        elif t2 >= 150:
            x -= 2 * (t2 - max(t, 150))
    if x > 200:                       # wrapped past the left edge
        x -= 256
    return x, y, 16, 8


def cost(s, move):
    mx, my = MOVES[move]
    px, py = s["px"], s["py"]
    c = 0.0
    for f in range(1, HORIZON + 1):
        if f <= HOLD:
            px = min(max(px + mx * SHIP_SPEED, 0), 144)
            py = min(max(py + my * SHIP_SPEED, 0), 128)
        hx, hy = px + 7, py + 4
        w = 1.0 / f
        for bx, by, dx, dy in s["bullets"]:
            ex, ey = abs(bx + dx * f - hx), abs(by + dy * f - hy)
            if ex < 5 and ey < 5:
                c += 1000 * w
            elif ex < 9 and ey < 9:
                c += 60 * w
        for e in s["enemies"]:
            x, y, ew, eh = enemy_at(*e, f)
            if x - 6 <= hx < x + ew + 6 and y - 6 <= hy < y + eh + 6:
                c += 1000 * w
        if s["boss"] in (BOSS_ENTER, BOSS_FIGHT):
            if s["bx"] - 8 <= hx < s["bx"] + 32 and s["by"] - 8 <= hy < s["by"] + 32:
                c += 1000 * w
    # Line up a shot: the boss, or the nearest enemy ahead.
    target = None
    if s["boss"] in (BOSS_ENTER, BOSS_FIGHT):
        target = s["by"] + 12
    else:
        ahead = [e for e in s["enemies"] if s["px"] + 16 < e[1] < 168]
        if ahead:
            target = min(ahead, key=lambda e: e[1])[2] + 4
    if target is not None:
        c += abs(py + 4 - target) * 0.8
    c += abs(px - 24) * 0.5 + abs(py - 64) * 0.05
    return c


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--out", default="keys/win.keys")
    ap.add_argument("--rom", default="build/shooter.gb")
    ap.add_argument("--session", default="bot")
    ap.add_argument("--max-frames", type=int, default=7000)
    ap.add_argument("--gbcli", default="gbcli")
    a = ap.parse_args()

    subprocess.run([a.gbcli, "sessions", "rm", a.session, "--with-checkpoints"], capture_output=True)
    g = Game(a.gbcli, a.session, a.rom)
    g.run(TITLE_FRAMES)
    g.run(2, g.press({"START"}))
    v = g.run(STEP, g.press({"A"}))
    while g.frame < a.max_frames:
        s = parse(v)
        if s["state"] != STATE_PLAY:
            break
        best = min(MOVES, key=lambda m: cost(s, m))
        v = g.run(STEP, g.press({"A", *best.split()} if best else {"A"}))
    s = parse(v)
    with open(a.out, "w") as f:
        f.write("# Vesper Swarm, a winning run: recorded by tools/bot.py, frames since boot\n")
        f.write("\n".join(g.log) + "\n")
    result = {STATE_WIN: "stage_clear", STATE_OVER: "game_over"}.get(s["state"], "unfinished")
    print(f"result={result} frame={g.frame} lives={s['lives']} key_events={len(g.log)} out={a.out}")
    sys.exit(0 if s["state"] == STATE_WIN else 1)


if __name__ == "__main__":
    main()
