#!/usr/bin/env python3
"""Regenerate tests/corpus/manifest.json and tests/corpus/SHA256SUMS (maintainer tool).

Sources and per-suite facts (license, pinned commit, pass rules) are declared
below; rows are derived from the files actually extracted, so the manifest can
never list a ROM the fetch does not produce. To change a pin: edit SOURCES or
SUITES, then

    python3 -I scripts/corpus/build_manifest.py --fetch   # download unverified, rebuild both files
    python3 -I scripts/corpus/fetch.py                    # prove the new pins reproduce

Without --fetch it rebuilds from an existing tests/corpus/fetched tree.
Stdlib only.
"""

import argparse
import json
import os
import re
import shutil
import sys

sys.dont_write_bytecode = True  # keep __pycache__ out of scripts/corpus/
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import fetch  # noqa: E402  (sibling module; -I drops the script dir from sys.path)

SCHEMA_VERSION = 1
RETRIEVED = "2026-11-06"

# --- Sources: where bytes come from. ---------------------------------------

CSP = "c-sp-game-boy-test-roms-v7.0"
MTS = "mooneye-test-suite-mts-20260714-0944-31510e1"
GBMT = "gbmicrotest-463eb6b"
DOCBOY = "docboy-01e04d3"
HUGE = "hugetracker-3fedced"

# hUGETracker's sample songs used by gbcli-music's tests: the original compositions only
# (covers of commercial game music and songs of unclear authorship are left out).
HUGE_SONGS = [
    "Jester - FishNChips", "Reed - Funkabeer's Revenge", "SVL - UwU", "Arachno - A Sad Touch",
    "Maktone - Softworld", "MelonadeM - Boatship for Rent", "SVL - Yyna",
    "raphaelgoulart - The Murderous Funk Machine (intro)", "Coffee Bat - Blue Ocean",
    "Coffee Bat - Pilgrim's Peril Menu", "Coffee Bat - Wyrmhole", "Chavez - Monkeys on Mars",
    "FADE - Microplastics in the Air", "FADE - Strap in and Suit Up", "Tempest - Kekri",
]

