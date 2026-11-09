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

## 2026-11-07 — 8x8 objects and an engine-budget reserve for images

What: sprite sheets of 8x8 objects (today `assets/sprites/` is 8x16 only, as the leaf specified), and a configurable reserve of tiles for an engine's font or HUD in the `vram_tiles` and `bg_tiles_map` checks. Why parked: both need a per-project setting, and `build` has no build file by design; neither has been asked for. Un-park when a user or the agent eval needs 8x8 sprites, or a starter grows a font that the image budget must leave room for (then decide where the setting lives: a directory such as `assets/sprites8/`, a name suffix, or a flag).

> **2026-11-09 (leaf RDlaLo): the starter now has a font.** `gbcli new` draws a line of text into a fixed 20-tile line buffer, tiles 236-255, which the image checks don't count; `gbcli help fonts` and the starter document it. Per design question 9 the reserve stays parked: the next check that miscounts because of it un-parks this.

## 2026-11-07 — Image checks at runtime and a preview of a failing image

What: the objects-per-line check from OAM during a run ("frame 120, line 64: 12 objects; 2 will not draw", research doc, `obj_per_line`), and a preview drawn from gbcli's own analysis when an image fails its checks (today only converted images get one; a problem names its tiles instead). Why parked: the build-time checks answer the asset question; the runtime one belongs with the OAM views, and a failing image's tiles are already listed. Un-park when an agent misses a flicker the build-time check can't see (sprites from several sheets on one line), or asks to see a failing image.


## 2026-11-07 — Color art matched to named palettes, tile by tile

**What:** under `build --gbc`, a color background drawn in the colors of the named palettes in `assets/palettes.txt` gets, for each tile, the named palette that holds all its colors; tiles no named palette holds are packed automatically as today. That gives plain-gbcli backgrounds several named, swappable palettes (grass and roof, then `grass_night` and `roof_night`) with no extra file. A tile drawn only in colors several palettes share (plain white) matches more than one; it takes one by a rule (first in the file) with a warning, since the choice decides how it looks after a swap.

**Why parked:** Ben chose one named palette per indexed image for Stage 10 (plan question 4, [2026-11-07-gbc-images-plan.md](2026-11-07-gbc-images-plan.md)). This is an addition to it, with nothing to migrate. LoadHigh assigns palettes per tile kind itself and doesn't need it. A per-tile palette PNG was considered and dropped in favor of this.

**Un-park when:** a project (or the agent eval) needs a multi-palette background with named run-time swaps in plain gbcli, and writing the attribute map by hand is the stumble.

> **Extended, 2026-11-07 (leaf YI5gQ6): this is the backlog's one entry for per-tile palette assignment in plain gbcli**, which Stage 10 left out (plan question 4: an indexed image takes one named palette). The other way weighed, a second, palette-index PNG per background, stays dropped (above). Until it lands, a program colors tiles differently by writing the attribute map itself, as LoadHigh does from tile kinds, or by drawing the background as color art, whose palettes are packed per tile but not named.

## 2026-11-07 — GBC-only builds (`rgbfix -C`)

**What:** a build for the GBC alone (header $C0), which a DMG refuses to run, and which is what the GBC-only features below need, since a dual ROM must still draw on a DMG.

**Why parked:** Stage 10's goal is dual mode (Ben, 2026-11-07, plan question 7, [2026-11-07-gbc-images-plan.md](2026-11-07-gbc-images-plan.md)): one ROM for both machines. A GBC-only target means a second meaning for every image check (no DMG version, so no derived shades, no `.dmg.png`, no `dmg_palette` for color art) and a second starter path. `gbcli help rgbds` says `-C` isn't offered.

**Un-park when:** a project wants GBC-only art or speed (double speed, HDMA, VRAM bank 1) and accepts losing the DMG, or a game's art can't fit dual mode's limits.

## 2026-11-07 — VRAM bank 1 tiles and BG flips

**What:** under the GBC, a background's tiles in both VRAM banks (768 tiles, not 384) and tiles deduplicated by X/Y flips, the bank and flip bits set in the attribute map (rgbgfx's `-m` and `-N 256,256`; [2026-11-07-asset-pipeline-research.md](2026-11-07-asset-pipeline-research.md), Recommendations (a)). This entry and the one above replace the CGB-images entry that Stage 10 un-parked.

