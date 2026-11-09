#!/usr/bin/env python3
"""The host reference renderer: casts the view the ROM shows and compares.

With a session, it asks gbcli for the screen (`gbcli shot`), the pose the
view on screen was cast from (wViewX, wViewY, wViewA, wViewBob), the map
(wMap), the keys still in the maze (wKeyCell) and OAM (`gbcli view oam`).
It renders the same view with raymath.py and compares the view's 128x64
pixels one by one, except under the key objects on screen; and it checks
that those objects are exactly the ones raymath.key_objects places: frame,
position, tile and which are hidden behind nearer walls. Exit 0 when both
match, 1 otherwise.

  python3 tools/reference.py --gbcli PATH --session ray [--dir PROJECT]
  python3 tools/reference.py --png view.png --pose X,Y,A --map map.bin

Take the session's head at a frame stop (run --frames N): a stop mid-frame
shows the last complete frame, which may predate the pose in wViewX.
--out FILE writes the reference picture; --diff FILE marks mismatches in red.
Without a session (--png) the keys aren't checked.
"""

import argparse
import json
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import pngio  # noqa: E402
import raymath as rm  # noqa: E402

# Where the view sits on the screen (VIEW_MAP_X, VIEW_MAP_Y in tiles).
VIEW_X = 16
VIEW_Y = 16
# A pixel of each shade, 0-3 (SWATCH_MAP_X, SWATCH_MAP_Y in defs.inc).
SWATCH = [(16 * 8 + 4 + 8 * i, 16 * 8 + 4) for i in range(4)]
MAZE_KEYS = 3

HERE = os.path.dirname(os.path.abspath(__file__))
PROJECT = os.path.normpath(os.path.join(HERE, ".."))


def gbcli(args, opts):
    cmd = [opts.gbcli] + args + ["--session", opts.session, "--dir", opts.dir]
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode != 0:
        sys.stderr.write(r.stdout + r.stderr)
        raise SystemExit("gbcli failed: %s" % " ".join(cmd))
    return r.stdout


def read_values(exprs, opts):
    out = gbcli(["read"] + exprs, opts)
    values = []
    for line in out.splitlines():
        m = re.match(r"read expr=.* value=(-?\d+)", line)
        if m:
            values.append(int(m.group(1)))
    if len(values) != len(exprs):
        raise SystemExit("could not read %s from:\n%s" % (exprs, out))
    return values