SOURCES = [
    {
        "id": CSP,
        "url": "https://github.com/c-sp/game-boy-test-roms/releases/download/v7.0/game-boy-test-roms-v7.0.zip",
        "kind": "zip",
        "cache_name": "game-boy-test-roms-v7.0.zip",
        "archive_sha256": "b9a9d7a1075aa35a3d07c07c34974048672d8520dca9e07a50178f5860c3832c",
        "strip_components": 0,
        "note": "Prebuilt release of many suites with reference screenshots in a common palette; MIT (c-sp/game-boy-test-roms LICENSE).",
        "extract": [
            {"suite": "blargg", "from": "blargg/", "to": "blargg/"},
            {"suite": "dmg-acid2", "from": "dmg-acid2/", "to": "dmg-acid2/"},
            {"suite": "cgb-acid2", "from": "cgb-acid2/", "to": "cgb-acid2/"},
            {"suite": "cgb-acid-hell", "from": "cgb-acid-hell/", "to": "cgb-acid-hell/"},
            {"suite": "mealybug", "from": "mealybug-tearoom-tests/", "to": "mealybug/"},
            {"suite": "samesuite", "from": "same-suite/", "to": "samesuite/"},
            {"suite": "age", "from": "age-test-roms/", "to": "age/"},
            {"suite": "bully", "from": "bully/", "to": "bully/"},
            {"suite": "strikethrough", "from": "strikethrough/", "to": "strikethrough/"},
            # Reference screenshots for mooneye's two screenshot tests; upstream's build has none.
            {"suite": "mooneye", "from": "mooneye-test-suite/manual-only/sprite_priority-dmg.png",
             "to": "mooneye/manual-only/sprite_priority-dmg.png"},
            {"suite": "mooneye", "from": "mooneye-test-suite/manual-only/sprite_priority-cgb.png",
             "to": "mooneye/manual-only/sprite_priority-cgb.png"},
            {"suite": "mooneye", "from": "mooneye-test-suite/madness/mgb_oam_dma_halt_sprites_expected.png",
             "to": "mooneye/madness/mgb_oam_dma_halt_sprites_expected.png"},
        ],
    },
    {
        "id": MTS,
        "url": "https://gekkio.fi/files/mooneye-test-suite/mts-20260714-0944-31510e1/mts-20260714-0944-31510e1.zip",
        "kind": "zip",
        "cache_name": "mts-20260714-0944-31510e1.zip",
        "archive_sha256": "18aa29462dfe1fcd32a2cb3621733abdc72410074918b9245aa99a6920f7d3f2",
        "strip_components": 1,
        "note": "Upstream's own CI build of mooneye-test-suite at 31510e1 (2026-07-14), with .sym files and LICENSE.",
        "extract": [
            {"suite": "mooneye", "from": d, "to": "mooneye/" + d}
            for d in ("acceptance/", "emulator-only/", "madness/", "manual-only/", "misc/", "LICENSE")
        ],
    },
    {
        "id": GBMT,
        "url": "https://codeload.github.com/aappleby/GBMicrotest/tar.gz/463eb6bc0fe31d61781ef63060ad6d74090c0255",
        "kind": "tar.gz",
        "cache_name": "gbmicrotest-463eb6b.tar.gz",
        "archive_sha256": None,
        "strip_components": 1,
        "note": "GitHub-generated tarball: its bytes are not guaranteed stable, so only the extracted files are pinned. Prebuilt ROMs live in bin/.",
        "extract": [
            {"suite": "gbmicrotest", "from": "bin/", "to": "gbmicrotest/"},
            {"suite": "gbmicrotest", "from": "LICENSE", "to": "gbmicrotest/LICENSE"},
            {"suite": "gbmicrotest", "from": "README.md", "to": "gbmicrotest/README.md"},
        ],
    },
    {
        "id": DOCBOY,
        "url": "https://codeload.github.com/Docheinstein/docboy/tar.gz/01e04d3d9fcb4ea28d05f468771127b2f6bd3549",
        "kind": "tar.gz",
        "cache_name": "docboy-01e04d3.tar.gz",
        "archive_sha256": None,
        "strip_components": 1,
        "note": "GitHub-generated tarball (about 15 MB); only the extracted files are pinned. docboy commits its test ROMs and expected results.",
        "extract": [
            {"suite": "docboy", "from": "tests/roms/dmg/", "to": "docboy/roms/dmg/"},
            {"suite": "docboy", "from": "tests/results/dmg/", "to": "docboy/results/dmg/"},
            {"suite": "docboy", "from": "tests/config/dmg.json", "to": "docboy/config/dmg.json"},
            {"suite": "docboy", "from": "LICENSE", "to": "docboy/LICENSE"},
        ],
    },
    {
        "id": HUGE,
        "url": "https://codeload.github.com/SuperDisk/hUGETracker/tar.gz/3fedced393807bdc5d818fd39be59f6de0e84b60",
        "kind": "tar.gz",
        "cache_name": "hugetracker-3fedced.tar.gz",
        "archive_sha256": None,
        "strip_components": 1,
        "note": "GitHub-generated tarball; only the extracted files are pinned. Sample songs live in sample-songs/.",
        "extract": [
            {"suite": "hugetracker-samples", "from": f"sample-songs/{name}.uge", "to": f"hugetracker-samples/{name}.uge"}
            for name in HUGE_SONGS
        ] + [{"suite": "hugetracker-samples", "from": "README.md", "to": "hugetracker-samples/README.md"}],
    },
]

# --- Suites: provenance and license, verified 2026-11-06 from upstream LICENSE files. ---


def lic(spdx, url, evidence, commit_permitted):
    return {"spdx": spdx, "url": url, "evidence": evidence, "commit_permitted": commit_permitted}


