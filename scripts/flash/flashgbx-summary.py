#!/usr/bin/env python3
"""Summarize FlashGBX's DMG flash-cart definitions for gbcli's flash profiles.

FlashGBX (github.com/lesserkuma/FlashGBX) is GPL-3.0, so gbcli vendors none of
its files: the profiles in crates/flash/src/profiles.rs are written by hand
from the facts this prints (unlock addresses and bytes, /WE pin, size, sector map),
each citing its file and commit. Re-run against a fresh checkout to check them.

Usage:
    python3 -I scripts/flash/flashgbx-summary.py <FlashGBX>/FlashGBX/config

Prints one line per AMD-command-set fc_DMG_*.txt: file, write pin, the first
two unlock cycles of single_write (address<-byte), whether the data lines are
D0/D1-swapped (the unlock bytes are A9/56 rather than AA/55), sector map, and
flash_commands_on_bank_1 and flash_size. Then counts of each unlock variant and pin.
"""

import collections
import json
import pathlib
import re
import sys


def load(path):
    text = path.read_text(encoding="utf-8")
    # FlashGBX writes hex literals, which JSON lacks.
    text = re.sub(r"\b0x([0-9A-Fa-f]+)\b", lambda m: str(int(m.group(1), 16)), text)
    return json.loads(text)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    root = pathlib.Path(sys.argv[1])
    variants = collections.Counter()
    pins = collections.Counter()
    for path in sorted(root.glob("fc_DMG_*.txt")):
        cfg = load(path)
        if cfg.get("command_set") != "AMD":
            continue
        write = cfg.get("commands", {}).get("single_write", [])
        unlock = [(a, d) for a, d in write[:2] if isinstance(a, int)]
        swapped = [d for _, d in unlock] == [0xA9, 0x56]
        unlock_s = " ".join(f"{a:04X}<-{d:02X}" for a, d in unlock)
        sectors = cfg.get("sector_size", "cfi" if cfg.get("sector_size_from_cfi") else "?")
        if isinstance(sectors, int):
            sectors = f"{sectors:#x}"
        elif isinstance(sectors, list):
            sectors = ",".join(f"{size:#x}x{count}" for size, count in sectors)
        pin = cfg.get("write_pin", "?")
        bank1 = cfg.get("flash_commands_on_bank_1", False)
        size = cfg.get("flash_size")
        size = f"{size:#x}" if isinstance(size, int) else "?"
        print(
            f"{path.name}\tpin={pin}\tunlock={unlock_s}\tswapped={int(swapped)}"
            f"\tsize={size}\tsectors={sectors}\tbank1={int(bank1)}"
        )
        variants[(tuple(a for a, _ in unlock), swapped)] += 1
        pins[pin] += 1
    print()
    for (addrs, swapped), n in variants.most_common():
        print(f"{n:3} unlock {'/'.join(f'{a:04X}' for a in addrs)} swapped={int(swapped)}")
    for pin, n in pins.most_common():
        print(f"{n:3} write_pin {pin}")


if __name__ == "__main__":
    main()
