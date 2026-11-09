#!/usr/bin/env python3
"""Lanternfall's map tool: read a floor out of a gbcli session and check it.

  maptool.py dump  --session S [--out FILE]   the map at the session's head, as text
                   [--raw FILE]               (and its 3072 bytes, exactly as in WRAM)
  maptool.py check FILE...                    every floor cell reachable from the start?
  maptool.py seeds F [F ...]                  for each title frame F: press START on
                                              it, dump the floor, check it
  maptool.py view  --session S                does the background show the map under
                                              the camera? (every cell the screen shows)
  maptool.py play --session S --mode win|die  a closed-loop bot that plays the run out:
                                              fights what's adjacent, drinks when low,
                                              takes the blade, heads down (win), or
                                              walks into enemies and waits (die);
                                              --keys-out FILE records it for a replay
  maptool.py path FILE                        the steps from the player to the stairs
  maptool.py descend --session S --floors N   a closed-loop bot: read the floor, walk
                                              the shortest path to the stairs, repeat;
                                              --keys-out FILE records every press
                                              (frames since boot) for a replay

gbcli has no block read, so `dump` reads the map a byte at a time in one
`gbcli read` call (3072 expressions). gbcli comes from PATH, or $GBCLI.
Run from examples/rogue. Python 3 standard library only.
"""

import argparse
import json
import os
import subprocess
import sys
from collections import deque

MAP_W, MAP_H = 64, 48
ROCK, FLOOR, WALL, STAIRS, POTION, BLADE, EMBER = range(7)
GLYPH = {ROCK: " ", FLOOR: ".", WALL: "#", STAIRS: ">", POTION: "!", BLADE: "/", EMBER: "*"}
WALKABLE = {FLOOR, STAIRS, POTION, BLADE, EMBER}
GOALS = {STAIRS, EMBER}
MAX_ENEMIES = 8
STATE_PLAY, STATE_WON, STATE_DEAD = 0, 1, 2
ROM = "build/rogue.gb"
SCALARS = ["u16[wSeed]", "[wDepth]", "[wPlayerX]", "[wPlayerY]",
           "[wStairsX]", "[wStairsY]", "[wRoomCount]"]


def gbcli(*args):
    exe = os.environ.get("GBCLI", "gbcli")
    out = subprocess.run([exe, *args, "--json"], capture_output=True, text=True)
    if out.returncode != 0:
        sys.exit(f"gbcli {' '.join(args[:3])}… failed ({out.returncode}): {out.stderr or out.stdout}")
    return json.loads(out.stdout)


def read_floor(session):
    """The map and its scalars at the session's head."""
    exprs = SCALARS + [f"[wMap+{i}]" for i in range(MAP_W * MAP_H)]
    reads = gbcli("read", *exprs, "--session", session)["result"]["reads"]
    vals = [r["value"] for r in reads]
    head = dict(zip(["seed", "depth", "px", "py", "sx", "sy", "rooms"], vals[:len(SCALARS)]))
    cells = vals[len(SCALARS):]
    return head, [cells[y * MAP_W:(y + 1) * MAP_W] for y in range(MAP_H)]


def to_text(head, grid):
    lines = ["lanternfall-map 1",
             " ".join(f"{k}={v}" for k, v in head.items())]
    for y, row in enumerate(grid):
        chars = [GLYPH.get(c, "?") for c in row]
        if y == head["py"]:
            chars[head["px"]] = "@"
        lines.append("".join(chars).rstrip())
    return "\n".join(lines) + "\n"


def from_text(text):
    lines = text.splitlines()
    if lines[0] != "lanternfall-map 1":
        sys.exit("not a lanternfall map")
    head = {k: int(v) for k, v in (kv.split("=") for kv in lines[1].split())}
    inv = {g: c for c, g in GLYPH.items()}
    inv["@"] = FLOOR
    grid = []
    for line in lines[2:2 + MAP_H]:
        line = line.ljust(MAP_W)
        grid.append([inv[ch] for ch in line])
    return head, grid


def bfs(grid, start):
    prev = {start: None}
    q = deque([start])
    while q:
        x, y = q.popleft()
        for dx, dy, key in ((1, 0, "RIGHT"), (-1, 0, "LEFT"), (0, -1, "UP"), (0, 1, "DOWN")):
            n = (x + dx, y + dy)
            if 0 <= n[0] < MAP_W and 0 <= n[1] < MAP_H and n not in prev \
                    and grid[n[1]][n[0]] in WALKABLE:
                prev[n] = ((x, y), key)
                q.append(n)
    return prev