SUITES = {
    "blargg": {
        "title": "Blargg's test ROMs (cpu_instrs, instr_timing, mem_timing, mem_timing-2, halt_bug, oam_bug, dmg_sound, cgb_sound, interrupt_time)",
        "upstream": "https://github.com/retrio/gb-test-roms", "commit": "c240dd7d700e5c0b00a7bbba52b53e4ee67b5f15",
        "source": CSP,
        "license": lic("NOASSERTION", None,
                       "No LICENSE file in retrio/gb-test-roms at c240dd7 (GitHub API reports no license); readme.txt files carry no license text.",
                       False),
    },
    "mooneye": {
        "title": "Mooneye Test Suite", "upstream": "https://github.com/Gekkio/mooneye-test-suite",
        "commit": "31510e12eea6286d36eea060a6adde755e1067aa", "source": MTS,
        "license": lic("MIT", "https://github.com/Gekkio/mooneye-test-suite/blob/31510e1/LICENSE",
                       "LICENSE: 'Copyright (c) 2014-2022 Joonas Javanainen ... Permission is hereby granted, free of charge'", True),
    },
    "dmg-acid2": {
        "title": "dmg-acid2", "upstream": "https://github.com/mattcurrie/dmg-acid2",
        "commit": "8a98ce731f96dde032ffb22ec36dc985d78fdb18", "source": CSP,
        "license": lic("MIT", "https://github.com/mattcurrie/dmg-acid2/blob/8a98ce731f/LICENSE",
                       "LICENSE: 'MIT License / Copyright (c) 2020 Matt Currie'", True),
    },
    "cgb-acid2": {
        "title": "cgb-acid2", "upstream": "https://github.com/mattcurrie/cgb-acid2",
        "commit": "04c6ca40cf75b6a93513fe596de4ab797efaff97", "source": CSP,
        "license": lic("MIT", "https://github.com/mattcurrie/cgb-acid2/blob/04c6ca40cf/LICENSE",
                       "LICENSE: 'MIT License / Copyright (c) 2020 Matt Currie'", True),
    },
    "cgb-acid-hell": {
        "title": "cgb-acid-hell", "upstream": "https://github.com/mattcurrie/cgb-acid-hell",
        "commit": "107b7c5a875f26473ebc1193e32c59394bdd3049", "source": CSP,
        "license": lic("MIT", "https://github.com/mattcurrie/cgb-acid-hell/blob/107b7c5a87/LICENSE",
                       "LICENSE: 'MIT License / Copyright (c) 2020 Matt Currie'", True),
    },
    "mealybug": {
        "title": "Mealybug Tearoom Tests", "upstream": "https://github.com/mattcurrie/mealybug-tearoom-tests",
        "commit": "70e88fb90b59d19dfbb9c3ac36c64105202bb1f4", "source": CSP,
        "license": lic("MIT", "https://github.com/mattcurrie/mealybug-tearoom-tests/blob/70e88fb90b/LICENSE",
                       "LICENSE: 'MIT License / Copyright (c) 2018 Matt Currie'", True),
    },
    "samesuite": {
        "title": "SameSuite", "upstream": "https://github.com/LIJI32/SameSuite",
        "commit": "f71b4b3c37", "source": CSP,
        "license": lic("MIT", "https://github.com/LIJI32/SameSuite/blob/f71b4b3c37/LICENSE",
                       "LICENSE at f71b4b3c37: 'MIT License / Copyright (c) 2018 Lior Halphon' (HEAD f15645f says 'X11 License / Copyright (c) 2018-2023')",
                       True),
    },
    "age": {
        "title": "AGE test ROMs", "upstream": "https://github.com/c-sp/age-test-roms",
        "commit": "cd3f654d13", "source": CSP,
        "license": lic("MIT", "https://github.com/c-sp/age-test-roms/blob/cd3f654d13/LICENSE",
                       "LICENSE: 'MIT License / Copyright (c) 2021 Christoph Sprenger'", True),
    },
    "gbmicrotest": {
        "title": "GBMicrotest (GateBoy's microtests)", "upstream": "https://github.com/aappleby/GBMicrotest",
        "commit": "463eb6bc0fe31d61781ef63060ad6d74090c0255", "source": GBMT,
        "license": lic("MIT", "https://github.com/aappleby/GBMicrotest/blob/463eb6b/LICENSE",
                       "LICENSE at 463eb6b (2024-04-26): 'MIT License / Copyright (c) 2024 aappleby'. The c-sp v7.0 build (f3b55497c1) predates it and has no LICENSE, which is why this suite is taken from upstream.",
                       True),
    },
    "bully": {
        "title": "BullyGB", "upstream": "https://github.com/Ashiepaws/BullyGB",
        "commit": "e24fe6fd7f3fbc6021e3ee7f0f29e9166fce5937", "source": CSP,
        "license": lic("MIT", "https://github.com/Ashiepaws/BullyGB/blob/e24fe6fd7f/LICENSE",
                       "LICENSE: 'MIT License / Copyright (c) 2020 Optix'", True),
    },
    "strikethrough": {
        "title": "strikethrough.gb", "upstream": "https://github.com/Ashiepaws/strikethrough.gb",
        "commit": "7cd01bf91665b85809379b71e655da751809994e", "source": CSP,
        "license": lic("MIT", "https://github.com/Ashiepaws/strikethrough.gb/blob/7cd01bf916/LICENSE",
                       "LICENSE: 'MIT License / Copyright (c) 2021 Optix'", True),
    },
    "docboy": {
        "title": "docboy's DMG test configuration (tests/config/dmg.json) with the ROMs and results it references",
        "upstream": "https://github.com/Docheinstein/docboy",
        "commit": "01e04d3d9fcb4ea28d05f468771127b2f6bd3549", "source": DOCBOY,
        "license": lic("MIT", "https://github.com/Docheinstein/docboy/blob/01e04d3/LICENSE",
                       "LICENSE: 'MIT License / Copyright (c) 2025 Docheinstein'; docboy's own ROMs come from docboy-test-suite (LICENSE: 'MIT License / Copyright (c) 2026 Docheinstein'). Third-party ROMs inside keep their upstream licenses: see each row.",
                       False),
    },
    "hugetracker-samples": {
        "title": "hUGETracker's sample songs (.uge, original compositions only), for gbcli-music's tests",
        "upstream": "https://github.com/SuperDisk/hUGETracker",
        "commit": "3fedced393807bdc5d818fd39be59f6de0e84b60", "source": HUGE,
        "license": lic("NOASSERTION", "https://github.com/SuperDisk/hUGETracker/blob/3fedced/README.md",
                       "README.md, under 'License': 'hUGETracker and hUGEDriver are dedicated to the public domain.' That names the tracker and the driver, not each composer's song; no LICENSE file, and no song carries a license of its own. User ruling 2026-11-07: fetch only, never commit.",
                       False),
    },
}

