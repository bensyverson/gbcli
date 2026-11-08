# Architecture

Which crate owns what. gbcli is a Rust workspace: library crates hold every decision, `gbcli-server` and `gbcli-cli` are thin adapters over them, and the `gbcli` binary only wires argv, the environment and stdio into `gbcli-cli`. The approved design behind this is [project/2026-11-06-design.md](../project/2026-11-06-design.md).

Add a crate or a major capability here when you build it; keep the head of `AGENTS.md` free of feature lists.

## The emulator

- **`crates/core` (`gbcli-core`)**: the `Core` trait, our own seam over any emulator backend, and its types (`CoreConfig`, cart clocks, flash notes).
- **`crates/sameboy-sys`**: vendored SameBoy, built by the `cc` crate, with hand-written FFI and the small C accessors in `shim/`. The vendored commit, compile flags and patches: [project/2026-11-06-sameboy-vendoring.md](../project/2026-11-06-sameboy-vendoring.md).
- **`crates/sameboy` (`gbcli-sameboy`)**: the SameBoy backend implementing `Core`: bank-aware filters, never-written tracking, the checkpoint blob, and a flash chip on flash-save carts.
- **`crates/flash` (`gbcli-flash`)**: an AMD/JEDEC flash-chip model for flash-save carts such as GB Studio's batteryless saves, parameterized by board (unlock addresses, D0/D1 swap, sector map, /WE pin, chip size, busy times) with named profiles from FlashGBX's database; commands that miss the board become `FlashNote`s, and mask-ROM programs that send flash commands get a hint. A session takes a board with `--flash BOARD` (`gbcli help flash`; [project/2026-11-07-flash-model.md](../project/2026-11-07-flash-model.md)).

## The engine and the session

- **`crates/engine` (`gbcli-engine`)**: inspection, the disassembler and `gbcli opcode`'s lookup (`disasm::lookup`, checked against rgbasm by a seeded differential test), sessions, checkpoints, the debugger, the cycle profiler (`measure`: cycles per routine and the per-frame budget) and the A/B equivalence check (`equiv::compare`: two builds under one key timeline, the first frame whose pixels, chosen RAM or sound-register writes differ). Run's `--measure`, `--profile`, `--frame-budget` and `--raster` (IO-register writes by line) come from here.
- **`crates/protocol` (`gbcli-protocol`)**: the wire types every crate shares: requests and responses, the checkpoint header and stop record, the build and image results, and the named palettes. A type crossing the JSONL seam is defined once, here.
- **`crates/bundle` (`gbcli-bundle`)**: the session bundle store (no daemon).
- **`crates/corpus` (`gbcli-corpus`)**: the test-ROM runner and the SameBoy baseline ([project/2026-11-06-sameboy-baseline.md](../project/2026-11-06-sameboy-baseline.md)).

## Views

- **`crates/view` (`gbcli-view`)**: image views as deterministic PNG pinned by goldens: labeled frames, scales and crops, filmstrips, frame diffs, tileset, tilemap and OAM.
- **`crates/audio` (`gbcli-audio`)**: the sound-register write log decoded into notes and settings, WAV export (mixed and per channel), piano roll and spectrograms, pinned by goldens; wired into `run` as `--audio-log`, `--wav PATH [--channels each]`, `--piano PATH` and `--spectrogram PATH`.

## The build loop and assets

- **`crates/build` (`gbcli-build`)**: the RGBDS loop. RGBDS from `PATH` checked against the 1.0.4 pin; a build by convention with every diagnostic as a file, line and message; `doctor`'s checks; the `new` starter project (vendored hardware.inc, hUGEDriver and a player for `assets/music/theme.song`). Songs under `assets/music` become hUGEDriver data by convention. PNGs in `assets/bg/` and `assets/sprites/` are checked against the hardware's limits (structured problems with remedies), converted by rgbgfx, previewed, and exposed through a generated `images.inc` and `images.asm` (`gbcli help images`). `build --gbc` writes a dual-mode header and derives each color image's DMG version (`images/derive.rs`; a `name.dmg.png` overrides it), a session made with `--model gbc` runs it as a GBC, and `images/pack.rs` packs color art into GBC palettes deterministically (not yet wired into the build); GBC images are Stage 10 ([project/2026-11-07-gbc-images-plan.md](../project/2026-11-07-gbc-images-plan.md)).
- **`crates/music` (`gbcli-music`)**: hUGETracker songs: `.uge` v0-v6 read and v6 written, a canonical tracker-grid text form both ways with parse errors located to line and column, a piano roll drawn from a song file, and the song to RGBDS `.asm` emitter, a port of hUGETracker's export (`gbcli song`, `gbcli help music`).

## Adapters

- **`crates/server` (`gbcli-server`)**: request handling over the library crates, screenshots, VRAM/OAM views, key input on runs, and the JSONL stdio server (`gbcli serve`).
- **`crates/cli` (`gbcli-cli`)**: argv to protocol requests, answered in-process by the server's handler; rows or `--json` (the serve response line); the primer, `--help`, help topics, recipes and the schema. Every server op is a verb; `gbcli --help` lists them, and `crates/cli/src/verbs/` holds the table.
- **`crates/gbcli`**: the binary, and the CLI's golden transcripts (`tests/cli/`).
