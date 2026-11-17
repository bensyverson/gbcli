"""Screenshots for a human to review the look, in build/review/.

    python3 tools/review_shots.py [--gbcli PATH] [-v]

  title.png         the title screen (DMG) after a round: the best, 12
  title-gbc.png     the same on a GBC (build/gbc/teddy-and-toys.gbc, --model gbc)
  round-dmg.png     mid-round on a DMG, Elle holding a half
  round-gbc.png     the same moment on a GBC: the toys in color on the belts,
                    in her hands and in the key
  held-gbc.png      Elle holding each finished toy on a GBC, key order
  pause-dmg.png     the pause menu, START pressed mid-round (DMG)
  pause-gbc.png     the same on a GBC, the cursor on Quit
  step-dmg.png      Elle stepped to her left limit, then her right, holding
                    two halves (DMG)
  step-gbc.png      the same on a GBC, facing away
  red-gbc.png       the last 10 seconds on a GBC: the clock face red
  turn.png          Elle turning up to the Finished belt, then back down:
                    one crop around her a frame or two apart, with the frame
                    after Up or Down under each
  away-dmg.png      Elle facing away holding a toy, then two halves (drawn
                    over them), then both facing the parts belt (DMG)
  away-gbc.png      the same on a GBC
  key.png           the key column (the window)
  results.png       the results screen after a round, with "New best!"
  toys.png          every toy from art/toys/: half A, half B, finished, on
                    the belt (its gray and lines) and on white, four times the size
Every shot except toys.png is a gbcli shot of the built ROMs; pokes set up
the moment (what Elle holds, the score), never the pixels.
"""

import gbcli as g
import gen_toys
import pixels
import png
import tuning

T = tuning.load()
TURN = T["TURN_FRAMES"]
STEP = T["ELLE_STEP_RANGE"] // T["ELLE_STEP_SPEED"]   # frames to walk to a limit
OUT = g.PROJECT / "build" / "review"
DMG_RGB = [(255, 255, 255), (170, 170, 170), (85, 85, 85), (0, 0, 0)]
ELLE_CROP = (36, 36, 64, 76)        # x, y, w, h around Elle and her hands
HOLD = ["wHands=1,0", "wHeldCount=1", "wWantedPiece=2"]   # a teddy head
HALVES = ["wHands=9,10", "wHeldCount=2", "wWantedPiece=0"]  # gag glasses and mustache


def shot(cli, session, at, frames, name, keys="", pokes=(), crop=None, extra=(), scale=3):
    path = OUT / name
    args = ["run", "--session", session, "--frames", str(frames), "--shot", str(path), "--scale", str(scale)]
    if at:
        args += ["--at", at]
    if keys:
        args += ["--keys", keys]
    for p in pokes:
        args += ["--poke", p]
    if crop:
        args += ["--crop", ",".join(map(str, crop))]
    cli.run(*args, *extra)
    return path


def start(cli, session, rom, model=()):
    cli.text("sessions", "rm", session)
    cli.run("run", rom, "--session", session, "--from", "boot", "--frames", "60", *model)
    cli.run("run", "--session", session, "--frames", "600", "--keys", "START@1 -START@2",
            "--checkpoint", "mid")


def scaled(path, scale):
    rows = png.read(path)
    return [[px for px in row for _ in range(scale)] for row in rows for _ in range(scale)]


