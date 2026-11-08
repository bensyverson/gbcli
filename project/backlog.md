# Backlog

Work decided against, parked rather than dropped in silence. Nothing here is scheduled or blocking — active work lives in `job`. Read it before proposing something that sounds novel; it may already have been weighed and parked.

- **One dated H2 per item:** what it is, why it's parked, and *what would un-park it*.
- **Delete an item when it lands, or when it stops being plausible.** A long list is one nobody reads.
- **Something parked that turns out to be needed becomes a task in `job`** — move it, don't work it from here.

Format: one dated H2 per entry, a headline, then what it is, why it's parked, and the trigger that would revive it.

---

## 2026-11-06 — GBDK (C) support in `build`

What: `build` and `new` support for GBDK-2020 projects, so game logic can be written in C around hand-tuned assembly. Why parked: the target use case is extreme DMG performance, which is an assembly job; RGBDS comes first (user ruling, 2026-11-06). Un-park when: the RGBDS loop has shipped and a user wants C, or the agent eval shows agents need C for non-hot-path logic.

## 2026-11-06 — docboy as the emulator backend

What: wrap docboy (MIT, C++17) instead of SameBoy. Why parked: lost the bake-off. It may be slightly more accurate, but it is 3.5× slower (6.2× vs 21.8× realtime, measured), selects the model at compile time, keeps state private behind friend classes, has an unversioned state format, and its debugger blocks on `std::cin`. See project/2026-11-06-backend-evaluation.md. Un-park when: SameBoy shows a fidelity gap on a DMG edge our users hit that docboy gets right, or as a third opinion when a bespoke core and SameBoy disagree.

## 2026-11-06 — MiSTer, GateBoy and PyBoy as backends

What: the other candidates from the original brief. Why parked: MiSTer has a T80 CPU, a ModelSim-only simulator, no debugger and no accuracy advantage; GateBoy has a non-die-traced CPU, known test failures, MBC1 only and no license; PyBoy's save states are exact only at frame boundaries and it has no watchpoints. Details in project/2026-11-06-backend-evaluation.md. Un-park when: one of them publishes results beating SameBoy on the DMG tests we care about (GateBoy would also need a license).

## 2026-11-06 — CI on macOS and Linux