def check(head, grid):
    """(ok, message): every walkable cell reachable from the start."""
    walk = {(x, y) for y in range(MAP_H) for x in range(MAP_W) if grid[y][x] in WALKABLE}
    start = (head["px"], head["py"])
    stairs = (head["sx"], head["sy"])
    problems = []
    if start not in walk:
        problems.append("start is not floor")
    if grid[stairs[1]][stairs[0]] not in GOALS:
        problems.append("no stairs (or Ember) at wStairsX/Y")
    reached = set(bfs(grid, start)) & walk
    items = sum(1 for (x, y) in walk if grid[y][x] in (POTION, BLADE))
    lost = walk - reached
    if lost:
        problems.append(f"{len(lost)} unreachable, first {sorted(lost)[:5]}")
    border = [(x, y) for (x, y) in walk if x in (0, MAP_W - 1) or y in (0, MAP_H - 1)]
    if border:
        problems.append(f"floor on the map's edge at {border[:3]}")
    msg = (f"seed={head['seed']} depth={head['depth']} rooms={head['rooms']} "
           f"floor={len(walk)} reachable={len(reached)} items={items} stairs_reachable={stairs in reached}")
    return (not problems, msg + ("" if not problems else " problems=" + "; ".join(problems)))


def path(head, grid):
    prev = bfs(grid, (head["px"], head["py"]))
    node = (head["sx"], head["sy"])
    if node not in prev:
        sys.exit("stairs unreachable")
    keys = []
    while prev[node] is not None:
        node, key = prev[node]
        keys.append(key)
    return keys[::-1]


STEP_FRAMES = 4      # a press, a release, and time for the view to catch up
FLOOR_FRAMES = 40    # time for the next floor to generate and stream in


def head_frame(session):
    return gbcli("read", "frame", "--session", session)["result"]["reads"][0]["value"]


def descend(a):
    """Walk to the stairs floor after floor; prints one row per floor."""
    log = []
    bad = 0
    for _ in range(a.floors):
        head, grid = read_floor(a.session)
        ok, msg = check(head, grid)
        bad += not ok
        print(("ok   " if ok else "FAIL ") + msg)
        if a.save:
            os.makedirs(a.save, exist_ok=True)
            open(os.path.join(a.save, f"seed{head['seed']}-depth{head['depth']}.map"), "w").write(to_text(head, grid))
        steps = path(head, grid)
        # Walk in two halves; between them, with the view settled, check that
        # the background shows the map under the camera.
        half = len(steps) // 2
        for part, chunk in enumerate((steps[:half], steps[half:])):
            base = head_frame(a.session)
            tokens = []
            for i, key in enumerate(chunk):
                f = 1 + i * STEP_FRAMES
                tokens += [f"{key}@{f}", f"-{key}@{f + 2}"]
                log += [f"{key}@{base + f}", f"-{key}@{base + f + 2}"]
            frames = 1 + len(chunk) * STEP_FRAMES + (FLOOR_FRAMES if part else 2)
            gbcli("run", "--session", a.session, "--frames", str(frames), "--keys", " ".join(tokens))
            if not part:
                vok, vmsg = view_check(a.session)
                bad += not vok
                print(("     view ok   " if vok else "     view FAIL ") + vmsg)
        depth = gbcli("read", "[wDepth]", "--session", a.session)["result"]["reads"][0]["value"]
        print(f"     walked {len(steps)} steps; now depth={depth}")
        if depth != head["depth"] + 1:
            sys.exit("the walk did not reach the next floor")
    if a.keys_out:
        open(a.keys_out, "a").write("\n".join(log) + "\n")
    sys.exit(1 if bad else 0)


