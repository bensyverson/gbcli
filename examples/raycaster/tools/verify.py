#!/usr/bin/env python3
"""Reruns every claim in README.md, in order, and says which hold.

  python3 tools/verify.py --gbcli PATH

Each step is a command from the README, run from this project in fresh
sessions (verify, verifyc, verifyp), with the exit status and output it must
give. Exit 0 when every step passes. Takes a minute or two.
"""

import argparse
import os
import re
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
PROJECT = os.path.normpath(os.path.join(HERE, ".."))
START = ["--frames", "120", "--keys", "START@10 -START@12", "--checkpoint", "start"]
SAFE = "([rSTAT] & 3) != 1"


def steps(g):
    py = [sys.executable, "-I"]
    t = lambda name: os.path.join(HERE, name)  # noqa: E731
    return [
        ("assets regenerate", py + [t("gen_font.py"), "--check"], r"ok: .*font\.pixels"),
        ("title regenerates", py + [t("gen_title.py"), "--check"], r"^ok:"),
        ("maze regenerates", py + [t("gen_maze.py"), "--check"], r"^ok:"),
        ("tables regenerate", py + [t("gen_tables.py"), "--check"], r"^ok:"),
        ("DMG build clean", [g, "build"], r"errors=0 warnings=0"),
        ("GBC build clean", [g, "build", "--gbc"], r"errors=0 warnings=0"),
        ("DMG session", [g, "run", "build/raycaster.gb", "--session", "verify"] + START, r"checkpoint name=start"),
        ("GBC session", [g, "run", "build/gbc/raycaster.gbc", "--session", "verifyc", "--model", "gbc"] + START,
         r"checkpoint name=start"),
        ("walls match the reference (DMG)", py + [t("check_poses.py"), "--gbcli", g, "--session", "verify"],
         r"^ok 16 of 16 views"),
        ("walls match the reference (GBC)", py + [t("check_poses.py"), "--gbcli", g, "--session", "verifyc"],
         r"^ok 16 of 16 views"),
        ("cast holds its bound", [g, "measure", "MainLoop..MainLoop.waitCopy", "--session", "verify", "--at", "start",
                                  "--iterations", "25", "--frames", "2000", "--keys-file", "keys/walk.keys",
                                  "--max", "300000"], r"max_t=300000"),
        ("view rate", py + [t("bench.py"), "--gbcli", g, "verify", "verifyc"], r"period_mean"),
        ("the bot wins", py + [t("bot.py"), "--gbcli", g, "--session", "verify"], r"won at frame"),
        ("the recorded game wins", [g, "run", "--session", "verify", "--at", "start", "--frames", "4180",
                                    "--keys-file", "keys/win.keys", "--expect", "[wState] == 1"], r"ok=1"),
        ("it wins on the GBC too", [g, "run", "--session", "verifyc", "--at", "start", "--frames", "4180",
                                    "--keys-file", "keys/win.keys", "--expect", "[wState] == 1"], r"ok=1"),
        ("a lost human wins", py + [t("human.py"), "--gbcli", g, "--session", "verify"], r"^won at frame"),
        ("the human's game wins with oil to spare", [g, "run", "--session", "verify", "--at", "start",
                                                     "--frames", "10627", "--keys-file", "keys/human.keys",
                                                     "--expect", "[wState] == 1", "--expect", "[wOilPx] >= 64"],
         r"ok=1"),
        ("and on the GBC", [g, "run", "--session", "verifyc", "--at", "start", "--frames", "10627",
                            "--keys-file", "keys/human.keys", "--expect", "[wState] == 1",
                            "--expect", "[wOilPx] >= 64"], r"ok=1"),
        ("the lamp runs out", [g, "run", "--session", "verify", "--at", "start", "--until", "[wState] == 2",
                               "--frames", "10000", "--keys-file", "keys/lose.keys"], r"stop predicate"),
        ("START goes back to the title", [g, "run", "--session", "verify", "--frames", "30", "--keys",
                                          "START@5 -START@7", "--expect", "[wMode] == 0"], r"ok=1"),
        ("a view left waiting doesn't flip the title", [g, "run", "--session", "verify", "--frames", "3",
                                                         "--poke", "wFlipReady=1", "--expect", "[rLCDC] == $91"],
         r"ok=1"),
        ("and START starts a new game", [g, "run", "--session", "verify", "--frames", "60", "--keys",
                                         "START@5 -START@7", "--expect", "[wMode] == 1", "--expect",
                                         "[wKeys] == 0"], r"ok=1"),
        ("effects own channel 1", [g, "run", "--session", "verify", "--at", "start", "--frames", "4180",
                                   "--keys-file", "keys/win.keys", "--audio-summary", "--audio-channel", "1"],
         r"symbol=SfxTick"),
        ("VRAM proof: session", [g, "run", "build/raycaster.gb", "--session", "verifyp"] + START, r"checkpoint"),
        ("VRAM proof: OAM DMA", [g, "break", "VBlank.oamDone", "--if", SAFE, "--session", "verifyp"], r"breakpoint"),
        ("VRAM proof: HUD", [g, "break", "VBlank.hudDone", "--if", SAFE, "--session", "verifyp"], r"breakpoint"),
        ("VRAM proof: copy", [g, "break", "CopyChunks.copied", "--if", SAFE, "--session", "verifyp"], r"breakpoint"),
        ("VRAM proof: flip", [g, "break", "ShowView.flipped", "--if", SAFE, "--session", "verifyp"], r"breakpoint"),
        ("VRAM proof: the keys' OAM DMA", [g, "break", "ShowView.oamDone", "--if", SAFE, "--session", "verifyp"],
         r"breakpoint"),
        ("VRAM proof: watch set B", [g, "watch", "write", "$97FF", "--if", "[$97FF] != new", "--session", "verifyp"],
         r"watch"),
        ("VRAM proof: watch set A", [g, "watch", "write", "$87FF", "--if", "[$87FF] != new", "--session", "verifyp"],
         r"watch"),
        ("no VRAM write outside VBlank", [g, "run", "--session", "verifyp", "--at", "start", "--frames", "4180",
                                          "--keys-file", "keys/win.keys", "--expect", "[wState] == 1"],
         r"stop budget_exhausted.*\n(.|\n)*ok=1"),
        ("nor in the human's game", [g, "run", "--session", "verifyp", "--at", "start", "--frames", "10627",
                                     "--keys-file", "keys/human.keys", "--expect", "[wState] == 1"],
         r"stop budget_exhausted.*\n(.|\n)*ok=1"),
    ]


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--gbcli", default="gbcli")
    opts = ap.parse_args()
    g = opts.gbcli
    for s in ("verify", "verifyc", "verifyp"):
        subprocess.run([g, "sessions", "rm", s, "--with-checkpoints", "--dir", PROJECT], capture_output=True)
    failed = 0
    t0 = time.time()
    for name, cmd, want in steps(g):
        if cmd[0] == g and cmd[1] != "build":
            # (build refuses --dir without --restore; it builds the cwd.)
            cmd = cmd + ["--dir", PROJECT]
        r = subprocess.run(cmd, capture_output=True, text=True, cwd=PROJECT)
        out = r.stdout + r.stderr
        ok = r.returncode == 0 and re.search(want, out, re.M)
        if name == "effects own channel 1":
            writers = re.findall(r"^audio_writer .*$", out, re.M)
            ok = ok and writers and all("SfxTick" in w for w in writers)
        print("%s %s" % ("PASS" if ok else "FAIL", name))
        if not ok:
            failed += 1
            sys.stdout.write("  $ %s\n" % " ".join(cmd))
            sys.stdout.write("".join("  " + line + "\n" for line in out.strip().splitlines()[-8:]))
        elif name in ("view rate", "the bot wins", "a lost human wins"):
            sys.stdout.write("".join("  " + line + "\n" for line in r.stdout.strip().splitlines()[-2:]))
    print("%s: %d failed, %.0f s" % ("ok" if not failed else "FAILED", failed, time.time() - t0))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