def turn_strip(cli):
    """Crops around Elle through a turn up, then a turn down.

    Measured with --read: a press at @1 reaches the game in the run's
    second frame, which starts the turn (wTurnTimer = TURN); the screen at
    the stop shows the objects of the frame before. So with the press at
    @1, shots after k = 3 to TURN + 2 frames show the turn, and TURN + 3
    the new facing."""
    frames = [3, TURN // 2 + 2, TURN + 2, TURN + 3]
    images = [scaled(shot(cli, "review", "mid", 3, "turn-up-0.png", "", HOLD, ELLE_CROP, scale=1), 3)]
    for k in frames:
        p = shot(cli, "review", "mid", k, f"turn-up-{k}.png", "UP@1 -UP@2", HOLD, ELLE_CROP, scale=1)
        images.append(scaled(p, 3))
    cli.run("run", "--session", "review", "--at", "mid", "--frames", str(TURN + 4),
            "--keys", "UP@1 -UP@2", "--poke", HOLD[0], "--poke", HOLD[1], "--checkpoint", "back")
    for k in frames:
        p = shot(cli, "review", "back", k, f"turn-down-{k}.png", "DOWN@1 -DOWN@2", crop=ELLE_CROP, scale=1)
        images.append(scaled(p, 3))
    for p in OUT.glob("turn-*-*.png"):
        p.unlink()
    png.write(OUT / "turn.png", png.hstack(images, gap=6))


def away_strip(cli, session, name):
    """Crops around Elle facing away (the Finished belt), holding a toy, then
    two halves, then the same facing the parts belt, for comparison."""
    away = ["wFacing=1"]
    cases = [away + ["wToy=1"], away + HALVES, ["wToy=1"], HALVES]
    images = []
    for i, pokes in enumerate(cases):
        p = shot(cli, session, "mid", 4, f"{name}-{i}.png", pokes=pokes, crop=ELLE_CROP, scale=1)
        images.append(scaled(p, 3))
        p.unlink()
    png.write(OUT / f"{name}.png", png.hstack(images, gap=6))


def title_after_round(cli, session, name):
    """The title after a round that scored 12 (BCD $12), a new best."""
    shot(cli, session, "mid", 90, name, keys="START@60 -START@61", pokes=["wRoundLeft=3,0", "wScore=$12,0"])


def held_strip(cli, session, name):
    """Crops around Elle holding each finished toy, facing the parts belt."""
    images = []
    for toy in range(1, len(gen_toys.load()) + 1):
        p = shot(cli, session, "mid", 4, f"{name}-{toy}.png", pokes=[f"wToy={toy}"], crop=ELLE_CROP, scale=1)
        images.append(scaled(p, 3))
        p.unlink()
    png.write(OUT / f"{name}.png", png.hstack(images, gap=6))


def step_strip(cli, session, name, pokes=()):
    """Elle walked (Left or Right held) to each end of her range, holding two halves."""
    images = []
    for i, key in enumerate(("LEFT", "RIGHT")):
        p = shot(cli, session, "mid", STEP + 10, f"{name}-{i}.png", keys=f"{key}@1 -{key}@{STEP + 8}",
                 pokes=HALVES + list(pokes), scale=1)
        images.append(scaled(p, 2))
        p.unlink()
    png.write(OUT / f"{name}.png", png.hstack(images, gap=6))


def toys_sheet():
    toys = gen_toys.load()
    rows = []
    for toy in toys:
        cells = []
        for clear in (2, 0):
            img = pixels.read(gen_toys.ART / toy["art"], clear=-1)
            # On the belt: dark gray with its black line every 8 px (the
            # game's ToyBeltTiles); on white: as Elle holds it, on the floor.
            img = [[s if s >= 0 else (3 if clear == 2 and x % 8 == 0 else clear) for x, s in enumerate(row)]
                   for row in img]
            # A one-piece toy (the slinky) shows its one piece as both halves.
            order = (0, 0, 2) if toy["same"] else (0, 1, 2)
            frames = [[row[f * 16:f * 16 + 16] for row in img] for f in order]
            cells += [[[DMG_RGB[s] for s in row] for row in fr] for fr in frames]
        rows.append(png.hstack(cells, gap=2))
    sheet = png.vstack(rows, gap=2)
    png.write(OUT / "toys.png", sheet, scale=4)


def require_fresh_roms():
    """Stops if a ROM is older than any source: this tool doesn't build, and
    shots of a stale ROM look like current art but aren't."""
    sources = [p for d in ("src", "include", "art", "assets", "data")
               for p in (g.PROJECT / d).rglob("*") if p.is_file()]
    newest = max(p.stat().st_mtime for p in sources)
    for rom in (g.ROM, g.ROM_GBC):
        path = g.PROJECT / rom
        if not path.exists() or path.stat().st_mtime < newest:
            raise SystemExit(f"{rom} is older than its sources: run python3 tools/make.py first")


def main():
    args = g.arg_parser(__doc__.splitlines()[0]).parse_args()
    require_fresh_roms()
    cli = g.Gbcli(args.gbcli, args.verbose)
    OUT.mkdir(parents=True, exist_ok=True)
    cli.text("sessions", "rm", "review")
    cli.run("run", g.ROM, "--session", "review", "--from", "boot", "--frames", "60")
    cli.run("run", "--session", "review", "--frames", "600", "--keys", "START@1 -START@2",
            "--checkpoint", "mid")
    title_after_round(cli, "review", "title.png")
    shot(cli, "review", "mid", 2, "round-dmg.png", pokes=HOLD)
    shot(cli, "review", "mid", 12, "pause-dmg.png", keys="START@1 -START@2", pokes=HOLD)
    step_strip(cli, "review", "step-dmg")
    shot(cli, "review", "mid", 1, "key.png", crop=(130, 0, 30, 144), scale=1)
    png.write(OUT / "key.png", png.read(OUT / "key.png"), scale=3)
    turn_strip(cli)
    away_strip(cli, "review", "away-dmg")
    shot(cli, "review", "mid", 30, "results.png", pokes=["wRoundLeft=3,0", "wScore=$12,0"])

    start(cli, "review-gbc", g.ROM_GBC, ["--model", "gbc"])
    shot(cli, "review-gbc", "mid", 2, "round-gbc.png", pokes=HOLD)
    held_strip(cli, "review-gbc", "held-gbc")
    shot(cli, "review-gbc", "mid", 16, "pause-gbc.png", keys="START@1 -START@2 DOWN@8 -DOWN@9", pokes=HOLD)
    step_strip(cli, "review-gbc", "step-gbc", ["wFacing=1"])
    away_strip(cli, "review-gbc", "away-gbc")
    title_after_round(cli, "review-gbc", "title-gbc.png")
    # "mid" is 10 s into the round; run on to 3 s into the red zone.
    red_at = (T["ROUND_SECONDS"] - T["CLOCK_RED_SECONDS"] + 3 - 10) * 60
    shot(cli, "review-gbc", "mid", red_at, "red-gbc.png")
    toys_sheet()
    for p in sorted(OUT.glob("*.png")):
        print(p)
    print(f"{cli.calls} gbcli calls")


if __name__ == "__main__":
    main()
