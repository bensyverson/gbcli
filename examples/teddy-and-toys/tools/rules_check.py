"""The rules check: one scripted scenario per rule, pass or fail per rule.

    python3 tools/rules_check.py [--gbcli PATH] [-v] [--only TEXT]

Each scenario starts from the same moment of a round (checkpoint `play` in
session `rules`: START pressed, 30 frames in, facing the parts belt, empty
hands), pokes the state it needs (what Elle holds, a piece on the belt in
front of her), presses keys, runs a few frames and reads the result. Rules
come from the controls table in project/2026-11-16-teddy-and-toys-plan.md
and Ben's play rulings there (pause, the music's speed-up, stepping, the
best on the title), and the numbers from include/tuning.inc (the chord
window, the step range, the red tempo), so a tuning change is checked as
it stands. Needs a fresh build (tools/make.py).
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
STATE_PLAY, STATE_RESULTS, STATE_PAUSED = 0, 1, 2   # src/main.asm: wGameState
SFX_PAUSE, SFX_SHUFFLE = 9, 10                  # src/sound.asm
ELLE_X = 60                                     # include/layout.inc: her home spot (left edge)
STEP = T["ELLE_STEP_RANGE"]                     # px each way from home
STEP_SPEED = T["ELLE_STEP_SPEED"]               # px a frame
RED_FRAMES = T["CLOCK_RED_SECONDS"] * 60
SONG_TICKS = 6                                  # assets/music/theme.song: ticks_per_row
RED_TICKS = T["MUSIC_RED_TICKS_PER_ROW"]
NR51 = "[$FF25]"                                # which channels reach the speakers


def at_x(elle_x, slot, belt="wPartsX"):
    """A poke scrolling a belt so slot's middle is under Elle's middle when
    her left edge is at elle_x."""
    return f"{belt}=0,{(slot * 16 + 8 - (elle_x + 8)) % 256}"


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

    # START pauses (Ben's play rulings, 2026-11-17).
    ("START pauses: the Continue/Quit menu opens on Continue, with the pause sound",
     [], press("START"), 20,
     {"[wGameState]": STATE_PAUSED, "[wPauseCursor]": 0, "[wSfxLast]": SFX_PAUSE}),
    ("Paused, only the effects' channel reaches the speakers: the music is silent",
     [], press("START"), 20, {NR51: 0x11}),
    ("Down moves the cursor to Quit, Up back to Continue",
     [], press("START") + " " + press("DOWN", at=8), 14, {"[wPauseCursor]": 1}),
    ("... Up back to Continue",
     [], press("START") + " " + press("DOWN", at=8) + " " + press("UP", at=14), 20, {"[wPauseCursor]": 0}),
    ("START again continues, and the music plays again",
     [], press("START") + " " + press("START", at=20), 30,
     {"[wGameState]": STATE_PLAY, NR51: 0xFF}),
    ("B continues too, without a buzz",
     [], press("START") + " " + press("B", at=10), 20,
     {"[wGameState]": STATE_PLAY, "[wBuzzes]": 0, "[wReturns]": 0}),
    ("A on Continue continues, and doesn't grab the piece in front",
     PARTS_AT_SLOT + [f"wPartsSlots+{SLOT}={HEAD}"], press("START") + " " + press("A", at=10), 14,
     {"[wGameState]": STATE_PLAY, "[wHeldCount]": 0, "[wGrabs]": 0, "[wBuzzes]": 0}),
    ("A on Quit goes to the title: no results screen, no best saved",
     ["wScore=9,0"], press("START") + " " + press("DOWN", at=8) + " " + press("A", at=14), 40,
     {"[wGameState]": STATE_RESULTS, "[wResultsAfterRound]": 0, "u16[wBest]": 0, "[wNewBest]": 0}),
    ("START on Quit quits too",
     [], press("START") + " " + press("DOWN", at=8) + " " + press("START", at=14), 40,
     {"[wGameState]": STATE_RESULTS, "[wResultsAfterRound]": 0}),
    ("... and START on the title then starts a new round",
     [], press("START") + " " + press("DOWN", at=8) + " " + press("START", at=14) + " " + press("START", at=50), 70,
     {"[wGameState]": STATE_PLAY, "u16[wScore]": 0}),
    ("START pauses mid-turn too, and the turn waits for Continue",
     [], press("UP") + " " + press("START", at=3), 30,
     {"[wGameState]": STATE_PAUSED, "[wFacing]": FACING_PARTS, "[wTurnTimer] > 0": 1}),
    ("... and finishes after it",
     [], press("UP") + " " + press("START", at=3) + " " + press("START", at=30), 30 + TURN + 4,
     {"[wGameState]": STATE_PLAY, "[wFacing]": FACING_FINISHED, "[wTurnTimer]": 0}),
    ("Left and Right don't step Elle while paused",
     [], press("START") + " RIGHT@6 -RIGHT@40", 42, {"[wElleX]": ELLE_X}),

    # The music speeds up when the clock turns red.
    ("The music plays at the song's tempo before the clock turns red",
     [f"wRoundLeft={(RED_FRAMES + 20) % 256},{(RED_FRAMES + 20) // 256}"], "", 10,
     {"[ticks_per_row]": SONG_TICKS}),
    (f"When the clock turns red the tempo steps up once: {RED_TICKS} ticks a row",
     [f"wRoundLeft={(RED_FRAMES + 3) % 256},{(RED_FRAMES + 3) // 256}"], "", 30,
     {"[ticks_per_row]": RED_TICKS}),
    ("... the title is back at the song's tempo after the results",
     [f"wRoundLeft={(RED_FRAMES + 3) % 256},{(RED_FRAMES + 3) // 256}"], press("START", at=RED_FRAMES + 40),
     RED_FRAMES + 60,
     {"[wGameState]": STATE_RESULTS, "[wResultsAfterRound]": 0, "[ticks_per_row]": SONG_TICKS}),
    ("... and after Quit",
     [f"wRoundLeft={(RED_FRAMES + 3) % 256},{(RED_FRAMES + 3) // 256}"],
     press("START", at=20) + " " + press("DOWN", at=26) + " " + press("A", at=32), 60,
     {"[wGameState]": STATE_RESULTS, "[ticks_per_row]": SONG_TICKS}),
    ("... and the next round starts at the song's tempo",
     [f"wRoundLeft={(RED_FRAMES + 3) % 256},{(RED_FRAMES + 3) // 256}"],
     press("START", at=RED_FRAMES + 40) + " " + press("START", at=RED_FRAMES + 80), RED_FRAMES + 100,
     {"[wGameState]": STATE_PLAY, "[ticks_per_row]": SONG_TICKS}),
    ("Pausing in the red silences the music; Continue keeps the faster tempo",
     [f"wRoundLeft={(RED_FRAMES + 3) % 256},{(RED_FRAMES + 3) // 256}"],
     press("START", at=20) + " " + press("START", at=40), 50,
     {"[wGameState]": STATE_PLAY, "[ticks_per_row]": RED_TICKS, NR51: 0xFF}),

    # Elle steps left and right.
    (f"Right steps Elle right, {STEP} px from home at most, with the shuffle sound",
     [], "RIGHT@1 -RIGHT@80", 82, {"[wElleX]": ELLE_X + STEP, "[wSfxLast]": SFX_SHUFFLE}),
    (f"Left steps her left, {STEP} px from home at most",
     [], "LEFT@1 -LEFT@80", 82, {"[wElleX]": ELLE_X - STEP}),
    ("A step is gradual: a few frames of Right move her a few px",
     [], "RIGHT@1 -RIGHT@5", 8,
     {f"[wElleX] > {ELLE_X}": 1, f"[wElleX] < {ELLE_X + STEP}": 1}),
    ("She steps facing the Finished belt too",
     [f"wFacing={FACING_FINISHED}"], "LEFT@1 -LEFT@80", 82, {"[wElleX]": ELLE_X - STEP}),
    ("She doesn't step during a turn",
     [], press("UP") + f" RIGHT@2 -RIGHT@{1 + TURN}", TURN + 4,
     {"[wElleX]": ELLE_X, "[wFacing]": FACING_FINISHED}),
    ("Stepped right, A grabs the piece in front of where she stands",
     [f"wElleX={ELLE_X + STEP}", at_x(ELLE_X + STEP, SLOT), f"wPartsSlots+{SLOT}={HEAD}"], press("A"), 6,
     {"[wHeldCount]": 1, "[wHands]": HEAD, SLOT_EXPR: 0, "[wBuzzes]": 0}),
    ("... and not the piece in front of her home spot",
     [f"wElleX={ELLE_X + STEP}", at_x(ELLE_X, SLOT), f"wPartsSlots+{SLOT}={HEAD}",
      f"wPartsSlots+{SLOT + 1}=0", f"wPartsSlots+{SLOT + 2}=0"], press("A"), 3,
     {"[wHeldCount]": 0, SLOT_EXPR: HEAD, "[wBuzzes]": 1}),
    ("Stepped left, B returns a half to the belt where she stands",
     [f"wElleX={ELLE_X - STEP}", at_x(ELLE_X - STEP, SLOT), f"wPartsSlots+{SLOT}=0"] + holding(HEAD),
     press("B"), 4, {"[wHeldCount]": 0, SLOT_EXPR: HEAD}),
    ("Stepped left, a toy set on the Finished belt lands where she stands",
     [f"wElleX={ELLE_X - STEP}", at_x(ELLE_X - STEP, FIN_SLOT, "wFinishedX"), f"wToy={TEDDY}",
      f"wFacing={FACING_FINISHED}", f"wFinishedSlots+{FIN_SLOT}=0"], press("B"), 4,
     {FIN_EXPR: TEDDY, "u16[wScore]": 1}),
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


HELD_STILL = ["u16[wRoundLeft]", "u16[wPartsX]", "u16[wFinishedX]", "[wClockStep]", "[wTurnTimer]",
              "[row]", "[tick]", "[current_order]"]


def paused_still(cli):
    """Paused, the clock, both belts, a turn and the music's place hold still,
    from 10 frames after START to 70; then Continue starts them again."""
    def state(frames, keys):
        args = ["run", "--session", SESSION, "--at", "play", "--frames", str(frames), "--keys", keys]
        for e in HELD_STILL:
            args += ["--read", e]
        return {r["expr"]: r.get("value") for r in cli.run(*args)["result"]["reads"]}
    pause = press("UP") + " " + press("START", at=3)
    early, late = state(13, pause), state(73, pause)
    wrong = [f"{e} moved while paused: {early[e]} -> {late[e]}" for e in HELD_STILL if early[e] != late[e]]
    resumed = state(100, pause + " " + press("START", at=73))
    wrong += [f"{e} still held after Continue: {late[e]}" for e in ("u16[wRoundLeft]", "u16[wPartsX]")
              if resumed[e] == late[e]]
    return wrong


def stepped_objects(cli):
    """(rule, wrong) for Elle's objects at each step limit: they move with her,
    and who draws over whom still holds, facing each way."""
    wrong = []
    for x in (ELLE_X - STEP, ELLE_X + STEP):
        at = [f"wElleX={x}"]
        objs = objects(cli, at + holding(HEAD, BODY))
        elle_x = min((o[2] for o in objs if o[3] == "elle"), default=None)
        if elle_x != x + 8:                     # OAM x is the screen x + 8
            wrong.append(f"at x {x}: Elle's leftmost object at OAM x {elle_x} (want {x + 8})")
        wrong += [f"at x {x}: {w}" for w in draw_order(objs, "held")]
        objs = objects(cli, at + [f"wFacing={FACING_FINISHED}"] + holding(HEAD, BODY))
        wrong += [f"at x {x}, facing away: {w}" for w in draw_order(objs, "elle")]
        if len(beside(objs)) != 2:
            wrong.append(f"at x {x}, facing away: want a column of each half beside her, got {beside(objs)}")
        objs = objects(cli, at + [f"wFacing={FACING_FINISHED}", f"wToy={TEDDY}"])
        wrong += [f"at x {x}, toy facing away: {w}" for w in draw_order(objs, "elle")]
    return [("At each step limit, Elle and what she holds move with her, drawn in the same order (DMG and GBC)",
             wrong)]


def title_best(cli):
    """After the power cycle in save_scenario: the title shows the best (7).
    The title's best line is composed in wLine ("Best: 7": glyph numbers,
    ASCII - 32, then MAIN_FONT_END) and set in tiles on map row 12."""
    best_row = 12                       # src/results.asm: TITLE_BEST_ROW
    reads = ["u16[wBest]", "[wGameState]", "[wResultsAfterRound]", "[wLine+6]", "[wLine+7]"]
    reads += [f"[${0x9800 + best_row * 32 + c:04X}]" for c in range(20)]
    reply = cli.run("run", "--session", SESSION, "--frames", "10", *sum((["--read", r] for r in reads), []))
    got = {r["expr"]: r.get("value") for r in reply["result"]["reads"]}
    wrong = [f"{e} = {got[e]} (want {v})" for e, v in
             {"u16[wBest]": 7, "[wGameState]": STATE_RESULTS, "[wResultsAfterRound]": 0,
              "[wLine+6]": ord("7") - 32, "[wLine+7]": 0xFF}.items() if got[e] != v]
    inked = sum(1 for c in range(20) if got[f"[${0x9800 + best_row * 32 + c:04X}]"])
    if inked < 4:
        wrong.append(f"map row {best_row} has {inked} text tiles (want 4 or more)")
    return wrong


def save_scenario(cli):
    """A new best is saved, kept across a power cycle, and not lowered.
    Returns (what's wrong with that, what's wrong with the title's best)."""
    unreached = ["the power cycle wasn't reached"]
    wrong = check(cli, ["wRoundLeft=3,0", "wScore=$07,0"], "", 8,
                  {"u16[wBest]": 7, "[wNewBest]": 1})
    if wrong:
        return wrong, unreached
    cli.run("power-cycle", "--session", SESSION)
    # power-cycle starts at power-on: the boot ROM runs first.
    reply = cli.run("run", "--session", SESSION, "--to", "MainLoop", "--read", "u16[wBest]")
    best = reply["result"]["reads"][0]["value"]
    if best != 7:
        return [f"after power-cycle u16[wBest] = {best} (want 7)"], unreached
    title = title_best(cli)
    # A lower score next round leaves the best alone.
    cli.run("run", "--session", SESSION, "--frames", "30", "--keys", "START@1 -START@2", "--checkpoint", "second")
    return check(cli, ["wRoundLeft=3,0", "wScore=$03,0"], "", 8,
                 {"u16[wBest]": 7, "[wNewBest]": 0}, at="second"), title


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
    def report(rule, wrong):
        results.append(not wrong)
        print(f"{'PASS' if not wrong else 'FAIL'} {rule}" + (": " + "; ".join(wrong) if wrong else ""))

    for rule, wrong in draw_order_scenarios(cli) + stepped_objects(cli):
        if not args.only or args.only in rule:
            report(rule, wrong)
    rule = "Paused, the clock, the belts, a turn and the music's place hold still; Continue starts them again"
    if not args.only or args.only in rule:
        report(rule, paused_still(cli))
    rule = "A new best is saved in battery RAM, kept across a power cycle, and a lower score keeps it"
    title_rule = "After the power cycle, the title shows the saved best"
    if not args.only or args.only in rule or args.only in title_rule:
        saved, title = save_scenario(cli)
        report(rule, saved)
        report(title_rule, title)

    print(f"{sum(results)}/{len(results)} rules pass; {cli.calls} gbcli calls, {time.time() - start:.1f} s")
    sys.exit(0 if all(results) else 1)


if __name__ == "__main__":
    main()