What: hosted CI building and testing on macOS arm64 and Linux x86_64 (job leaf `j2uBID`, cancelled). Why parked: the repo has no remote and one committer working on a Mac, so CI would only duplicate the local pre-commit hook (`scripts/hooks/pre-commit`: fmt, clippy, tests). The real gap it would close is Linux portability (SameBoy's `_GNU_SOURCE`, gcc vs clang), which matters only once someone builds on Linux. Un-park when: the repo gets a GitHub remote or goes public, a second contributor arrives, or the first Linux build is needed. A one-off Linux build in Docker can check portability before then.

## 2026-11-06 — More test-ROM suites in the corpus

What: the suites the corpus leaf weighed and left out — the Gambatte test suite (3,524 ROMs, GPL-2.0), wilbertpol's mooneye fork (GPL-3.0), rtc3test, MBC3 Tester, little-things-gb, TurtleTests, Scribbltests, the shootout's own `daid`/`cpp` tests, and docboy's CGB configs. Why parked: size and overlap for Gambatte, superseded for wilbertpol, mapper scope for the RTC/MBC3 tests, no license file for MBC3 Tester and TurtleTests, DMG first for docboy's CGB set. Details in project/2026-11-06-test-rom-corpus.md ▸ Rejected. Un-park when: a failure the current suites can't localize (Gambatte), the mapper leaf starts (rtc3test, MBC3 Tester), we want to reproduce shootout rows exactly (`daid`/`cpp`), or CGB fidelity work begins (docboy CGB).

## 2026-11-06 — Power-loss durability for the session bundle

What: fsync each written file before its rename and the directory after it, so a power loss leaves either the old head or the new one, never an empty or truncated file (leaf Z8Wbnn decided against it; design §1, the Z8Wbnn update, "Durability"). Why parked: bundle files are scratch states that a rerun or rebuild recreates, `~1` recovers from a damaged head, and on macOS `File::sync_all` is `F_FULLFSYNC`, a full device-cache flush (not measured here) against a 28 ms round trip with two or three writes per request. Un-park when: a user loses a named checkpoint they couldn't recreate, named checkpoints start being shared or committed as artifacts, or someone measures `F_FULLFSYNC` at under 10% of a round trip. Named checkpoints alone could be fsynced first: they are written rarely and are the one thing a rerun can't recreate.

## 2026-11-06 — Watches on cart RAM mirrors within a bank

What: a watch or breakpoint on cart RAM smaller than a bank also catching the in-bank mirrors of its byte — 2 KiB RAM repeats every $800, MBC2's 512 bytes every $200 — so a write watch on `$A000` sees a write at `$A800` (found by leaf fenCqJ; design §2.3, the fenCqJ update, "Still a gap"). Options: arm every in-bank alias (at most 16, for MBC2) and decide by backing byte in `debug/decide.rs`, or have the core report cart RAM hits at a canonical address. Why parked: Pan Docs lists the 2 KiB RAM size code ($01) as unused, MBC2 is rare in homebrew, and `--until` predicates already match by backing byte, so they are unaffected. Un-park when: a user's watch misses a mirrored write, or MBC2 support becomes a goal of the mapper leaf (`OU9rOL`).

## 2026-11-06 — AddressSanitizer over the test suite

**What:** run `cargo test --workspace` under AddressSanitizer (nightly: `RUSTFLAGS=-Zsanitizer=address`, with SameBoy's C built `-fsanitize=address` through the `cc` crate) to catch use-after-free and out-of-bounds accesses on both sides of the FFI boundary. All of the workspace's `unsafe` sits in `crates/sameboy` and `crates/sameboy-sys`; Miri can't follow calls into C, so this is the one check that covers it.

**Why parked:** it needs a nightly toolchain and a second build of SameBoy, and nothing has yet pointed at memory corruption. The cheap guards come first: `unsafe_code = "forbid"` in the crates without FFI and a required `SAFETY` comment on every `unsafe` block (user discussion, 2026-11-06).

**Un-park when:** a crash, a nondeterministic test or a state divergence we can't explain; before a first release (as part of the pre-release suite); or when the FFI surface grows substantially (a new shim family, a second backend).

## 2026-11-07 — The rumble motor's state through the core

What: a `Core` accessor (and a stop-record or `read` field) for whether an MBC5 rumble cart's motor is on, which SameBoy tracks as `rumble_strength`. Leaf OU9rOL tested the rumble bit only as far as the core interface reaches: bit 3 of $4000–$5FFF drives the motor and selects no RAM bank (`crates/sameboy/tests/it/mappers.rs`). Why parked: a game's motor writes are already visible to a watch on $4000–$5FFF, and no agent task has asked whether the motor is on. Un-park when: a rumble game's behavior needs asserting by state rather than by its writes, or a frontend wants to show it.

## 2026-11-07 — HuC3 and TPP1 clocks in `.sav` import

What: read SameBoy's HuC3 and TPP1 clock footers on `power-cycle --sav` without host time, as the MBC3 footer is read (`crates/sameboy-sys/shim/gbcli_battery.c`); today those carts' imports are refused as `unsupported` (`battery_save`), while export works. Why parked: neither is a must-have mapper (the cart research in the evaluation doc), and each footer is SameBoy's own struct, so the work is a parser per format. Un-park when: a homebrew project targets HuC3 or TPP1 and wants a save carried in from a file.

## 2026-11-07 — CGB images

> **Un-parked, 2026-11-07 (Ben):** a `--gbc` flag picks the target and dual mode is the goal. Research and a plan come first; see the job leaf "GBC images in dual mode".

What: `build` converting images for a CGB target: rgbgfx `-m -a -p -N 256,256` (flip dedup, attribute map, palettes, both VRAM banks), the `cgb_palettes` check (8 BG + 8 OBJ palettes of 4, naming the tiles that force the ninth) and 768 VRAM tiles (research doc, Recommendations (a)). Why parked: `build` has no CGB target (rgbfix runs `-v -p 0xFF`, no `-C`), so nothing would choose these flags; the image leaf (WdnjHI) built the DMG checks. Un-park when the build gains a CGB switch, or an agent eval targets the CGB.

## 2026-11-07 — 8x8 objects and an engine-budget reserve for images

What: sprite sheets of 8x8 objects (today `assets/sprites/` is 8x16 only, as the leaf specified), and a configurable reserve of tiles for an engine's font or HUD in the `vram_tiles` and `bg_tiles_map` checks. Why parked: both need a per-project setting, and `build` has no build file by design; neither has been asked for. Un-park when a user or the agent eval needs 8x8 sprites, or a starter grows a font that the image budget must leave room for (then decide where the setting lives: a directory such as `assets/sprites8/`, a name suffix, or a flag).

## 2026-11-07 — Image checks at runtime and a preview of a failing image

What: the objects-per-line check from OAM during a run ("frame 120, line 64: 12 objects; 2 will not draw", research doc, `obj_per_line`), and a preview drawn from gbcli's own analysis when an image fails its checks (today only converted images get one; a problem names its tiles instead). Why parked: the build-time checks answer the asset question; the runtime one belongs with the OAM views, and a failing image's tiles are already listed. Un-park when an agent misses a flicker the build-time check can't see (sprites from several sheets on one line), or asks to see a failing image.


## 2026-11-07 — Color art matched to named palettes, tile by tile

**What:** under `build --gbc`, a color background drawn in the colors of the named palettes in `assets/palettes.txt` gets, for each tile, the named palette that holds all its colors; tiles no named palette holds are packed automatically as today. That gives plain-gbcli backgrounds several named, swappable palettes (grass and roof, then `grass_night` and `roof_night`) with no extra file. A tile drawn only in colors several palettes share (plain white) matches more than one; it takes one by a rule (first in the file) with a warning, since the choice decides how it looks after a swap.

**Why parked:** Ben chose one named palette per indexed image for Stage 10 (plan question 4, [2026-11-07-gbc-images-plan.md](2026-11-07-gbc-images-plan.md)). This is an addition to it, with nothing to migrate. LoadHigh assigns palettes per tile kind itself and doesn't need it. A per-tile palette PNG was considered and dropped in favor of this.

**Un-park when:** a project (or the agent eval) needs a multi-palette background with named run-time swaps in plain gbcli, and writing the attribute map by hand is the stumble.
