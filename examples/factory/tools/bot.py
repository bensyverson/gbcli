"""A closed-loop bot that plays one round through gbcli, then checks the best.

    python3 tools/bot.py [--gbcli PATH] [-v]

Session `bot`: from boot, press START, then repeat: read the state (hands,
facing, the parts belt's slots and scroll), choose the next move, and run
just far enough to make it (a piece reaching Elle's reach, a chord, a
drop). Every key press is logged at its frame since boot in build/bot.keys,
so the whole round replays in one run:

    gbcli run build/factory.gb --session replay --from boot --frames N \
        --keys-file build/bot.keys --keys-since boot --frame-budget

After the round: the results screen (build/prove/bot-results.png), a
power cycle with the best read back, and a second round left to run out,
whose results screen must show the same best and no "New best!"
(build/prove/bot-results2.png).
"""

import sys
import time

import gbcli as g
import tuning

SESSION = "bot"
T = tuning.load()
ELLE_CENTER_X = 68                  # include/layout.inc
KEY_X = 130                         # include/layout.inc
SPEED = T["BELT_PARTS_SPEED"]
SIGN = 1 if T["BELT_PARTS_DIR"] == T["BELT_LEFT"] else -1
REACH = T["PICKUP_REACH"]
TURN = T["TURN_FRAMES"]               # a turn ignores A and B this long
SLOTS = 16

STATE_READS = ["[wGameState]", "[wHeldCount]", "[wHands]", "[wHands+1]", "[wToy]",
               "[wFacing]", "[wWantedPiece]", "u16[wPartsX]", "u16[wRoundLeft]", "u16[wScore]"]
STATE_READS += [f"[wPartsSlots+{s}]" for s in range(SLOTS)]


