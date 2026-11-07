# Test-ROM corpus: sources, licenses and the manifest

2026-11-06. **Decision: every suite is fetched by a pinned script, none is committed.** The corpus is 12 suites, 3,855 manifest rows over 4,728 files (190.9 MB), reproduced from four pinned sources and checked file by file with SHA-256. Blargg's ROMs have no license at all, so they could never be committed; the MIT suites could be, but one fetch path for everything keeps the repo code-only and gives CI one cache key. Leaf `ZGHaOP`; the runner that consumes this is stage 1 (`runner`).

**Flagged for the user:**

- **Blargg's test ROMs carry no license.** retrio/gb-test-roms at `c240dd7` has no LICENSE file (GitHub's API reports no license) and the readme.txt files say nothing about terms. Fetched only; never commit them.
- **GBMicrotest had no license at the commit c-sp built from** (`f3b55497c1`). Upstream added an MIT LICENSE at `463eb6b` (2024-04-26), so this suite comes from upstream's `bin/` at that commit instead of from c-sp. 509 of the 513 ROMs are byte-identical between the two builds (measured: SHA-256 of each file in both trees).
- **Some third-party ROMs inside docboy's suite have no license we could find**: `little-things-gb/whichboot.gb` and `double-halt-cancel.gb` (no per-folder LICENSE at pinobatch/little-things-gb master; its siblings `tellinglys` and `firstwhite` are zlib) and `zoomten/mbc30.gb` (source not identified). Their rows say `NOASSERTION`. Fetched only.

## Files