def view_check(session):
    """(ok, message): the 32x32 background ring against wMap at the camera."""
    head, grid = read_floor(session)
    cam = gbcli("read", "[wCamX]", "[wCamY]", "[rSCX]", "[rSCY]", "--session", session)["result"]["reads"]
    cx, cy, scx, scy = (r["value"] for r in cam)
    rows = gbcli("view", "map", "--session", session)["result"]["view"]["data"]["ids"]
    ring = [[int(v, 16) for v in row.split()] for row in rows]
    bad = []
    for y in range(cy, min(cy + 18, MAP_H)):
        for x in range(cx, min(cx + 20, MAP_W)):
            if ring[y % 32][x % 32] != grid[y][x]:
                bad.append((x, y))
    scroll_ok = scx == (cx * 8) % 256 and scy == (cy * 8) % 256
    msg = f"depth={head['depth']} cam={cx},{cy} scroll={scx},{scy} cells=20x18 mismatched={len(bad)}"
    if bad:
        msg += f" first={bad[:4]}"
    if not scroll_ok:
        msg += " scroll does not match the camera"
    return (not bad and scroll_ok, msg)


# --- the closed-loop player -------------------------------------------------

DIRS = {"RIGHT": (1, 0), "LEFT": (-1, 0), "UP": (0, -1), "DOWN": (0, 1)}
STATE_EXPRS = (["frame", "[wPlayerX]", "[wPlayerY]", "[wHP]", "[wAtk]", "[wGameState]", "[wDepth]"]
               + [f"[wEnemyX+{i}]" for i in range(MAX_ENEMIES)]
               + [f"[wEnemyY+{i}]" for i in range(MAX_ENEMIES)]
               + [f"[wEnemyHP+{i}]" for i in range(MAX_ENEMIES)])


def parse_state(reads):
    v = [r["value"] for r in reads]
    st = dict(zip(["frame", "px", "py", "hp", "atk", "state", "depth"], v[:7]))
    xs, ys, hps = v[7:15], v[15:23], v[23:31]
    st["enemies"] = {(xs[i], ys[i]) for i in range(MAX_ENEMIES) if hps[i]}
    return st


def bfs_to(grid, start, targets, blocked):
    """The first key toward the nearest target, or None."""
    prev = {start: None}
    q = deque([start])
    while q:
        cur = q.popleft()
        if cur in targets:
            while prev[cur] and prev[cur][0] != start:
                cur = prev[cur][0]
            return prev[cur][1] if prev[cur] else None
        for key, (dx, dy) in DIRS.items():
            n = (cur[0] + dx, cur[1] + dy)
            if n not in prev and grid[n[1]][n[0]] in WALKABLE and n not in blocked:
                prev[n] = (cur, key)
                q.append(n)
    return None


def choose(mode, st, grid):
    """The key for this turn: a direction, or B to wait."""
    me = (st["px"], st["py"])
    adjacent = [k for k, (dx, dy) in DIRS.items() if (me[0] + dx, me[1] + dy) in st["enemies"]]
    cells = lambda kinds: {(x, y) for y in range(MAP_H) for x in range(MAP_W) if grid[y][x] in kinds}
    if mode == "die":
        if adjacent:
            return "B"                      # stand and take it
        return (bfs_to(grid, me, st["enemies"], set()) or bfs_to(grid, me, cells(GOALS), set()) or "B")
    if adjacent:
        return adjacent[0]                  # fight what's next to you
    targets = None
    if st["hp"] <= 14 and cells({POTION}):
        targets = cells({POTION})
    elif st["atk"] < 4 and cells({BLADE}):
        targets = cells({BLADE})
    targets = targets or cells(GOALS)
    # Around enemies if possible, else through them.
    return bfs_to(grid, me, targets, st["enemies"]) or bfs_to(grid, me, targets, set()) or "B"