class Bot:
    def __init__(self, cli):
        self.cli = cli
        self.keys = []               # "A@123" tokens, frames since boot
        self.frame = 0               # the head's frame since boot
        self.state = {}

    def run(self, frames, presses=(), extra=()):
        """Runs `frames` frames, pressing (button, offset) pairs; reads the state."""
        tokens = []
        for button, at in presses:
            tokens += [f"{button}@{at}", f"-{button}@{at + 1}"]
            self.keys += [f"{button}@{self.frame + at}", f"-{button}@{self.frame + at + 1}"]
        args = ["run", "--session", SESSION, "--frames", str(frames), *extra]
        if tokens:
            args += ["--keys", " ".join(tokens)]
        for r in STATE_READS:
            args += ["--read", r]
        reply = self.cli.run(*args)
        result = reply["result"]
        self.frame = result["stop"]["time"]["frame"]
        self.state = {r["expr"]: r.get("value") for r in result["reads"]}
        return result

    def target(self):
        """(frames until a wanted piece is centered on Elle, its slot), or None."""
        s = self.state
        x = s["u16[wPartsX]"]
        wanted = s["[wWantedPiece]"] if s["[wHeldCount]"] == 1 else None
        best = None
        for slot in range(SLOTS):
            piece = s[f"[wPartsSlots+{slot}]"]
            if not piece or (wanted is not None and piece != wanted):
                continue
            middle = (slot * 16 + 8 - (x >> 8)) % 256   # its middle's screen x now
            # Only dealt slots count: a slot beyond the arriving one (src/belts.asm,
            # ARRIVING_SLOT) is dealt afresh before it shows.
            if SIGN > 0:
                if middle >= KEY_X + 24:                # arriving: middle 138-153
                    continue
            else:
                if middle >= KEY_X + 24:
                    middle -= 256                       # off the left edge
                if middle <= -24:                       # arriving: middle -23..-8
                    continue
            ahead = (middle - ELLE_CENTER_X) * SIGN     # px still to travel to Elle
            if abs(ahead) <= REACH - 2:
                t = 1                                   # in reach now
            elif ahead > 0:
                t = -(-(ahead * 256) // SPEED)
            else:
                continue                                # gone past
            if best is None or t < best[0]:
                best = (t, slot)
        return best

    def step(self):
        s = self.state
        if s["[wToy]"]:
            # Turn to the Finished belt, drop it as the turn ends, turn back.
            return self.run(2 * TURN + 4, [("UP", 1), ("B", 1 + TURN), ("DOWN", 2 + TURN)])
        if s["[wHeldCount]"] == 2:
            return self.run(3, [("B", 1), ("A", 1)])
        if s["[wFacing]"] != 0:
            return self.run(3, [("DOWN", 1)])
        found = self.target()
        if found is None:
            return self.run(16)
        t, _ = found
        return self.run(t + 2, [("A", t)])


def main():
    args = g.arg_parser(__doc__.splitlines()[0]).parse_args()
    cli = g.Gbcli(args.gbcli, args.verbose)
    start = time.time()
    (g.PROJECT / "build" / "prove").mkdir(parents=True, exist_ok=True)
    cli.text("sessions", "rm", SESSION)
    bot = Bot(cli)
    bot.run(60, extra=[g.ROM, "--from", "boot"])
    bot.run(4, [("START", 1)])
    bot.run(60, extra=["--until", "[wGameState] == 0"])   # the round's screen is drawn with the LCD off
    steps = 0
    while bot.state["[wGameState]"] == 0:
        bot.step()
        steps += 1
    score = bot.state["u16[wScore]"]
    play_calls = cli.calls
    play_time = time.time() - start
    keys_path = g.PROJECT / "build" / "bot.keys"
    keys_path.write_text("# Teddy & Toys: the bot's round, frames since boot (tools/bot.py)\n"
                         + "\n".join(bot.keys) + "\n")
    rows = []
    ok = True

    def claim(name, good, detail):
        nonlocal ok
        ok = ok and good
        rows.append(f"{'PASS' if good else 'FAIL'} {name}: {detail}")

    best1 = cli.run("run", "--session", SESSION, "--frames", "30", "--shot", "build/prove/bot-results.png",
                    "--read", "u16[wBest]", "--read", "[wNewBest]")["result"]["reads"]
    claim("round", score > 0, f"round played to the results screen at frame {bot.frame}, "
          f"score {score:x} (BCD), {steps} moves, {play_calls} gbcli calls, {play_time:.1f} s")
    claim("best", best1[0]["value"] == score and best1[1]["value"] == 1,
          f"u16[wBest] = {best1[0]['value']:x}, wNewBest = {best1[1]['value']} (build/prove/bot-results.png)")
    cli.run("power-cycle", "--session", SESSION)
    kept = cli.run("run", "--session", SESSION, "--to", "MainLoop", "--read", "u16[wBest]")["result"]["reads"][0]
    claim("power cycle", kept["value"] == score, f"after power-cycle u16[wBest] = {kept['value']:x}")
    cli.run("sav", "build/prove/bot.sav", "--session", SESSION)
    # A second round, left to run out: score 0, the best stays.
    cli.run("run", "--session", SESSION, "--frames", "60")
    cli.run("run", "--session", SESSION, "--frames", "4", "--keys", "START@1 -START@2")
    after = cli.run("run", "--session", SESSION, "--until", "[wGameState] == 1", "--frames", "4000",
                    "--wall-guard", "120")
    second = cli.run("run", "--session", SESSION, "--frames", "30", "--shot", "build/prove/bot-results2.png",
                     "--read", "u16[wBest]", "--read", "[wNewBest]", "--read", "u16[wScore]")["result"]["reads"]
    claim("second round", second[0]["value"] == score and second[1]["value"] == 0 and second[2]["value"] == 0,
          f"score {second[2]['value']}, best {second[0]['value']:x}, wNewBest {second[1]['value']} "
          f"(build/prove/bot-results2.png); save in build/prove/bot.sav")
    print("\n".join(rows))
    print(f"keys: build/bot.keys ({len(bot.keys)} tokens, last frame {bot.frame}); "
          f"total {cli.calls} gbcli calls, {time.time() - start:.1f} s")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
