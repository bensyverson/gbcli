#!/usr/bin/env python3
"""Classify every failing docboy case in a baseline, as project/2026-11-06-docboy-baseline.md does.

    python3 -I scripts/corpus/docboy_classes.py [tests/corpus/baseline-sameboy.json]

Prints each class with its count and exits 1 if a failing docboy case matches no
class (a new failure to explain) or a class lists a case that no longer fails
(an explanation to retire). Stdlib only.
"""

import json
import re
import sys

# (class, why, id pattern). Patterns match the part of the id after "docboy/roms/dmg/";
# the first match wins, so the specific ones come first.
CLASSES = [
    ("unexplained: after STOP", "the key wakes the core (job Jwo9sg), then the case writes its fail value; not investigated",
     r"^docboy/(cpu|ppu|dma)/interactive(_visual)?/(stop_read_(ly|stat)\w*|"
     r"dma_stop_during_transfer_timing_round1|stop_trigger_interrupt)\.gb$|"
     r"^docboy/mbc/mbc3/interactive/mbc3_rtc_gb_stopped_recover\.gb$"),
    ("harness: STOP, unexplained", "the key is held into STOP and released; the case times out",
     r"^docboy/cpu/interactive_visual/stop_immediate_exit_joypad_interrupt\.gb$"),
    ("harness: power-on OAM seed", "SameBoy's seed 0 leaves $FF at $FE35, which these tests read as 'OAM locked'",
     r"^gbmicrotest/boot/poweron_oam_\d+\.gb$"),
    ("reference: probable", "docboy's own reference; the shootout passes SameBoy and fails docboy on this ROM",
     r"^cpp/latch-rtc-test\.gb$"),
    ("SameBoy gap: Mealybug DMG-blob", "the nine mid-mode-3 failures of the shootout (emulator-made reference)",
     r"^mealybug/m3_\w+\.gb$"),
    ("SameBoy gap: GBMicrotest", "fails from both boot ROMs; GBMicrotest is checked on a DMG-CPU-08 board",
     r"^gbmicrotest/ppu/lyc1_int_if_edge_c\.gb$"),
    ("disputed: docboy PPU visual", "mid-mode-3 window/tile-data/SCY writes: a few pixels on one line differ, siblings pass",
     r"^docboy/ppu/visual/\w+\.gb$"),
    ("disputed: docboy OAM scan blocking", "OAM scan blocked by DMA or a CPU/IDU OAM write",
     r"^docboy/ppu/oam_scan_blocked_by_\w+\.gb$"),
    ("disputed: docboy OAM bug", "docboy's OAM-corruption cases (blargg's oam_bug passes on SameBoy)",
     r"^docboy/oam_bug/\w+\.gb$"),
    ("disputed: docboy APU ch1 sweep", "fail at every frame-sequencer phase; docboy fails SameSuite's channel_1_sweep_restart_2",
     r"^docboy/apu/ch1_period_sweep\w*\.gb$"),
    ("disputed: docboy other", "STAT write bug, TIMA reload, serial abort, MBC3 RTC latch, HuC3 command",
     r"^docboy/(stat_write_bug|timers|serial|mbc/mbc3|mbc/huc3)/\w+\.gb$"),
]
PREFIX = "docboy/roms/dmg/"


def main():
    path = sys.argv[1] if len(sys.argv) > 1 else "tests/corpus/baseline-sameboy.json"
    with open(path, encoding="utf-8") as f:
        results = [r for r in json.load(f)["results"] if r["suite"] == "docboy"]
    compiled = [(name, why, re.compile(p)) for name, why, p in CLASSES]
    counts = {name: 0 for name, _, _ in CLASSES}
    unclassified = []
    for r in results:
        if r["status"] != "fail":
            continue
        rel = re.sub(r"@\d+$", "", r["id"][len(PREFIX):])
        for name, _, pattern in compiled:
            if pattern.search(rel):
                counts[name] += 1
                break
        else:
            unclassified.append(f'{r["id"]}: {r.get("detail", "")}')
    statuses = {}
    for r in results:
        statuses[r["status"]] = statuses.get(r["status"], 0) + 1
    print(f"docboy: {len(results)} runs, " + ", ".join(f"{n} {s}" for s, n in sorted(statuses.items())))
    for name, why, _ in CLASSES:
        print(f"{counts[name]:5}  {name} ({why})")
    stale = [name for name, n in counts.items() if n == 0]
    for line in unclassified:
        print(f"unclassified: {line}")
    for name in stale:
        print(f"no longer fails: {name}")
    return 1 if unclassified or stale else 0


if __name__ == "__main__":
    sys.exit(main())
