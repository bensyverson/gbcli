"""The rules check: one scripted scenario per rule, pass or fail per rule.

    python3 tools/rules_check.py [--gbcli PATH] [-v] [--only TEXT]

Each scenario starts from the same moment of a round (checkpoint `play` in
session `rules`: START pressed, 30 frames in, facing the parts belt, empty
hands), pokes the state it needs (what Elle holds, a piece on the belt in
front of her), presses keys, runs a few frames and reads the result. Rules
come from the controls table in project/2026-11-16-teddy-and-toys-plan.md
and the numbers from include/tuning.inc (the chord window), so a tuning
change is checked as it stands. Needs a fresh build (tools/make.py).
"""

import sys
import time

import gbcli as g
import gen_toys
import tuning

SESSION = "rules"
T = tuning.load()
W = T["CHORD_FRAMES"]
TURN = T["TURN_FRAMES"]
TOYS = gen_toys.load()


def piece(toy_id, half):
    """A piece's number: toy * 2 + half + 1 (half 0 is half A)."""
    toy = next(i for i, t in enumerate(TOYS) if t["id"] == toy_id)
    return toy * 2 + half + 1


def partner(p):
    """The piece that completes piece p (itself, for a one-piece toy)."""
    toy = TOYS[(p - 1) // 2]
    return p if toy["same"] else ((p - 1) ^ 1) + 1


HEAD, BODY, STACK, ENGINE = piece("teddy", 0), piece("teddy", 1), piece("train", 0), piece("train", 1)
COIL = piece("slinky", 0)
TEDDY = 1                          # wToy holds toy + 1
SLINKY = COIL // 2 + 1

# The belts are poked so Elle stands at the middle of a known slot.
SLOT = 8
ELLE_CENTER_X = 68                 # include/layout.inc
PARTS_AT_SLOT = [f"wPartsX=0,{(SLOT * 16 + 8 - ELLE_CENTER_X) % 256}"]
FIN_SLOT = 4
FINISHED_AT_SLOT = [f"wFinishedX=0,{(FIN_SLOT * 16 + 8 - ELLE_CENTER_X) % 256}"]
SLOT_EXPR = f"[wPartsSlots+{SLOT}]"
# The slot that reaches Elle next, and how many frames until its middle is hers.
NEXT_SLOT = SLOT + (1 if T["BELT_PARTS_DIR"] == T["BELT_LEFT"] else -1)
NEXT_FRAMES = -(-16 * 256 // T["BELT_PARTS_SPEED"])
FIN_EXPR = f"[wFinishedSlots+{FIN_SLOT}]"
FACING_PARTS, FACING_FINISHED = 0, 1
ELLE_Y_PARTS, ELLE_Y_FINISHED = 58, 50          # include/layout.inc: she steps 8 px toward the belt she faces
FRAME_BACK, FRAME_FRONT, FRAME_SIDE = 0, 1, 2   # src/play.asm: frames of assets/sprites/elle


def holding(*pieces):
    """Pokes for Elle holding these halves (first grabbed first)."""
    hands = list(pieces) + [0] * (2 - len(pieces))
    wanted = partner(pieces[0]) if len(pieces) == 1 else 0
    return [f"wHands={hands[0]},{hands[1]}", f"wHeldCount={len(pieces)}", f"wWantedPiece={wanted}"]


def press(*buttons, at=1):
    """Key tokens pressing buttons together at frame `at`, released 1 frame later."""
    return " ".join([f"{b}@{at}" for b in buttons] + [f"-{b}@{at + 1}" for b in buttons])


# (rule, pokes, keys, frames, {expression: expected value})
SCENARIOS = [
    ("A grabs the piece in front with a free hand",
     PARTS_AT_SLOT + [f"wPartsSlots+{SLOT}={HEAD}"], press("A"), 6,
     {"[wHeldCount]": 1, "[wHands]": HEAD, SLOT_EXPR: 0, "[wGrabs]": 1, "[wBuzzes]": 0}),
    ("A grabs the half that matches the one held; halves never join on their own",
     PARTS_AT_SLOT + holding(HEAD) + [f"wPartsSlots+{SLOT}={BODY}"], press("A"), 90,
     {"[wHeldCount]": 2, "[wHands+1]": BODY, SLOT_EXPR: 0, "[wToy]": 0, "[wAssembles]": 0}),
    ("A refuses a half that doesn't match, with a buzz",
     PARTS_AT_SLOT + holding(HEAD) + [f"wPartsSlots+{SLOT}={ENGINE}"], press("A"), 6,
     {"[wHeldCount]": 1, SLOT_EXPR: ENGINE, "[wBuzzes]": 1, "[wGrabs]": 0}),
    ("A refuses the same half again, with a buzz",
     PARTS_AT_SLOT + holding(HEAD) + [f"wPartsSlots+{SLOT}={HEAD}"], press("A"), 6,
     {"[wHeldCount]": 1, SLOT_EXPR: HEAD, "[wBuzzes]": 1}),
    ("A with nothing in front buzzes",
     PARTS_AT_SLOT + [f"wPartsSlots+{SLOT}=0"], press("A"), 6,
     {"[wHeldCount]": 0, "[wBuzzes]": 1}),
    ("A with the piece out of reach buzzes",
     [f"wPartsX=0,{(SLOT * 16 - ELLE_CENTER_X) % 256}", f"wPartsSlots+{SLOT}={HEAD}"], press("A"), 3,
     {"[wHeldCount]": 0, SLOT_EXPR: HEAD, "[wBuzzes]": 1}),
    ("B with one half puts it back on the belt where Elle stands",
     PARTS_AT_SLOT + holding(HEAD) + [f"wPartsSlots+{SLOT}=0"], press("B"), 6,
     {"[wHeldCount]": 0, SLOT_EXPR: HEAD, "[wReturns]": 1, "[wBuzzes]": 0}),
    ("... and it keeps moving with the belt",
     PARTS_AT_SLOT + holding(HEAD) + [f"wPartsSlots+{SLOT}=0"], press("B"), 120,
     {SLOT_EXPR: HEAD, f"[wPartsUnder] != {SLOT}": 1}),
    ("B with two halves puts back the last one grabbed",
     PARTS_AT_SLOT + holding(HEAD, BODY) + [f"wPartsSlots+{SLOT}=0"], press("B"), W + 4,
     {"[wHeldCount]": 1, "[wHands]": HEAD, SLOT_EXPR: BODY, "[wToy]": 0}),
    (f"B then A {W - 1} frames later (inside the {W}-frame window) assembles",
     holding(HEAD, BODY), press("B") + " " + press("A", at=1 + W - 1), W + 4,
     {"[wToy]": TEDDY, "[wHeldCount]": 0, "[wAssembles]": 1, "[wReturns]": 0}),
    (f"A then B {W - 1} frames later (inside the window) assembles",
     holding(HEAD, BODY), press("A") + " " + press("B", at=1 + W - 1), W + 4,
     {"[wToy]": TEDDY, "[wAssembles]": 1, "[wBuzzes]": 0}),
    ("B and A on the same frame assemble",
     holding(BODY, HEAD), press("B", "A"), 4,
     {"[wToy]": TEDDY, "[wAssembles]": 1}),
    (f"B then A {W} frames later (outside the window) is a plain B: no toy",
     PARTS_AT_SLOT + holding(HEAD, BODY) + [f"wPartsSlots+{SLOT}=0"],
     press("B") + " " + press("A", at=1 + W), W + 4,
     {"[wToy]": 0, "[wAssembles]": 0, "[wReturns]": 1}),
    ("B+A assembles facing the Finished belt too",
     holding(HEAD, BODY) + [f"wFacing={FACING_FINISHED}"], press("B", "A"), 4,
     {"[wToy]": TEDDY, "[wAssembles]": 1}),
    ("B facing the Finished belt with a toy drops it there; score +1",
     FINISHED_AT_SLOT + [f"wToy={TEDDY}", f"wFacing={FACING_FINISHED}", f"wFinishedSlots+{FIN_SLOT}=0"],
     press("B"), 4,
     {"[wToy]": 0, FIN_EXPR: TEDDY, "u16[wScore]": 1, "[wDrops]": 1, "[wBuzzes]": 0}),
    ("B facing the parts belt with a toy buzzes",
     [f"wToy={TEDDY}"], press("B"), 4,
     {"[wToy]": TEDDY, "[wBuzzes]": 1, "u16[wScore]": 0}),
    ("A with a toy buzzes",
     PARTS_AT_SLOT + [f"wToy={TEDDY}", f"wPartsSlots+{SLOT}={HEAD}"], press("A"), 4,
     {"[wToy]": TEDDY, SLOT_EXPR: HEAD, "[wBuzzes]": 1}),
    ("B facing the Finished belt with a half buzzes",
     holding(HEAD) + [f"wFacing={FACING_FINISHED}"], press("B"), 4,
     {"[wHeldCount]": 1, "[wBuzzes]": 1, "[wReturns]": 0}),
    ("A facing the Finished belt buzzes",
     PARTS_AT_SLOT + [f"wFacing={FACING_FINISHED}", f"wPartsSlots+{SLOT}={HEAD}"], press("A"), 4,
     {"[wHeldCount]": 0, SLOT_EXPR: HEAD, "[wBuzzes]": 1}),
    ("B with empty hands buzzes",
     [], press("B"), 4,
     {"[wBuzzes]": 1, "[wReturns]": 0}),
    ("Up turns Elle to the Finished belt; she steps up toward it",
     [], press("UP"), TURN + 3,
     {"[wFacing]": FACING_FINISHED, "[wElleY]": ELLE_Y_FINISHED, "[wElleFrame]": FRAME_BACK}),
    ("Down turns her back to the parts belt; she steps down toward it",
     [f"wFacing={FACING_FINISHED}"], press("DOWN"), TURN + 3,
     {"[wFacing]": FACING_PARTS, "[wElleY]": ELLE_Y_PARTS, "[wElleFrame]": FRAME_FRONT}),
    (f"Mid-turn she shows her side, between the two, still facing the old belt",
     [], press("UP"), max(1, TURN // 2),
     {"[wFacing]": FACING_PARTS, "[wElleFrame]": FRAME_SIDE,
      f"[wElleY] < {ELLE_Y_PARTS}": 1, f"[wElleY] > {ELLE_Y_FINISHED}": 1}),
    (f"A and B pressed during the {TURN}-frame turn do nothing, with no buzz",
     PARTS_AT_SLOT + FINISHED_AT_SLOT + [f"wToy={TEDDY}", f"wFinishedSlots+{FIN_SLOT}=0"],
     press("UP") + " " + press("B", at=2) + " " + press("A", at=TURN), TURN + 4,
     {"[wToy]": TEDDY, "[wBuzzes]": 0, "[wDrops]": 0, "[wFacing]": FACING_FINISHED}),
    (f"... A pressed during the turn doesn't grab the piece still in front, nor start a chord",
     PARTS_AT_SLOT + [f"wPartsSlots+{SLOT}={HEAD}"], press("UP") + " " + press("A", at=TURN), TURN + 4,
     {"[wHeldCount]": 0, SLOT_EXPR: HEAD, "[wBuzzes]": 0, "[wChordA]": 0}),
    (f"B pressed {TURN} frames after Up, the first frame after the turn, acts on the new facing: drop, score +1",
     FINISHED_AT_SLOT + [f"wToy={TEDDY}", f"wFinishedSlots+{FIN_SLOT}=0"],
     press("UP") + " " + press("B", at=1 + TURN), TURN + 4,
     {"[wToy]": 0, FIN_EXPR: TEDDY, "u16[wScore]": 1, "[wBuzzes]": 0}),
    ("Holding one slinky coil, the wanted piece is another coil (the supply follows it)",
     PARTS_AT_SLOT + [f"wPartsSlots+{SLOT}={COIL}"], press("A"), 6,
     {"[wHeldCount]": 1, "[wWantedPiece]": COIL, "[wBuzzes]": 0}),
    ("A grabs two coils off the belt, one after the other, and B+A makes the slinky",
     PARTS_AT_SLOT + [f"wPartsSlots+{SLOT}={COIL}", f"wPartsSlots+{NEXT_SLOT}={COIL}"],
     press("A") + " " + press("A", at=1 + NEXT_FRAMES) + " " + press("B", "A", at=4 + NEXT_FRAMES),
     8 + NEXT_FRAMES,
     {"[wToy]": SLINKY, "[wGrabs]": 2, "[wAssembles]": 1, "[wBuzzes]": 0}),
    ("A coil refuses a different toy's half, with a buzz",
     PARTS_AT_SLOT + [f"wPartsSlots+{SLOT}={COIL}", f"wPartsSlots+{NEXT_SLOT}={ENGINE}"],
     press("A") + " " + press("A", at=1 + NEXT_FRAMES), 4 + NEXT_FRAMES,
     {"[wHeldCount]": 1, f"[wPartsSlots+{NEXT_SLOT}]": ENGINE, "[wBuzzes]": 1}),
    ("The round ends when the clock runs out, on the results screen",
     ["wRoundLeft=3,0"], "", 8, {"[wGameState]": 1, "[wResultsAfterRound]": 1}),
    ("START on the results screen goes back to the title",
     ["wRoundLeft=3,0"], press("START", at=30), 50, {"[wGameState]": 1, "[wResultsAfterRound]": 0}),
    ("... and START on the title starts a new round",
     ["wRoundLeft=3,0"], press("START", at=30) + " " + press("START", at=70), 90,
     {"[wGameState]": 0, "u16[wScore]": 0}),
]


def check(cli, pokes, keys, frames, expects, at="play"):
    args = ["run", "--session", SESSION, "--at", at, "--frames", str(frames)]
    for p in pokes:
        args += ["--poke", p]
    if keys:
        args += ["--keys", keys]
    for expr in expects:
        args += ["--read", expr]
    reply = cli.run(*args, check=False)
    if not reply.get("ok"):
        err = reply.get("error", {})
        return [f"gbcli {err.get('code')}: {err.get('message')}"]
    got ={r["expr"]: r.get("value") for r in reply["result"]["reads"]}
    wrong = [f"{e} = {got.get(e)} (want {v})" for e, v in expects.items() if got.get(e) != v]
    return wrong


def save_scenario(cli):
    """A new best is saved, kept across a power cycle, and not lowered."""
    wrong = check(cli, ["wRoundLeft=3,0", "wScore=$07,0"], "", 8,
                  {"u16[wBest]": 7, "[wNewBest]": 1})
    if wrong:
        return wrong
    cli.run("power-cycle", "--session", SESSION)
    # power-cycle starts at power-on: the boot ROM runs first.
    reply = cli.run("run", "--session", SESSION, "--to", "MainLoop", "--read", "u16[wBest]")
    best = reply["result"]["reads"][0]["value"]
    if best != 7:
        return [f"after power-cycle u16[wBest] = {best} (want 7)"]
    # A lower score next round leaves the best alone.
    cli.run("run", "--session", SESSION, "--frames", "30", "--keys", "START@1 -START@2", "--checkpoint", "second")
    return check(cli, ["wRoundLeft=3,0", "wScore=$03,0"], "", 8,
                 {"u16[wBest]": 7, "[wNewBest]": 0}, at="second")


T_TOYS, T_ELLE = 74, 146           # include/layout.inc: held things' tiles, then Elle's
OBJECTS = 10                       # OAM entries Elle and what she holds use


def objects(cli, pokes):
    """[(index, y, x, kind)] of the visible objects after a few frames; kind
    is 'elle' or 'held' by tile number."""
    exprs = [f"oam[{i}].{f}" for i in range(OBJECTS) for f in ("y", "x", "tile")]
    args = ["run", "--session", SESSION, "--at", "play", "--frames", "4"]
    for p in pokes:
        args += ["--poke", p]
    for e in exprs:
        args += ["--read", e]
    got = {r["expr"]: r.get("value") for r in cli.run(*args)["result"]["reads"]}
    out = []
    for i in range(OBJECTS):
        y, x, tile = (got[f"oam[{i}].{f}"] for f in ("y", "x", "tile"))
        if 0 < y < 160:
            out.append((i, y, x, "elle" if tile >= T_ELLE else "held" if tile >= T_TOYS else "?"))
    return out


def draw_order(objs, front):
    """What's wrong with who draws over whom. On a GBC the lower OAM index
    draws on top; on a DMG the smaller X does, then the lower index. `front`
    is the object kind that must win wherever an Elle object and a held one
    overlap, on both machines."""
    wrong = []
    elle = [o for o in objs if o[3] == "elle"]
    held = [o for o in objs if o[3] == "held"]
    if not elle or not held:
        return [f"want Elle and held objects, got {objs}"]
    for e in elle:
        for h in held:
            if abs(e[1] - h[1]) >= 16 or abs(e[2] - h[2]) >= 8:
                continue                # they don't overlap
            top, under = (e, h) if front == "elle" else (h, e)
            if top[0] > under[0]:
                wrong.append(f"GBC: OAM {under[0]} ({under[3]}) draws over OAM {top[0]} ({top[3]})")
            if top[2] > under[2] or (top[2] == under[2] and top[0] > under[0]):
                wrong.append(f"DMG: OAM {under[0]} ({under[3]}, x {under[2]}) draws over "
                             f"OAM {top[0]} ({top[3]}, x {top[2]})")
    return wrong


def beside(objs):
    """The held objects that overlap no Elle object: what peeks out at her sides."""
    elle = [o for o in objs if o[3] == "elle"]
    return [h for h in objs if h[3] == "held"
            and all(abs(e[1] - h[1]) >= 16 or abs(e[2] - h[2]) >= 8 for e in elle)]


def draw_order_scenarios(cli):
    """(rule, wrong) for who draws over whom, facing each way."""
    away = [f"wFacing={FACING_FINISHED}"]
    out = []
    objs = objects(cli, away + [f"wToy={TEDDY}"])
    wrong = draw_order(objs, "elle")
    if beside(objs):
        wrong.append(f"the toy shows beside her: {beside(objs)}")
    out.append(("Facing away, Elle draws over the toy she holds, hidden behind her (DMG and GBC)", wrong))
    objs = objects(cli, away + holding(HEAD, BODY))
    wrong = draw_order(objs, "elle")
    if len(beside(objs)) != 2:
        wrong.append(f"want one column of each half beside her, got {beside(objs)}")
    out.append(("Facing away, Elle draws over the halves she holds; each peeks out at a side (DMG and GBC)", wrong))
    wrong = draw_order(objects(cli, holding(HEAD, BODY)), "held")
    wrong += draw_order(objects(cli, [f"wToy={TEDDY}"]), "held")
    out.append(("Facing the parts belt, the halves or toy she holds draw over her (DMG and GBC)", wrong))
    return out


def main():
    p = g.arg_parser(__doc__.splitlines()[0])
    p.add_argument("--only", help="run only rules whose text contains this")
    args = p.parse_args()
    cli = g.Gbcli(args.gbcli, args.verbose)
    start = time.time()
    # A fresh session (and save) every time.
    cli.text("sessions", "rm", SESSION)
    cli.run("run", g.ROM, "--session", SESSION, "--from", "boot", "--frames", "30")
    cli.run("run", "--session", SESSION, "--frames", "30", "--keys", "START@1 -START@2", "--checkpoint", "play")

    results = []
    for rule, pokes, keys, frames, expects in SCENARIOS:
        if args.only and args.only not in rule:
            continue
        wrong = check(cli, pokes, keys, frames, expects)
        results.append(not wrong)
        print(f"{'PASS' if not wrong else 'FAIL'} {rule}" + (": " + "; ".join(wrong) if wrong else ""))
    for rule, wrong in draw_order_scenarios(cli):
        if args.only and args.only not in rule:
            continue
        results.append(not wrong)
        print(f"{'PASS' if not wrong else 'FAIL'} {rule}" + (": " + "; ".join(wrong) if wrong else ""))
    rule = "A new best is saved in battery RAM, kept across a power cycle, and a lower score keeps it"
    if not args.only or args.only in rule:
        wrong = save_scenario(cli)
        results.append(not wrong)
        print(f"{'PASS' if not wrong else 'FAIL'} {rule}" + (": " + "; ".join(wrong) if wrong else ""))

    print(f"{sum(results)}/{len(results)} rules pass; {cli.calls} gbcli calls, {time.time() - start:.1f} s")
    sys.exit(0 if all(results) else 1)


if __name__ == "__main__":
    main()
