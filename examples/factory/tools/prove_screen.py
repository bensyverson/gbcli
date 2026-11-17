"""Proves the screen's technique with gbcli, and prints pass or fail per claim.

    python3 tools/prove_screen.py [--gbcli PATH] [-v]

Claims, each checked on the built ROM (build/factory.gb) in session `prove`:
  bands     every line of each band is drawn with its band's scroll (the
            raster's drawn SCX): HUD 0, Finished belt, floor 0, parts belt
  hblank    every SCX write the game makes lands in HBlank or VBlank
  opposite  three seconds later the belts have moved the opposite ways,
            each by its speed from include/tuning.inc
  key       the key column's pixels are the same three seconds later,
            while the belts' pixels are not
  clock     the clock hand has advanced three steps' worth
  budget    a whole round (and a bit) from boot has no lag frame
Screenshots land in build/prove/.
"""

import sys

import gbcli as g
import png
import tuning

SESSION = "prove"
START_KEYS = "START@60 -START@62"  # leaves the title screen
LATER = 180                       # frames between the two samples: 3 s

BANDS = [                         # first line, last line, which scroll
    (0, 55, "hud"), (56, 87, "finished"), (88, 111, "floor"), (112, 143, "parts"),
]


def raster(cli, at):
    reply = cli.run("run", "--session", SESSION, "--at", at, "--frames", "2", "--raster")
    return reply["result"]["raster"]


def band_scroll(drawn):
    """{band: scx} if every line of each band shares one SCX, else None."""
    out = {}
    for first, last, name in BANDS:
        values = {drawn["scx"][line - drawn["first_line"]] for line in range(first, last + 1)}
        if len(values) != 1:
            return None, f"{name} lines {first}-{last} drawn with SCX {sorted(values)}"
        out[name] = values.pop()
    return out, None


def read(cli, at, expr):
    reply = cli.run("run", "--session", SESSION, "--at", at, "--frames", "0", "--read", expr)
    return reply["result"]["reads"][0]["value"]


def main():
    args = g.arg_parser(__doc__.splitlines()[0]).parse_args()
    cli = g.Gbcli(args.gbcli, args.verbose)
    t = tuning.load()
    results = []

    def claim(name, ok, detail):
        results.append(ok)
        print(f"{'PASS' if ok else 'FAIL'} {name}: {detail}")

    (g.PROJECT / "build" / "prove").mkdir(parents=True, exist_ok=True)
    # From boot, START on the title at frame 60, then into the round.
    cli.run("run", g.ROM, "--session", SESSION, "--from", "boot", "--frames", "300",
            "--keys", START_KEYS, "--checkpoint", "early")
    cli.run("run", "--session", SESSION, "--at", "early", "--frames", str(LATER), "--checkpoint", "later")

    # bands and hblank
    r1 = raster(cli, "early")
    r2 = raster(cli, "later")
    s1, why1 = band_scroll(r1["drawn"][-1])
    s2, why2 = band_scroll(r2["drawn"][-1])
    ok = s1 is not None and s2 is not None and s1["hud"] == s1["floor"] == 0
    claim("bands", ok, why1 or why2 or f"HUD 0, Finished {s1['finished']}, floor 0, parts {s1['parts']} (one SCX per band, every line)")

    bad = [
        f"{w['reg']} at line {row['line']} in {w['mode']}"
        for r in (r1, r2) for row in r["rows"] for w in row["writes"]
        if w["reg"] in ("rSCX", "rSCY", "rWX", "rWY") and w["mode"] not in ("hblank", "vblank")
    ]
    claim("hblank", not bad, "; ".join(bad) or "every scroll write landed in HBlank or VBlank")

    # opposite
    if s1 and s2:
        def moved(name, speed, direction):
            delta = (s2[name] - s1[name]) % 256
            want = round(LATER * speed / 256) % 256
            if direction == t["BELT_RIGHT"]:
                want = (-want) % 256
            return delta, want, abs((delta - want + 128) % 256 - 128) <= 1
        dp, wp, okp = moved("parts", t["BELT_PARTS_SPEED"], t["BELT_PARTS_DIR"])
        df, wf, okf = moved("finished", t["BELT_FINISHED_SPEED"], t["BELT_FINISHED_DIR"])
        claim("opposite", okp and okf and dp != df,
              f"parts SCX {s1['parts']}->{s2['parts']} (moved {dp}, want {wp}), "
              f"Finished SCX {s1['finished']}->{s2['finished']} (moved {df}, want {wf}) in {LATER} frames")

    # key column and belts, by screenshot
    shots = {}
    for at in ("early", "later"):
        for name, crop in (("key", "136,0,24,144"), ("belts", "0,56,136,88"), ("full", None)):
            path = f"build/prove/{name}-{at}.png"
            extra = ["--crop", crop] if crop else []
            cli.run("run", "--session", SESSION, "--at", at, "--frames", "0", "--shot", path, *extra)
            shots[name, at] = png.read(g.PROJECT / path)
    same_key = shots["key", "early"] == shots["key", "later"]
    same_belts = shots["belts", "early"] == shots["belts", "later"]
    claim("key", same_key and not same_belts,
          f"key column x 136-159 {'identical' if same_key else 'CHANGED'}, belts "
          f"{'changed' if not same_belts else 'IDENTICAL'} (build/prove/key-*.png, belts-*.png, full-*.png)")

    # clock
    c1 = read(cli, "early", "[wClockStep]")
    c2 = read(cli, "later", "[wClockStep]")
    per_step = t["ROUND_SECONDS"] * 60 // t["CLOCK_STEPS"]
    want = LATER // per_step
    claim("clock", (c2 - c1) % t["CLOCK_STEPS"] == want, f"hand step {c1} -> {c2} (want +{want})")

    # budget
    # The title, then the whole round up to its last frames (the switch to
    # the results screen is a screen change, not play). tools/bot.py's keys
    # replay measures a round with play in it.
    frames = 60 + t["ROUND_SECONDS"] * 60
    reply = cli.run("run", "--session", SESSION, "--from", "boot", "--frames", str(frames),
                    "--keys", START_KEYS, "--frame-budget", "--read", "u16[wRoundLeft]", "--wall-guard", "300")
    budget = reply["result"]["frame_budget"]
    left = reply["result"]["reads"][0]["value"]
    claim("budget", budget["lag"] == 0 and 0 < left < 60,
          f"{frames} frames from boot (round frames left at the end: {left}), lag frames {budget['lag']}, "
          f"LCD-off frames {budget['lcd_off']}, busiest frame {budget['active_t']['max']} of 70224 T active")

    print(f"{sum(results)}/{len(results)} claims pass; gbcli calls {cli.calls}")
    sys.exit(0 if all(results) else 1)


if __name__ == "__main__":
    main()