**Why parked:** in dual mode the one tilemap must draw on the DMG, whose map has no bank or flip bits and whose VRAM has one bank, so a tile kept only flipped, or only in bank 1, has no DMG version (plan, "Flips", and question 7). Sheets already flip on both machines, through OAM. So both features need a GBC-only build first.

**Un-park when:** GBC-only builds land (the entry above), or a dual project runs out of `vram_tiles` or `bg_tiles_map` with art whose tiles are mirrors of each other.

## 2026-11-07 — Per-scene palette budgets

**What:** counting the GBC palettes resident at one time across the backgrounds and sheets a scene shows together, rather than per background (8 BG palettes) and across every sheet (8 OBJ palettes) as `gbc_palettes` does now.

**Why parked:** gbcli can't see scenes: which images are on screen together is the program's choice, and the build has no file to say it (plan question 5: the build counts what it places, the program owns the rest, and palettes loaded only for swaps aren't counted). LoadHigh, which knows its scenes, counts its own. The cost: a program can load more palettes than a scene has slots for, or swap one into a slot another image still uses, and no check says so.

**Un-park when:** plain gbcli gains a notion of a scene (a manifest or a build file), or an agent ships a scene whose palettes overflow and no check caught it.

## 2026-11-07 — `equiv --after N`: compare only from frame N

**What:** a flag that runs both sides from their starts but compares only from frame N on, for builds whose RAM layout differs. A checkpoint can't start both sides once a build moves a RAM symbol: `equiv --at` is refused as `rom_changed`, and `--allow-rom-change` would load side A's layout into side B.

**Why parked:** agent eval run 3 hit the case (leaf k22J86). The fix that shipped (b486fc9) names a never-written byte and says to initialize it before the first vblank, and that's the right fix for a program. Comparing later would hide a real difference in the first N frames.

**Un-park when:** a program can't initialize the byte early (a boot-time race, a library that owns the RAM), or an eval agent stumbles on it again after the remedy.

## 2026-11-07 — A spectrogram scaled to its own peak

**What:** color a short clip's spectrogram against its own peak instead of the fixed −90 dBFS floor, so a quiet 20-frame blip doesn't render nearly black.

**Why parked:** eval runs 1-3 (leaf k22J86) found the decoded `--audio-log` and the piano roll more useful than the spectrogram, and `--scale` now reaches it. Normalizing changes what the colors mean across captures, so two spectrograms could no longer be compared by eye.

Eval run 5 asked for axis labels or a legend on the image too; `help music` now names the axes and scale in text, but the PNG carries none.

**Un-park when:** a sound task needs the spectrogram (noise, timbre, a mix where the log's note names don't tell enough) and an agent can't read it.

## 2026-11-08 — A bespoke Rust emulator core

**What:** a Rust core implementing the same core trait as the SameBoy backend: DMG first, no APU, judged by blargg, mooneye, dmg-acid2 and lockstep with SameBoy. It would answer what the wrapper reports as *unsupported*: side-effect-free reads, sub-instruction stepping, saved cycle counters. The spike plan is in [2026-11-06-backend-evaluation.md](2026-11-06-backend-evaluation.md) ▸ The bespoke core question.

**Why parked:** Ben's ruling, 2026-11-08, on the evidence in [2026-11-07-bespoke-core-decision-brief.md](2026-11-07-bespoke-core-decision-brief.md). Across five eval runs the gap ledger stayed empty, no stumble came from a core limit, and SameBoy's only DMG failures are mid-mode-3 LCDC and SCY writes judged against another emulator's screenshots. The spike costs about 6 agent sessions.

**Un-park when** any one holds:
- an agent's real work (an eval run or a LoadHigh session) records an `unsupported` that blocks its task, and no adapter work can serve it;
- SameBoy disagrees with hardware truth (a hardware-verified test ROM or a hardware capture) in a way that affects programs gbcli users write, and upstream won't take a fix;
- an eval task class that needs mid-mode-3 effects is added, and it fails on SameBoy where docboy passes.

> **Update, 2026-11-09 (eighteenth session):** gbcli now carries two observation-only SameBoy patches (`layer-identity`, `mode-blocked-writes`), kept as files under `crates/sameboy-sys/patches/` with a hashed patch set, which is the fork: no separate repository is needed while the patches stay small and observation-only. Neither un-park condition moved: the ignored-write count needed only a callback in SameBoy's own blocked branches, and the unknown PPU-mode window late in a batched mode 3 is narrowed by answering what the core does know (it is mode 0 or 3; leaf AjUdpW). **The web ruling adds an argument this entry didn't weigh:** SameBoy is C, so a `wasm32` build needs a C toolchain for that target (Emscripten or wasi-sdk), while a pure-Rust core builds with Cargo alone. **Un-park also when:** the web spike ([2026-11-09-web-target.md](2026-11-09-web-target.md)) finds SameBoy can't be built or run acceptably for `wasm32`.

## 2026-11-08 — Small run and view refinements from the example games

**What:** two items from [2026-11-08-examples-dx-review.md](2026-11-08-examples-dx-review.md), item 12. First, separate default output names for the DMG and GBC builds (both write `build/NAME.gb` today). Second, `dropped_by_line_limit` saying which lines dropped an object.

> **Update, 2026-11-09:** a third item, keys-file offsets from a time base, was un-parked: both D and E hit it, and Ben accepted `run --keys-since BASE` ([2026-11-09-examples-dx-review-de.md](2026-11-09-examples-dx-review-de.md), question 4). The rogue also built `--gbc` over its plain build's `build/rogue.gb` and ran the wrong ROM for 20 minutes. Ben accepted `model=`/`cgb=` in `show` and `sessions` for that, and then un-parked separate default output names as well: `build --gbc` writes `build/gbc/NAME.gbc`, with its `.sym` and `.map` beside it (Ben, 2026-11-09, "It already would have saved some time"). It gets its own folder because symbol files are found by swapping the ROM's extension, in gbcli and in emulators, so `NAME.gb` and `NAME.gbc` side by side would share one `NAME.sym`. Only `dropped_by_line_limit` naming its lines stays parked here.

**Why parked:** each cost an agent five minutes or less, each had a clean workaround (measure from boot, `--out`, read the screenshot), and Ben accepted the review's call to backlog them.

**Un-park when:** a later example game or LoadHigh hits one of them again.

## 2026-11-08 — Placing generated data in a ROM bank

**What:** a way to keep generated images, songs or tables in a chosen bank (or ROM0), so an MBC game can read them every frame without bank switching ([2026-11-08-examples-dx-review.md](2026-11-08-examples-dx-review.md), item 8).

**Why parked:** the racer hit 32 KB under `--gbc` and shrank its data instead; no game has needed MBC yet. A help note that `--gbc` color art roughly doubles image ROM covers the surprise for now.

**Un-park when:** an example game or LoadHigh needs more than 32 KB with generated data read every frame.

## 2026-11-08 — STAT late in a batched mode 3, from a scratch copy

**What:** read STAT (and give a raster write its mode) in the one window SameBoy's display state can't answer: late in a mode 3 batched by its 300-dot bound, on a line with objects or the window and no HBlank interrupt (the window was missing here until 2026-11-09, leaf nin31Q; `show`'s `mode=` says `unknown` there since that leaf). The way to do it is to run the display on a scratch copy of the state, never the live instance, or, for a write to a PPU register only, to sync early in the write callback, which the write does a moment later anyway ([2026-11-08-ppu-mode-spike.md](2026-11-08-ppu-mode-spike.md)).

**Why parked:** neither example game hit the window on a PPU write: the racer's 162,660 scroll writes over 900 frames and the shooter's 8,973 split writes all had a mode. Only 95 of the shooter's sound writes did. A condition that hits it now says `condition_error`, so nothing is hidden. Both ways cost real care: a scratch copy is a second instance or a raw copy of SameBoy's state, and the early sync must be shown to leave the state byte-identical.

**Un-park when:** an agent's real check hits `condition_error` or a missing `mode=` on a write it needs to judge.

## 2026-11-08 — Tile and frame names as program constants

What: let pixel text (or a sheet's name) name its tiles and frames so `images.inc` defines constants for them (`FONT_TILE_A`, `HERO_FRAME_STEP`), instead of programs hard-coding tile numbers as the shooter's `include/game.inc` did. Why parked: the pixel text design's open question 7, which Ben ruled on 2026-11-08 ([2026-11-08-pixel-text-design.md](2026-11-08-pixel-text-design.md)): it changes `images.inc`, the LoadHigh seam, and the dedupe question (a merged tile would have two names), and only one game needed it. Un-park when: a third game hard-codes tile or frame numbers again.

## 2026-11-09 — Embedded scripting for input bots (QuickJS)

**What:** a JavaScript interpreter inside gbcli, so an agent's closed-loop input bot (read RAM, decide, press keys, every frame) runs in the session's own loop instead of as an outside Python script driving `gbcli run` call by call (the shooter's bot made 1,150 calls, four frames each). Scripts would see only the emulator: reads, key presses, run-until, never the filesystem or network, so a bot run stays deterministic and replays as a key file.

**Ben's lean (2026-11-09):** QuickJS (through the `rquickjs` crate), one engine on every platform; not a bot client over `serve` ("it could be harder to embed a `serve` bot in an agent loop"), and not JavaScriptCore (macOS only, so Linux would need a second engine, which threatens determinism). The dependency still needs Ben's go-ahead when un-parked.

**Why parked:** most of the games' Python was art (now `.pixels`), build-time data tables (normal), or post-processing gbcli output (since made features); only the bots argue for in-process scripting, and the script API is a second interface to design and keep stable beside the JSONL protocol. The examples DX review (2026-11-08) set the bar: a built-in input driver when a third game shows it's needed.

**Un-park when:** an agent ranks driving an input bot among its costliest stumbles, or a real-time game's bot needs a decision every frame or two, so per-call overhead dominates. C (the raycaster) is the next test, and its agent reports its bot's call count and wall time.

> **Update, 2026-11-09:** the condition used to read "the D or E DX doc shows an input bot … costing real time or calls again". D's bot made about 1,100 cheap calls (6.5 s a win), and the agent ranked it its best help; E had no bot. The condition held by its wording but not its intent, so Ben kept the entry parked and sharpened the condition ([2026-11-09-examples-dx-review-de.md](2026-11-09-examples-dx-review-de.md), question 5).

> **Update, 2026-11-09 (seventeenth session):** C's bot won Lantern in 108 calls and about 4 s, and its agent judged that gbcli needs no embedded scripting for it, so speed doesn't un-park this. But Ben ruled that gbcli must stay usable in a web IDE built to WASM ([2026-11-09-web-target.md](2026-11-09-web-target.md)), where there is no Python or shell. That makes **portability** the stronger argument: one bot script that runs the same in the CLI, under `serve` and in a browser. QuickJS compiles to WASM; the alternatives are each host driving the protocol in its own language (bots don't move between hosts) or a small built-in input driver in the protocol. **Un-park also when:** the web work starts (its spike decides), or a bot is needed in a web host. C's bot is mostly *planning* (a BFS over the map) and only lightly *driving* (one call a turn or straight stretch): a built-in input driver could run its segments but not its search, which only a script engine or the host's own language can.

## 2026-11-09 — `equiv --sync-on EXPR`: compare frames where an expression changes

**What:** equiv compares the two sides not at the end of every frame but each time an expression changes (`--sync-on 'u16[wViews]'`), so a faster renderer that shows each view sooner still matches view for view.

**Why parked:** C (the raycaster) wanted it for its speed-ups, but a renderer that takes a new pose each view samples input at different frames once it is faster, so the faster build shows *different* views while keys are held, not only earlier ones. A frame-locked test mode in the ROM is the sound check, and `help optimize` now says so; `equiv --screen off` covers the timing-free `--ram` compare ([2026-11-09-examples-dx-review-c.md](2026-11-09-examples-dx-review-c.md), C-1).

**Un-park when:** a game whose views don't depend on input timing (a demo, a replayed cutscene, a pose taken from a fixed table) needs to check a speed-up and a frame lock won't do.

## 2026-11-09 — The screen's shade indices as data in `shot --json`

**What:** `shot --json` carries the 160x144 screen as shade indices (0-3, or CGB colors), so a caller compares pixels without decoding a PNG. Useful to Rust-library and web callers, who may have no PNG decoder.

**Why parked:** one agent (C) decoded `shot`'s 2-bit palette PNG, once, with a small fix to its decoder; `shot --help` now names the format ([2026-11-09-examples-dx-review-c.md](2026-11-09-examples-dx-review-c.md), C-9).

**Un-park when:** a second agent decodes `shot` PNGs to compare pixels, or a library or web caller needs the screen as data.