def play(a):
    """Plays from the session's head until the run is won or lost."""
    log = []
    head, grid = read_floor(a.session)
    turns = 0
    views = {True: 0, False: 0}
    st = parse_state(gbcli("read", *STATE_EXPRS, "--session", a.session)["result"]["reads"])
    while st["state"] == STATE_PLAY and turns < a.max_turns:
        key = choose(a.mode, st, grid)
        base = st["frame"]
        log += [f"{key}@{base + 1}", f"-{key}@{base + 3}"]
        res = gbcli("run", "--session", a.session, "--frames", str(STEP_FRAMES),
                    "--keys", f"{key}@1 -{key}@3", *sum((["--read", e] for e in STATE_EXPRS), []))
        new = parse_state(res["result"]["reads"])
        turns += 1
        if new["depth"] != st["depth"]:
            res = gbcli("run", "--session", a.session, "--frames", str(FLOOR_FRAMES),
                        *sum((["--read", e] for e in STATE_EXPRS), []))
            new = parse_state(res["result"]["reads"])
            head, grid = read_floor(a.session)
            print(f"turn {turns}: down to depth {new['depth']} hp={new['hp']} atk={new['atk']}")
        elif key in DIRS:
            # Taking an item turns its cell to floor.
            cell = grid[new["py"]][new["px"]]
            if cell in (POTION, BLADE):
                grid[new["py"]][new["px"]] = FLOOR
                print(f"turn {turns}: took {'potion' if cell == POTION else 'blade'} hp={new['hp']} atk={new['atk']}")
        st = new
        if a.check_view and turns % a.check_view == 0 and st["state"] == STATE_PLAY:
            vok, vmsg = view_check(a.session)
            views[vok] += 1
            if not vok:
                print(f"turn {turns}: view FAIL {vmsg}")
    if a.check_view:
        print(f"view checks: {views[True]} ok, {views[False]} failed")
    outcome = {STATE_WON: "won", STATE_DEAD: "dead", STATE_PLAY: "still playing"}[st["state"]]
    print(f"{outcome} after {turns} turns at depth {st['depth']} hp={st['hp']} frame={st['frame']}")
    if a.keys_out:
        open(a.keys_out, "a").write("\n".join(log) + "\n")
    want = {"win": STATE_WON, "die": STATE_DEAD}[a.mode]
    sys.exit(0 if st["state"] == want and not views[False] else 1)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    d = sub.add_parser("dump")
    d.add_argument("--session", required=True)
    d.add_argument("--out")
    d.add_argument("--raw")
    c = sub.add_parser("check")
    c.add_argument("files", nargs="+")
    s = sub.add_parser("seeds")
    s.add_argument("frames", nargs="+", type=int)
    s.add_argument("--session", default="mapcheck")
    s.add_argument("--save", help="folder to save each map in")
    pl = sub.add_parser("play")
    pl.add_argument("--session", required=True)
    pl.add_argument("--mode", choices=["win", "die"], default="win")
    pl.add_argument("--max-turns", type=int, default=3000)
    pl.add_argument("--keys-out")
    pl.add_argument("--check-view", type=int, metavar="N",
                    help="every N turns, check the background against the map (maptool.py view)")
    v = sub.add_parser("view")
    v.add_argument("--session", required=True)
    p = sub.add_parser("path")
    p.add_argument("file")
    w = sub.add_parser("descend")
    w.add_argument("--session", required=True)
    w.add_argument("--floors", type=int, default=1)
    w.add_argument("--keys-out")
    w.add_argument("--save", help="folder to save each floor's map in")
    a = ap.parse_args()

    if a.cmd == "dump":
        head, grid = read_floor(a.session)
        text = to_text(head, grid)
        if a.raw:
            open(a.raw, "wb").write(bytes(c for row in grid for c in row))
        if a.out:
            open(a.out, "w").write(text)
        else:
            sys.stdout.write(text)
    elif a.cmd == "check":
        bad = 0
        for f in a.files:
            ok, msg = check(*from_text(open(f).read()))
            print(("ok   " if ok else "FAIL ") + f + " " + msg)
            bad += not ok
        sys.exit(1 if bad else 0)
    elif a.cmd == "seeds":
        bad = 0
        for f in a.frames:
            gbcli("run", ROM, "--session", a.session, "--from", "boot",
                  "--frames", str(f + 60), "--keys", f"START@{f} -START@{f + 2}")
            head, grid = read_floor(a.session)
            ok, msg = check(head, grid)
            print(("ok   " if ok else "FAIL ") + f"frame={f} " + msg)
            if a.save:
                os.makedirs(a.save, exist_ok=True)
                open(os.path.join(a.save, f"seed{head['seed']}.map"), "w").write(to_text(head, grid))
            bad += not ok
        sys.exit(1 if bad else 0)
    elif a.cmd == "play":
        play(a)
    elif a.cmd == "view":
        ok, msg = view_check(a.session)
        print(("ok   " if ok else "FAIL ") + msg)
        sys.exit(0 if ok else 1)
    elif a.cmd == "descend":
        descend(a)
    elif a.cmd == "path":
        print(" ".join(path(*from_text(open(a.file).read()))))


if __name__ == "__main__":
    main()