# Licenses of third-party directories inside docboy's tests/roms/dmg (row-level).
DOCBOY_DIR_LICENSE = {
    "docboy": "MIT", "blargg": "NOASSERTION", "mooneye": "MIT", "gbmicrotest": "MIT", "mealybug": "MIT",
    "mattcurrie": "MIT", "samesuite": "MIT", "age": "MIT", "hacktix": "MIT", "daid": "MIT", "cpp": "MIT",
    "aaaaaa123456789": "Unlicense", "tepples": "Zlib", "firstwhite": "Zlib",
    "little-things-gb": "NOASSERTION", "zoomten": "NOASSERTION",
}

PALETTES = {
    "dmg_gray": "Four DMG shades as 8-bit gray: $FF, $AA, $55, $00 (lightest to darkest), per the mealybug and c-sp READMEs.",
    "cgb_rgb555": "CGB colors: each 5-bit channel x becomes (x << 3) | (x >> 2).",
    "cgb_rgb555_shift": "CGB colors: each 5-bit channel x becomes x << 3, with no low-bit fill (white is $F8), as some references were captured.",
    "cgb_compat_csp": "A DMG-only ROM on CGB: the CGB boot ROM's compatibility palette, which c-sp lists as BG #000000 #0063C6 #7BFF31 #FFFFFF and OBJ #000000 #943939 #FF8484 #FFFFFF (8-bit RGB). Depends on which boot ROM runs.",
}

FAMILIES = ["dmg", "mgb", "sgb", "sgb2", "cgb", "agb"]
REVISIONS = {"dmg": ["0", "A", "B", "C"], "cgb": ["0", "A", "B", "C", "D", "E"], "agb": ["0", "A", "AE", "B", "BE"]}

FIB = {"kind": "fibonacci_registers", "b": 3, "c": 5, "d": 8, "e": 13, "h": 21, "l": 34, "fail_value": 0x42}
FIB_SERIAL = {"kind": "serial_bytes", "bytes": [3, 5, 8, 13, 21, 34], "fail_bytes": [0x42] * 6}
SERIAL_TEXT = {"kind": "serial_text", "pass_contains": "Passed", "fail_contains": "Failed"}
BLARGG_MEM = {"kind": "blargg_memory", "signature_address": 0xA001, "signature": [0xDE, 0xB0, 0x61],
              "status_address": 0xA000, "running_value": 0x80, "pass_value": 0x00, "text_address": 0xA004}
LD_B_B = {"kind": "ld_b_b"}


def secs(s):
    return {"kind": "emulated_seconds", "seconds": s}


def fam(name, revisions=None):
    m = {"family": name}
    if revisions:
        m["revisions"] = list(revisions)
    return m


def merge_models(models):
    out = {}
    for m in models:
        f = m["family"]
        if f in out and ("revisions" not in out[f] or "revisions" not in m):
            out[f].pop("revisions", None)
        elif f in out:
            out[f]["revisions"] = sorted(set(out[f]["revisions"]) | set(m["revisions"]), key=REVISIONS[f].index)
        else:
            out[f] = dict(m)
    return [out[f] for f in FAMILIES if f in out]


ALL = [fam(f) for f in FAMILIES]

# Mooneye's naming: https://github.com/Gekkio/mooneye-test-suite#test-naming (SameSuite follows it).
MOONEYE_TOKEN = re.compile(r"dmg[0ABC]*|mgb|sgb2|sgb|cgb[0ABCDE]*|agb|ags|GS|G|S|C|A")


def mooneye_models(stem):
    if "-" not in stem:
        return None
    suffix = stem.rsplit("-", 1)[1]
    tokens = MOONEYE_TOKEN.findall(suffix)
    if "".join(tokens) != suffix:
        return None
    models = []
    for t in tokens:
        if t.startswith("dmg"):
            models.append(fam("dmg", t[3:]))
        elif t.startswith("cgb"):
            models.append(fam("cgb", t[3:]))
        elif t in ("mgb", "sgb", "sgb2"):
            models.append(fam(t))
        elif t in ("agb", "ags", "A"):
            models.append(fam("agb"))
        elif t == "G":
            models += [fam("dmg"), fam("mgb")]
        elif t == "S":
            models += [fam("sgb"), fam("sgb2")]
        elif t == "GS":
            models += [fam("dmg"), fam("mgb"), fam("sgb"), fam("sgb2")]
        elif t == "C":
            models += [fam("cgb"), fam("agb")]
    return merge_models(models)