| Path | What |
|---|---|
| `tests/corpus/manifest.json` | Sources, suites (license, pinned commit), palettes, model vocabulary, and one row per test. Generated; do not hand-edit. |
| `tests/corpus/SHA256SUMS` | One `sha256sum`-format line per corpus file (ROMs, reference PNGs, `.sym` files, licenses, docboy's config). `shasum -a 256 -c` works on it from inside the corpus directory. |
| `scripts/corpus/fetch.py` | Downloads, checks archive pins, extracts, verifies every file. |
| `scripts/corpus/build_manifest.py` | Maintainer tool: the sources and per-suite facts live here; regenerates both files above from an extracted tree. |
| `scripts/corpus/shootout_failures.py` | Parses the gbdev shootout page; see [2026-11-06-shootout-failures.md](2026-11-06-shootout-failures.md). |
| `tests/corpus/fetched/` | The corpus (gitignored). |
| `tests/corpus/.downloads/` | Archive cache (gitignored). |

## Running the fetch

```
python3 -I scripts/corpus/fetch.py                       # whole corpus into tests/corpus/fetched/
python3 -I scripts/corpus/fetch.py --only mooneye        # one suite (repeatable)
python3 -I scripts/corpus/fetch.py --verify-only         # re-check an existing tree, no network
```

Python 3 standard library only. It exits non-zero on any missing, changed or unexpected file, printing one line per problem. Measured 2026-11-06 with `/usr/bin/time -p python3 -I scripts/corpus/fetch.py --dest <scratch>/fetched --cache <scratch>/downloads` from an empty directory: **4,728 files, 190,876,531 bytes, 15.6 s wall, all checksums OK**; a second run from the warm cache took 3.8 s. A mutation check (one byte appended to a ROM, one ROM deleted, one stray file added) made `--verify-only` report all three and exit 1.

Hosts it contacts: `github.com` and its release-asset host, `codeload.github.com`, `gekkio.fi`.

To move a pin: edit `SOURCES` or `SUITES` in `build_manifest.py`, run `python3 -I scripts/corpus/build_manifest.py --fetch` (downloads, checks archive pins only, rewrites manifest and sums), then run `fetch.py` from a clean directory to prove the new pins reproduce.

## Sources

| Source | Pin | Archive check | Supplies |
|---|---|---|---|
| [c-sp/game-boy-test-roms](https://github.com/c-sp/game-boy-test-roms) release v7.0 (2024-02-25) | `game-boy-test-roms-v7.0.zip` | SHA-256 `b9a9d7a1…3832c` | blargg, dmg-acid2, cgb-acid2, cgb-acid-hell, mealybug, SameSuite, AGE, Bully, Strikethrough; mooneye's two reference screenshots |
| Mooneye upstream CI build, [gekkio.fi](https://gekkio.fi/files/mooneye-test-suite/) | `mts-20260714-0944-31510e1.zip` (commit `31510e1`, 2026-07-14) | SHA-256 `18aa2946…7d3f2` | mooneye ROMs, `.sym` files, LICENSE |
| aappleby/GBMicrotest | commit `463eb6b` (codeload tarball) | none (see below) | 513 prebuilt ROMs from `bin/`, LICENSE |
| Docheinstein/docboy | commit `01e04d3` (codeload tarball, 15 MB) | none (see below) | `tests/config/dmg.json`, `tests/roms/dmg/`, `tests/results/dmg/`, LICENSE |

c-sp's release is the one prebuilt, versioned bundle with stable URLs and reference screenshots converted to a common palette; it is MIT (`LICENSE: 'MIT License / Copyright (c) 2020 Christoph Sprenger'`). Its per-suite howto files (copied into each suite directory) are where the run times and screenshot rules below come from. Mooneye comes from upstream instead because c-sp's copy is from 2022 (`8d742b9d55`) and Gekkio publishes his own CI builds; the 115 ROM names are identical between the two.

GitHub-generated tarballs (codeload) are not guaranteed byte-stable over time, so their archive hash is not pinned; every extracted file is. Two downloads on 2026-11-06 were identical (docboy `f25e6f64…`, GBMicrotest `7f85ca12…`).

**Python's `urllib` through the sandbox's filtering proxy ended downloads early without an error** (e.g. 409,600 of 423,779 bytes, repeatedly; `curl` through the same proxy did not). `fetch.py` checks `Content-Length` and resumes with an HTTP `Range` request; the archive and file checksums catch anything that slips through.

## Suites

License evidence was read from each upstream LICENSE file at the pinned commit (fetched from `raw.githubusercontent.com/<repo>/<commit>/LICENSE`), 2026-11-06.

| Suite | Rows | Size | Upstream pin | License (evidence) | Commit? | Decision |
|---|---|---|---|---|---|---|
| blargg | 65 | 2.1 MB | retrio/gb-test-roms `c240dd7` (via c-sp) | none: no LICENSE file | **no** | fetch, flagged |
| mooneye | 114 | 26 MB | Gekkio/mooneye-test-suite `31510e1` | MIT: "Copyright (c) 2014-2022 Joonas Javanainen … Permission is hereby granted" | yes | fetch |
| dmg-acid2 | 2 | 52 KB | mattcurrie/dmg-acid2 `8a98ce7` | MIT: "Copyright (c) 2020 Matt Currie" | yes | fetch |
| cgb-acid2 | 1 | 48 KB | mattcurrie/cgb-acid2 `04c6ca4` | MIT: "Copyright (c) 2020 Matt Currie" | yes | fetch |
| cgb-acid-hell | 1 | 44 KB | mattcurrie/cgb-acid-hell `107b7c5` | MIT: "Copyright (c) 2020 Matt Currie" | yes | fetch |
| mealybug | 77 | 1.4 MB | mattcurrie/mealybug-tearoom-tests `70e88fb` (= HEAD) | MIT: "Copyright (c) 2018 Matt Currie" | yes | fetch |
| samesuite | 78 | 2.5 MB | LIJI32/SameSuite `f71b4b3c37` | MIT at the pin: "Copyright (c) 2018 Lior Halphon"; HEAD says "X11 License … 2018-2023" | yes | fetch |
| age | 51 | 1.6 MB | c-sp/age-test-roms `cd3f654d13` | MIT: "Copyright (c) 2021 Christoph Sprenger" | yes | fetch |
| gbmicrotest | 513 | 18 MB | aappleby/GBMicrotest `463eb6b` (= HEAD) | MIT: "Copyright (c) 2024 aappleby" (absent at `f3b55497c1`) | yes, at `463eb6b` | fetch, flagged |
| bully | 2 | 68 KB | Ashiepaws/BullyGB `e24fe6f` | MIT: "Copyright (c) 2020 Optix" | yes | fetch |
| strikethrough | 2 | 48 KB | Ashiepaws/strikethrough.gb `7cd01bf` | MIT: "Copyright (c) 2021 Optix" | yes | fetch |
| docboy | 2,949 | 137 MB | Docheinstein/docboy `01e04d3` | MIT: "Copyright (c) 2025 Docheinstein"; its own ROMs come from docboy-test-suite, MIT "Copyright (c) 2026 Docheinstein"; third-party ROMs keep theirs (per row) | **no** (mixed) | fetch, flagged |

Sizes are `du -sk` of each suite directory in the fetched tree.

**Stale builds, checked:** c-sp built SameSuite at `f71b4b3c37` (2022) and AGE at `cd3f654d13` (2022); both upstreams moved on (SameSuite HEAD `f15645f`, 2025-10; AGE HEAD `1f5bc10`, 2025-02). `diff -rq` of the two source trees shows every SameSuite `.asm` changed; the two diffs sampled (`channel_1_sweep_restart_2.asm`, `include` and `Makefile`) are RGBDS syntax migrations (`EQU` → `DEF … EQU`, `X: MACRO` → `MACRO X`, `rgbasm -h -i` → `-I`), the AGE diff sampled likewise. Not every file was read, so "no behavior change" is a hypothesis. Neither upstream publishes built ROMs; rebuilding needs RGBDS and each repo's Makefile (RGBDS 1.0.4 is installed locally). **Not built**: the c-sp builds are what the shootout and docboy measure against, and a self-built ROM's hash would depend on our RGBDS version.

### Rejected

| Suite | Why | What would bring it in |
|---|---|---|
| Gambatte test suite (in c-sp v7.0: 3,524 ROMs) | GPL-2.0 (`COPYING` at pokemon-speedrunning/gambatte-core master); the largest suite by far, pass values encoded in file names, heavy overlap with docboy's and AGE's PPU/IRQ timing coverage | A failure the other suites can't localize, or the bespoke-core spike wanting a second timing oracle |
| Mooneye (wilbertpol fork) | GitHub reports GPL-3.0; a 2016 fork superseded by upstream mooneye | none expected |
| rtc3test, MBC3 Tester, little-things-gb, TurtleTests, Scribbltests (in c-sp) | Mapper/RTC and one-off tests; MBC3 Tester and TurtleTests have no LICENSE file at master; rtc3test is Unlicense, Scribbltests MIT | The mapper leaf (`mappers`) — docboy's suite already carries rtc3test and three MBC3 tests |
| The shootout's own `daid`/`cpp` tests | Only meaningful as shootout rows; MIT (shootout repo) | Wanting to reproduce shootout rows exactly |
| docboy's CGB configs (`cgb.json`, `cgb_dmg_mode.json`, `cgb_dmg_ext_mode.json`) | DMG first; the CGB trees are a further ~265 MB of ROMs | CGB fidelity work |

These are also in [backlog.md](backlog.md).

### docboy: all 2,949 cases, as docboy defines them

The brief offered all, a subset, or a reference. **All**, because docboy's own 2,355 cases (`roms/dmg/docboy/`) are the unique value and the remaining 594 are docboy's calibration of third-party ROMs, which is worth having as a second opinion beside ours. Each row points at its case by `{config, category, index}` in the fetched `docboy/config/dmg.json` instead of copying it, so there is one definition. A runner needs a docboy adapter: cases use `memory` expectations (address, value, optional fail value), `serial` bytes, `framebuffer` PNGs under `results/dmg/` with an optional `grey` palette, `check_at_tick`/`max_ticks` in T-cycles (4,194,304 Hz; default `max_ticks` 100,000,000 per `tests/testutils/testutils/runners.h`), `check_at_instruction`, `inputs` (joypad events at ticks), `limit_speed` (real-time RTC), and two-ROM serial cases (`rom2`/`framebuffer2`). docboy runs its tests **without a boot ROM** by default (`option(ENABLE_BOOTROM "Enable boot rom" OFF)`), so post-boot state matters. 81 GBMicrotest ROMs are absent from docboy's set; see below.

## The manifest

JSON, because the stage-1 runner is Rust and `serde_json` reads it with no further dependency; TOML would need a crate, and a CSV can't hold nested pass rules. Rows are one per line so diffs stay readable. `schema_version` is 1.

A row is **one test: a ROM plus, where references differ by model, one model variant** (`id` = path, plus `@variant`). One ROM can therefore have several rows: `dmg-acid2.gb@dmg` and `@cgb` compare against different screenshots. Fields:

- `suite`, `path` (relative to `tests/corpus/fetched/`), `license` (SPDX; `NOASSERTION` = none found).
- `models`: a list of `{family, revisions?}`. Families: `dmg mgb sgb sgb2 cgb agb`; revisions per `model_revisions`. Omitted revisions mean any. Derived from file-name suffixes (mooneye's convention: `dmgABC`, `G` = dmg+mgb, `S` = sgb+sgb2, `C` = cgb+agb, `A` = agb; AGE's: `dmgC`, `cgbBCE`, `ncmBCE` = a DMG ROM on CGB), from the reference screenshot's suffix, or from the suite's README where named below. A mooneye test with no suffix has no model restriction.
- `pass`, tagged by `kind`:
  - `fibonacci_registers` — B/C/D/E/H/L = 3/5/8/13/21/34 at `LD B,B`; `fail_value` 0x42 (mooneye README).
  - `serial_bytes` — the same six bytes on the link port (mooneye's alternative).
  - `serial_text` — link-port text contains `Passed` / `Failed` (blargg).
  - `blargg_memory` — `$A001-$A003` = `DE B0 61`, then `$A000` holds `$80` while running and the result code after (0 = pass), text at `$A004` (blargg's `dmg_sound/readme.txt`, "Output to memory").
  - `framebuffer` — compare the frame with `reference` under a named `palette` (`dmg_gray`, `cgb_rgb555`, `cgb_rgb555_shift` (added 2026-11-06 for `oam_bug@cgb`), `cgb_compat_csp`; defined in the manifest).
  - `memory_byte` — `$FF82` = 0x01 pass, 0xFF fail (GBMicrotest).
  - `docboy_case` — see above.
  - `manual` — no automatic rule; `reason` says why.
- `pass_alternatives`: other signals the same ROM emits (e.g. blargg serial beside its screenshot).
- `exit`: when to judge — `ld_b_b`, `emulated_seconds`, `emulated_frames`, `result_reported` (blargg singles: serial or `$A000` final result), `docboy_case`.
- `timeout_s`: upstream's stated bound; `null` where upstream states none (the runner picks a default).
- `boot_sensitive`: the test checks post-boot state (`boot_*`); SameBoy's open boot ROMs, not Nintendo's, will run them.
- `notes`.

### Pass rules by suite, and where each came from

- **blargg** — the multi-test ROMs use c-sp's screenshots at c-sp's emulated-second marks (cpu_instrs 55 s on DMG, instr_timing 1, mem_timing 3, mem_timing-2 4, halt_bug 2, dmg_sound 36, cgb_sound 37, oam_bug 21, interrupt_time 2), one row per model because a framebuffer row carries one palette. Single-test ROMs have no screenshots and use text output. Which output each ROM uses came from **a static byte scan, not a run**: `LD A,$81 / LDH (SC),A` (serial) appears in cpu_instrs, instr_timing and mem_timing; the `$DE`-to-`$A001` signature code appears in mem_timing-2, oam_bug, dmg_sound, cgb_sound, halt_bug and interrupt_time. halt_bug and interrupt_time declare no cartridge RAM in the header, so their `$A000` output may never be visible. `oam_bug@cgb` and `interrupt_time@dmg` reference the failing screen real hardware shows. Models: c-sp's table (cgb_sound fails on CGB-B, so `cgb` revisions C, E).
- **mooneye** — Fibonacci registers at `LD B,B`, serial alternative, 120 s bound (c-sp's howto citing mooneye-gb's runner). `manual-only/sprite_priority` and `madness/mgb_oam_dma_halt_sprites` are screenshot tests with c-sp's references. `utils/` (boot ROM dumpers) is not extracted.
- **acid** — screenshot at `LD B,B`: dmg-acid2 has DMG and CGB references; cgb-acid2 and cgb-acid-hell CGB only.
- **mealybug** — screenshot at `LD B,B`, one row per reference: `dmg_blob`, `dmg_b` (DMG-B; only two tests), `cgb_c`, `cgb_d`. **Upstream's references are "screenshots from my Game Boy emulator (which I believe to be correct)", checked against "blurry photos" of hardware** (README), and "DMG-blob" names no CPU revision. Four ROMs (`dma/hdma_*-C`, `mbc/mbc3_rtc`, `ppu/win_without_bg`) have no reference in the release: `manual`.
- **samesuite** — Fibonacci registers at `LD B,B` (c-sp). Models from suffixes, else from `apu/README.md`: CGB-E passes all APU tests, CGB-D all but `channel_1_sweep_restart_2`, CGB-C the channel 3 and non-channel tests, pre-CGB only `div_write_trigger` and `div_write_trigger_10`; `dma/` and `ppu/` use CGB-only hardware; `sgb/` is SGB.
- **age** — Fibonacci registers at `LD B,B`, or a screenshot per model where `<rom>-<models>.png` exists (c-sp). `speed-switch/caution/` carries upstream's WARNING.md.
- **gbmicrotest** — `$FF82` after two frames (`is_if_set_during_ime0` 0.38 s), per c-sp and upstream README. 432 ROMs are in docboy's set; of the other 81, 50 contain a write to `$FF82` (kept, noted "calibrate") and 31 contain none by the same static scan (`LDH ($82),A` or `LD ($FF82),A`), e.g. `temp.gb`, `minimal.gb`, `audio_testbench.gb`: `manual`.
- **bully, strikethrough** — screenshot after 0.5 s (c-sp). Bully fails on DMG-C hardware per c-sp ("Bad Echo RAM Reads").

### Open questions for the runner leaf

> **Answered, 2026-11-06 (leaf `rInSEQ`):** see [2026-11-06-sameboy-baseline.md](2026-11-06-sameboy-baseline.md) § Open questions. In short: SameBoy's open CGB boot ROM matches `cgb_compat_csp`; SameBoy matches neither Mealybug's DMG-blob nor its DMG-B references on the two tests that have both; every blargg output choice produced output except the `blargg_memory` alternatives of `halt_bug` and `interrupt_time` (no cart RAM, as suspected); 47 of the 50 uncalibrated GBMicrotest rows pass and 3 report failure, none "no result". Two manifest problems surfaced: `mgb_oam_dma_halt_sprites` never reaches its `ld_b_b` exit, and `oam_bug@cgb`'s reference uses `x << 3`, not `cgb_rgb555`.
>
> **Fixed, 2026-11-06 (leaf `5ewkYZ`):** both manifest problems. `mgb_oam_dma_halt_sprites` is judged after 1 emulated second, and `oam_bug@cgb` in a new `cgb_rgb555_shift` palette (x << 3); see [2026-11-06-docboy-baseline.md](2026-11-06-docboy-baseline.md) § Other findings.

- **Compatibility-mode colors.** A DMG ROM on CGB is colored by the boot ROM's palette choice. c-sp's references assume particular colors (`cgb_compat_csp`); SameBoy's open CGB boot ROM may choose differently. 53 rows depend on it.
- **DMG revision.** SameBoy emulates DMG-B only; mealybug's DMG references are "DMG-blob" and AGE's are DMG-C. Where a revision-specific reference exists only for another revision, a mismatch is not necessarily a bug.
- **Calibrate the blargg output choice** (static scan above) and the 50 uncalibrated GBMicrotest rows before trusting them.