def key_frames(project):
    """The key sheet's frame lists, from the build's build/images/images.asm:
    for each frame, its objects as (y, x, tile, attr)."""
    path = os.path.join(project, "build", "images", "images.asm")
    with open(path) as f:
        text = f.read()
    frames = []
    for m in re.finditer(r"^KeyFrame(\d+)::\n\tdb (\d+)\n((?:\tdb .*\n)*)", text, re.M):
        objs = [tuple(int(v) for v in line.split("db ")[1].split(", "))
                for line in m.group(3).splitlines()]
        frames.append(objs[:int(m.group(2))])
    if not frames:
        raise SystemExit("no KeyFrame lists in %s: build the project first" % path)
    return frames


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--gbcli", default="gbcli")
    ap.add_argument("--session")
    ap.add_argument("--dir", default=PROJECT)
    ap.add_argument("--png", help="a 1x screenshot instead of asking gbcli")
    ap.add_argument("--pose", help="X,Y,A instead of reading wViewX, wViewY, wViewA")
    ap.add_argument("--map", help="the 256-byte map instead of reading wMap")
    ap.add_argument("--current", action="store_true",
                    help="also fail if the view on screen isn't cast from the player's pose now")
    ap.add_argument("--out", help="write the reference view as a PNG")
    ap.add_argument("--diff", help="write the screen's view with mismatches in red")
    opts = ap.parse_args()

    work = os.path.join(opts.dir, "build", "reference")
    os.makedirs(work, exist_ok=True)

    if opts.png:
        shot = opts.png
    else:
        shot = os.path.join(work, "shot.png")
        gbcli(["shot", shot, "--scale", "1"], opts)
    bob = 0
    if opts.pose:
        px, py, pa = (int(v, 0) for v in opts.pose.split(","))
    else:
        px, py, pa, bob = read_values(["u16[wViewX]", "u16[wViewY]", "u16[wViewA]", "[wViewBob]"], opts)
        if opts.current:
            now = read_values(["u16[wPosX]", "u16[wPosY]", "u16[wAngle]"], opts)
            if now != [px, py, pa]:
                print("stale view x=$%04X y=$%04X a=%d, player at x=$%04X y=$%04X a=%d"
                      % (px, py, pa, now[0], now[1], now[2]))
                return 1
    if opts.map:
        with open(opts.map, "rb") as f:
            cells = list(f.read(256))
    else:
        mapbin = os.path.join(work, "map.bin")
        gbcli(["view", "memory", "wMap", "256", "--out", mapbin], opts)
        with open(mapbin, "rb") as f:
            cells = list(f.read(256))

    # The key objects: what OAM shows, and what raymath says it should.
    objects = []
    keys_note = ""
    if opts.session and not opts.png:
        oam = json.loads(gbcli(["view", "oam", "--json"], opts))["result"]["view"]["data"]["objects"]
        objects = sorted((o["y"], o["x"], o["tile"], o["flags"]) for o in oam if o["y"] != 0)
        key_cells = [None if v == 0xFF else v
                     for v in read_values(["[wKeyCell+%d]" % i for i in range(MAZE_KEYS)], opts)]
        depths = [d for _, _, d in rm.ray_columns(px, py, pa, cells)]
        want_objs = sorted(rm.key_objects(px, py, pa, bob, key_cells, key_frames(opts.dir), depths))
        if objects != want_objs:
            print("keys differ x=$%04X y=$%04X a=%d oam=%s reference=%s"
                  % (px, py, pa, objects, want_objs))
            return 1
        keys_note = " objects=%d" % len(objects)

    want = rm.render(px, py, pa, cells)
    width, height, rows = pngio.read_png(shot)
    if (width, height) != (160, 144):
        raise SystemExit("%s is %dx%d; take it with --scale 1" % (shot, width, height))
    # Pixels under a key object (8x16, OAM x - 8, y - 16) aren't the walls'.
    covered = set()
    for oy, ox, _, _ in objects:
        for y in range(oy - 16, oy):
            for x in range(ox - 8, ox):
                covered.add((x - VIEW_X, y - VIEW_Y))
    view = [row[VIEW_X:VIEW_X + rm.VIEW_W] for row in rows[VIEW_Y:VIEW_Y + rm.VIEW_H]]
    # The swatch shows shades 0-3 in the ROM's own palette, DMG or GBC.
    lum_to_shade = {}
    for shade, (sx, sy) in enumerate(SWATCH):
        lum_to_shade[sum(rows[sy][sx])] = shade
    if len(lum_to_shade) != 4:
        raise SystemExit("the swatch at %s shows %d colors, not 4" % (SWATCH, len(lum_to_shade)))
    unknown = {sum(view[y][x]) for y in range(rm.VIEW_H) for x in range(rm.VIEW_W)
               if (x, y) not in covered} - set(lum_to_shade)
    if unknown:
        raise SystemExit("the view has colors the swatch lacks: %s" % sorted(unknown))

    bad = 0
    first = None
    checked = 0
    for y in range(rm.VIEW_H):
        for x in range(rm.VIEW_W):
            if (x, y) in covered:
                continue
            checked += 1
            got = lum_to_shade[sum(view[y][x])]
            if got != want[y][x]:
                bad += 1
                if first is None:
                    first = (x, y, got, want[y][x])

    grays = [(255, 255, 255), (170, 170, 170), (85, 85, 85), (0, 0, 0)]
    if opts.out:
        pngio.write_png(opts.out, [[grays[v] for v in row] for row in want])
    if opts.diff:
        pngio.write_png(opts.diff, [
            [view[y][x] if (x, y) in covered or lum_to_shade[sum(view[y][x])] == want[y][x] else (255, 0, 0)
             for x in range(rm.VIEW_W)] for y in range(rm.VIEW_H)])

    pose = "x=$%04X y=$%04X a=%d" % (px, py, pa)
    if bad:
        x, y, got, exp = first
        print("mismatch %s pixels=%d first=(%d,%d) screen=%d reference=%d" % (pose, bad, x, y, got, exp))
        return 1
    print("match %s pixels=%d%s" % (pose, checked, keys_note))
    return 0


if __name__ == "__main__":
    sys.exit(main())