# AGE's naming: dmgC, cgbBCE, ncmBCE (a DMG ROM on CGB: "non-CGB mode"); ds and nocgb are variant tags.
AGE_TOKEN = re.compile(r"^(dmg|cgb|ncm)([0A-E]*)$")


def age_models(tokens):
    models = []
    for t in tokens:
        m = AGE_TOKEN.match(t)
        if m:
            models.append(fam("dmg" if m.group(1) == "dmg" else "cgb", m.group(2)))
    return merge_models(models)


def is_age_model_token(t):
    return bool(AGE_TOKEN.match(t))


def header_cgb_flag(path):
    with open(path, "rb") as f:
        f.seek(0x143)
        return f.read(1)[0]


def palette_for(model, rom_path):
    if model["family"] in ("dmg", "mgb", "sgb", "sgb2"):
        return "dmg_gray"
    return "cgb_rgb555" if header_cgb_flag(rom_path) & 0x80 else "cgb_compat_csp"


# References captured in another palette than their model's default, found by running them
# (project/2026-11-06-sameboy-baseline.md): the frame matched in shape and differed only in colors.
REFERENCE_PALETTE = {
    "blargg/oam_bug/oam_bug-cgb.png": "cgb_rgb555_shift",  # white is $F8F8F8
}


def fb(ref, model, rom_path):
    palette = REFERENCE_PALETTE.get(ref) or palette_for(model, rom_path)
    return {"kind": "framebuffer", "reference": ref, "palette": palette}


class Rows:
    def __init__(self, dest):
        self.dest = dest
        self.rows = []

    def abs(self, rel):
        return os.path.join(self.dest, rel)

    def add(self, suite, path, models, pass_rule, exit_rule, timeout_s=None, variant=None, alternatives=None,
            notes=None, license=None, boot_sensitive=False):
        if not os.path.exists(self.abs(path)):
            raise SystemExit(f"row path missing from fetched tree: {path}")
        ref = pass_rule.get("reference")
        if ref and not os.path.exists(self.abs(ref)):
            raise SystemExit(f"reference missing from fetched tree: {ref}")
        row = {"id": path + (f"@{variant}" if variant else ""), "suite": suite, "path": path, "models": models,
               "pass": pass_rule, "exit": exit_rule, "timeout_s": timeout_s,
               "license": license or SUITES[suite]["license"]["spdx"]}
        if alternatives:
            row["pass_alternatives"] = alternatives
        if boot_sensitive:
            row["boot_sensitive"] = True
        if notes:
            row["notes"] = notes
        self.rows.append(row)


def roms_under(dest, top):
    out = []
    for dirpath, _, files in os.walk(os.path.join(dest, top)):
        for fn in files:
            if fn.endswith((".gb", ".gbc")):
                out.append(os.path.relpath(os.path.join(dirpath, fn), dest).replace(os.sep, "/"))
    return sorted(out)


# --- Per-suite row builders. -------------------------------------------------

DMG, CGB = fam("dmg"), fam("cgb")

# Blargg: c-sp's howto (blargg/game-boy-test-roms-howto.md in the release) gives the run time per ROM and
# the screenshots. Serial vs $A000 output comes from a static byte scan of each ROM (see the corpus doc).
BLARGG_MULTI = [
    ("cpu_instrs/cpu_instrs.gb", [DMG, CGB], "cpu_instrs/cpu_instrs-dmg-cgb.png", 55, SERIAL_TEXT),
    ("instr_timing/instr_timing.gb", [DMG, CGB], "instr_timing/instr_timing-dmg-cgb.png", 1, SERIAL_TEXT),
    ("mem_timing/mem_timing.gb", [DMG, CGB], "mem_timing/mem_timing-dmg-cgb.png", 3, SERIAL_TEXT),
    ("mem_timing-2/mem_timing.gb", [DMG, CGB], "mem_timing-2/mem_timing-dmg-cgb.png", 4, BLARGG_MEM),
    ("halt_bug.gb", [DMG, CGB], "halt_bug-dmg-cgb.png", 2, BLARGG_MEM),
    ("dmg_sound/dmg_sound.gb", [DMG], "dmg_sound/dmg_sound-dmg.png", 36, BLARGG_MEM),
    ("cgb_sound/cgb_sound.gb", [fam("cgb", "CE")], "cgb_sound/cgb_sound-cgb.png", 37, BLARGG_MEM),
    ("oam_bug/oam_bug.gb", [DMG], "oam_bug/oam_bug-dmg.png", 21, BLARGG_MEM),
    ("oam_bug/oam_bug.gb", [CGB], "oam_bug/oam_bug-cgb.png", 21, None),
    ("interrupt_time/interrupt_time.gb", [CGB], "interrupt_time/interrupt_time-cgb.png", 2, BLARGG_MEM),
    ("interrupt_time/interrupt_time.gb", [DMG], "interrupt_time/interrupt_time-dmg.png", 2, None),
]
BLARGG_SINGLES = {
    "cpu_instrs/individual/": ([DMG, CGB], 55, SERIAL_TEXT),
    "mem_timing/individual/": ([DMG, CGB], 3, SERIAL_TEXT),
    "mem_timing-2/rom_singles/": ([DMG, CGB], 4, BLARGG_MEM),
    "dmg_sound/rom_singles/": ([DMG], 36, BLARGG_MEM),
    "cgb_sound/rom_singles/": ([fam("cgb", "CE")], 37, BLARGG_MEM),
    "oam_bug/rom_singles/": ([DMG], 21, BLARGG_MEM),
}
BLARGG_NOTES = {
    "oam_bug-cgb.png": "CGB has no OAM bug; the reference is the failing screen a real CGB shows.",
    "interrupt_time-dmg.png": "CGB-only test; the reference is the failing screen a real DMG shows (checksum 7F8F4AAF per c-sp).",
    "halt_bug-dmg-cgb.png": "Header declares no cartridge RAM, so the $A000 alternative may never appear; calibrate.",
    "interrupt_time-cgb.png": "Header declares no cartridge RAM, so the $A000 alternative may never appear; calibrate.",
    "cgb_sound-cgb.png": "Fails on CGB-B (test 3, code 04) per c-sp; loops forever on hardware, SameBoy terminates it.",
    "dmg_sound-dmg.png": "Loops forever on hardware; c-sp's screenshot was made with SameBoy.",
}


def build_blargg(r):
    # A framebuffer row carries one palette, so a reference shared by DMG and CGB becomes one row per family.
    for rel, models, ref, seconds, alt in BLARGG_MULTI:
        path, refp = "blargg/" + rel, "blargg/" + ref
        for m in models:
            r.add("blargg", path, [m], fb(refp, m, r.abs(path)), secs(seconds), timeout_s=seconds,
                  variant=m["family"], alternatives=[alt] if alt else None,
                  notes=BLARGG_NOTES.get(os.path.basename(ref)))
    covered = {"blargg/" + m[0] for m in BLARGG_MULTI}
    for path in roms_under(r.dest, "blargg"):
        if path in covered:
            continue
        inner = path[len("blargg/"):]
        prefix = next((p for p in BLARGG_SINGLES if inner.startswith(p)), None)
        if prefix is None:
            raise SystemExit(f"blargg ROM without a rule: {path}")
        models, seconds, rule = BLARGG_SINGLES[prefix]
        r.add("blargg", path, models, rule, {"kind": "result_reported"}, timeout_s=seconds)


def build_mooneye(r):
    for path in roms_under(r.dest, "mooneye"):
        inner = path[len("mooneye/"):]
        stem = os.path.splitext(os.path.basename(inner))[0]
        boot = "boot_" in stem
        models = mooneye_models(stem) or ALL
        if inner == "manual-only/sprite_priority.gb":
            for f, ref in (("dmg", "sprite_priority-dmg.png"), ("cgb", "sprite_priority-cgb.png")):
                m = fam(f)
                r.add("mooneye", path, [m], fb("mooneye/manual-only/" + ref, m, r.abs(path)), LD_B_B, 120,
                      variant=f, notes="Screenshot test; reference from c-sp v7.0 in the common palette.")
            continue
        if inner == "madness/mgb_oam_dma_halt_sprites.gb":
            m = fam("mgb")
            # It starts an OAM DMA at LY 144 and HALTs with no interrupt enabled, so it never
            # reaches LD B,B: the screen it leaves is the result.
            r.add("mooneye", path, [m], fb("mooneye/madness/mgb_oam_dma_halt_sprites_expected.png", m, r.abs(path)),
                  secs(1), notes="'madness' test; reference from c-sp v7.0 in upstream's palette, unverified. "
                                 "Halts forever (IE=0) after its DMA; judged after 1 emulated second.")
            continue
        r.add("mooneye", path, models, FIB, LD_B_B, 120, alternatives=[FIB_SERIAL], boot_sensitive=boot)


def build_acid(r):
    p = "dmg-acid2/dmg-acid2.gb"
    for f in ("dmg", "cgb"):
        r.add("dmg-acid2", p, [fam(f)], fb(f"dmg-acid2/dmg-acid2-{f}.png", fam(f), r.abs(p)), LD_B_B, variant=f)
    for suite in ("cgb-acid2", "cgb-acid-hell"):
        p = f"{suite}/{suite}.gbc"
        r.add(suite, p, [CGB], fb(f"{suite}/{suite}.png", CGB, r.abs(p)), LD_B_B)


MEALYBUG_REF = {"dmg_blob": fam("dmg"), "dmg_b": fam("dmg", "B"), "cgb_c": fam("cgb", "C"), "cgb_d": fam("cgb", "D")}


def build_mealybug(r):
    for path in roms_under(r.dest, "mealybug"):
        base = path[:-3]
        refs = [(k, m) for k, m in MEALYBUG_REF.items() if os.path.exists(r.abs(f"{base}_{k}.png"))]
        if not refs:
            models = [fam("cgb", "C")] if path.endswith("-C.gb") else ([DMG, CGB])
            r.add("mealybug", path, models, {"kind": "manual", "reason": "no reference screenshot in the pinned release"},
                  LD_B_B)
            continue
        for k, m in refs:
            note = "DMG-blob reference: upstream does not name the CPU revision." if k == "dmg_blob" else None
            r.add("mealybug", path, [m], fb(f"{base}_{k}.png", m, r.abs(path)), LD_B_B, variant=k, notes=(
                (note + " " if note else "") + "References come from the author's emulator, checked against photos of hardware (upstream README)."))


def samesuite_models(inner, stem):
    explicit = mooneye_models(stem)
    if explicit:
        return explicit
    top = inner.split("/")[0]
    if top == "sgb":
        return [fam("sgb"), fam("sgb2")]
    if top in ("dma", "ppu"):
        return [CGB]
    if top == "interrupt":
        return [DMG, CGB]
    # apu/README.md: CGB-E passes all; CGB-D all but channel_1_sweep_restart_2; CGB-C channel 3 and
    # non-channel tests; pre-CGB only div_write_trigger and div_write_trigger_10.
    if stem == "channel_1_sweep_restart_2":
        return [fam("cgb", "E")]
    if stem in ("div_write_trigger", "div_write_trigger_10"):
        return [DMG, fam("cgb", "CDE")]
    if stem.startswith("channel_3_") or not stem.startswith("channel_"):
        return [fam("cgb", "CDE")]
    return [fam("cgb", "DE")]


def build_samesuite(r):
    for path in roms_under(r.dest, "samesuite"):
        inner = path[len("samesuite/"):]
        stem = os.path.splitext(os.path.basename(inner))[0]
        r.add("samesuite", path, samesuite_models(inner, stem), FIB, LD_B_B)


def build_age(r):
    pngs = {}
    for dirpath, _, files in os.walk(r.abs("age")):
        for fn in files:
            if fn.endswith(".png"):
                rel = os.path.relpath(os.path.join(dirpath, fn), r.dest).replace(os.sep, "/")
                stem, token = rel[:-4].rsplit("-", 1)
                if not is_age_model_token(token):
                    raise SystemExit(f"AGE screenshot without a model token: {rel}")
                pngs.setdefault(stem, []).append((token, rel))
    for path in roms_under(r.dest, "age"):
        stem = path[:-3]
        if stem in pngs:
            for token, ref in sorted(pngs[stem]):
                models = age_models([token])
                r.add("age", path, models, fb(ref, models[0], r.abs(path)), LD_B_B, variant=token)
        else:
            tokens = os.path.basename(stem).split("-")
            models = age_models(tokens)
            if not models:
                raise SystemExit(f"AGE ROM without model tokens: {path}")
            note = "Under speed-switch/caution: upstream WARNING.md applies." if "/caution/" in path else None
            r.add("age", path, models, FIB, LD_B_B, notes=note)


def writes_ff82(path):
    """Static scan: does the ROM contain LDH ($FF82),A or LD ($FF82),A? A heuristic, not a disassembly."""
    with open(path, "rb") as f:
        b = f.read()
    return b"\xe0\x82" in b or b"\xea\x82\xff" in b


def build_gbmicrotest(r):
    rule = {"kind": "memory_byte", "address": 0xFF82, "pass_value": 0x01, "fail_value": 0xFF}
    with open(r.abs("docboy/config/dmg.json"), encoding="utf-8") as f:
        docboy_set = {os.path.basename(c["rom"]) for cases in json.load(f).values() for c in cases
                      if c["rom"].startswith("gbmicrotest/")}
    for path in roms_under(r.dest, "gbmicrotest"):
        name = os.path.basename(path)
        slow = name == "is_if_set_during_ime0.gb"
        exit_rule = secs(0.38) if slow else {"kind": "emulated_frames", "frames": 2}
        note = "Checked on a DMG-CPU-08 board (CPU B or C) per upstream README and c-sp."
        if name in docboy_set:
            r.add("gbmicrotest", path, [DMG], rule, exit_rule, notes=note)
        elif writes_ff82(r.abs(path)):
            r.add("gbmicrotest", path, [DMG], rule, exit_rule,
                  notes=note + " Not in docboy's gbmicrotest set (reason not recorded upstream); calibrate before trusting.")
        else:
            r.add("gbmicrotest", path, [DMG],
                  {"kind": "manual", "reason": "no write to $FF82 found by a static scan: likely a testbench or utility, not a pass/fail test"},
                  exit_rule, notes=note + " Not in docboy's gbmicrotest set.")


def build_hacktix(r):
    p = "bully/bully.gb"
    for f in ("dmg", "cgb"):
        note = "Fails on DMG-C hardware with 'Bad Echo RAM Reads' per c-sp; CGB passes." if f == "dmg" else None
        r.add("bully", p, [fam(f)], fb("bully/bully.png", fam(f), r.abs(p)), secs(0.5), 0.5, variant=f, notes=note)
    p = "strikethrough/strikethrough.gb"
    for f in ("dmg", "cgb"):
        r.add("strikethrough", p, [fam(f)], fb(f"strikethrough/strikethrough-{f}.png", fam(f), r.abs(p)), secs(0.5), 0.5,
              variant=f)


def build_docboy(r):
    with open(r.abs("docboy/config/dmg.json"), encoding="utf-8") as f:
        config = json.load(f)
    seen = {}
    for category, cases in config.items():
        for index, case in enumerate(cases):
            path = "docboy/roms/dmg/" + case["rom"]
            n = seen[path] = seen.get(path, 0) + 1
            top = case["rom"].split("/")[0]
            if top not in DOCBOY_DIR_LICENSE:
                raise SystemExit(f"docboy ROM directory without a license entry: {top}")
            for extra in ("rom2",):
                if extra in case and not os.path.exists(r.abs("docboy/roms/dmg/" + case[extra])):
                    raise SystemExit(f"docboy second ROM missing: {case[extra]}")
            for ref_key in ("framebuffer", "framebuffer2"):
                if ref_key in case and not os.path.exists(r.abs("docboy/results/dmg/" + case[ref_key])):
                    raise SystemExit(f"docboy reference missing: {case[ref_key]}")
            r.add("docboy", path, [DMG], {"kind": "docboy_case", "config": "docboy/config/dmg.json", "category": category, "index": index},
                  {"kind": "docboy_case"}, variant=str(n) if n > 1 else None, license=DOCBOY_DIR_LICENSE[top])


BUILDERS = [build_blargg, build_mooneye, build_acid, build_mealybug, build_samesuite, build_age,
            build_gbmicrotest, build_hacktix, build_docboy]


# --- Output. -----------------------------------------------------------------

def write_manifest(path, manifest):
    rows = manifest.pop("rows")
    head = json.dumps(manifest, indent=2)
    body = ",\n".join("    " + json.dumps(row, separators=(", ", ": ")) for row in rows)
    with open(path, "w", encoding="utf-8") as f:
        f.write(head[:-2] + ',\n  "rows": [\n' + body + "\n  ]\n}\n")


def write_sums(path, dest):
    lines = []
    for suite in SUITES:
        for dirpath, _, files in os.walk(os.path.join(dest, suite)):
            for fn in files:
                full = os.path.join(dirpath, fn)
                rel = os.path.relpath(full, dest).replace(os.sep, "/")
                lines.append(f"{fetch.sha256_file(full)}  {rel}")
    lines.sort(key=lambda l: l.split("  ", 1)[1])
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")
    return len(lines)


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--fetch", action="store_true", help="download and extract first, checking archive pins only")
    ap.add_argument("--dest", default=fetch.DEFAULT_DEST)
    ap.add_argument("--cache", default=fetch.DEFAULT_CACHE)
    args = ap.parse_args()

    if args.fetch:
        for suite in SUITES:
            shutil.rmtree(os.path.join(args.dest, suite), ignore_errors=True)
        for source in SOURCES:
            path = fetch.download(source, args.cache)
            print(f"{source['id']}: {fetch.extract(source, path, args.dest, list(SUITES))} files", file=sys.stderr)

    r = Rows(args.dest)
    for build in BUILDERS:
        build(r)
    suites = {}
    for name, s in SUITES.items():
        suites[name] = dict(s, rows=sum(1 for row in r.rows if row["suite"] == name), dir=name + "/")
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "generated_by": "scripts/corpus/build_manifest.py",
        "retrieved": RETRIEVED,
        "doc": "project/2026-11-06-test-rom-corpus.md",
        "paths_relative_to": "tests/corpus/fetched/",
        "checksums": "tests/corpus/SHA256SUMS",
        "model_families": FAMILIES,
        "model_revisions": REVISIONS,
        "palettes": PALETTES,
        "sources": SOURCES,
        "suites": suites,
        "rows": r.rows,
    }
    out_dir = os.path.dirname(fetch.MANIFEST)
    os.makedirs(out_dir, exist_ok=True)
    write_manifest(fetch.MANIFEST, manifest)
    n = write_sums(fetch.SUMS, args.dest)
    print(f"wrote {len(r.rows)} rows across {len(SUITES)} suites; {n} checksummed files", file=sys.stderr)


if __name__ == "__main__":
    main()
