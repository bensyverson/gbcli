# Handoff: 2026-11-07 (eleventh session)

This handoff is current until the next commit after the one that adds it. Start with `job orient`, then read this. The previous handoff is [2026-11-07-handoff-tenth-session.md](2026-11-07-handoff-tenth-session.md).

## State of play

Ben was present throughout. Every agent commit was squash-merged after a read of its staged diff, through the pre-commit hook.

**The machine had no toolchain at the start.** This checkout had no Rust, RGBDS, FPC, corpus or `local/`, and `core.hooksPath` was unset, so `36a03b3` was committed without the hook (the suite later passed on it). Ben approved the setup: rustup (rustc 1.99.1), `brew install rgbds fpc` (1.0.4, 3.2.2), the hook enabled, the corpus fetched and the music references exported. Ben added the GB Studio ROM at `local/demo/build/rom/demo.gb`. Steps in [docs/development.md](../docs/development.md); traps in gotchas (2026-11-07 entries).

**Landed:**

| Commit | Leaf | What |
|---|---|---|
| `36a03b3` | — | Ben's flash-session rulings in the flash-model doc; the GB Studio flash tests moved to pre-release. |
| `06d9903` | k5Afkg | GBC plan revision 2 after the LoadHigh review; the e9cba4f and d0e891a review findings filed. |
| `e29f471` | L3PdEW | The project-level `shared_shade` lists image-joined groups of colors (chains included), so its message is true. |
| `38ef57b` | k5Afkg | GBC plan approved and imported as Stage 10 (`NRCs8I`); option C parked in backlog. |
| `68b7e40` | mDk1lR | Seeded differential test of `gbcli opcode` against rgbasm (400 lines warm, 0.2 s; 40,000 pre-release, 5.8 s); eight disagreement classes fixed. |
| `fc85d19` | 6pr1FU, 4hM0UN | Flash review fixes (mapper from the unpadded image, then `gbcli_set_rom_size`; datasheet decode masks; chip commits after SameBoy accepts) and flash sessions (`--flash BOARD`, header field, FlashConflict, `game.flash.gb` beside `game.sav`, notes, unlock-cycle hint). |
| `b480dc2` | — | AGENTS.md head cut to what every agent needs (Ben's ruling, in the head); crate map in `docs/architecture.md`, setup in `docs/development.md`, build figures in `scripts/bench/README.md`. Carries Ben's agents-md sync. |
| `81cf3d2` | vo5gRl, ZA5Z7c, WmNCxc | `build --gbc` (rgbfix -c, typed `BuildTarget`), the GBC protocol types (`Rgb555`, `NamedPalettes`, `GbcImage`, …), `assets/palettes.txt` replacing `palette.txt`. |
| `54c9d3e` | d7DbmT, cnXB3b | `--model gbc` → CgbE; deterministic palette packing (`crates/build/src/images/pack.rs`, not wired yet). Also untracks `.gbcli/` (committed by mistake in `fc85d19`) and ignores it. |
| `6c5b029` | DkhKgi | Flash-session review fixes: run, step and their failures report their own notes (`ErrorBody::flash_notes`); a rebuild drops carried sectors that overlap the new image, with a warning; `sav` writes both files or neither. |

**Ben's rulings this session:**
- Flash: `--flash PROFILE` at creation, named profiles only, the unlock-cycle hint, `sav` carries the flash save area; the GB Studio tests in the pre-release suite (flash-model doc, "Decided").
- GBC plan: question 4 is one named palette per indexed image (after asking what that implies); question 5 counts what the build places; the rest are the recommendations. Option C (color art matched to named palettes tile by tile) is in backlog.md.
- AGENTS.md: the head holds only what every agent needs; details go in `docs/`.

**Decisions made without Ben (two-way doors):**
- A rebuild that grows into a carried flash sector keeps the new code and drops that sector with a warning (DkhKgi).
- Colors reduce to RGB555 by truncation, as rgbgfx reads a PNG, not by rounding (plan corrected; LoadHigh should match).
- The opcode differential test compares opcode and length, which is everything the lookup reports.

**Reviews this session:**
- `e9cba4f` (flash model): nothing serious; three lows fixed in `fc85d19`.
- `d0e891a` (shades): two lows. One is fixed in `e29f471`; the other became a palettes-leaf criterion.
- `fc85d19` (flash sessions): nothing serious. Two lows and three nits are fixed in `6c5b029`, and the hint fired on none of 3,809 corpus ROMs.
- radJxi: covered by mDk1lR's differential test.

Labeled `reviewed`: L00RuF, GTXXyr, radJxi, mDk1lR, 6pr1FU, 4hM0UN.

## In flight at handoff

Nothing. No agents running, no worktrees.

## Needs Ben

Nothing open. One thing to pass on: LoadHigh should truncate colors to RGB555 (top 5 bits), matching gbcli and rgbgfx.

## Next, in order

1. **Review the unreviewed commits:** `81cf3d2` (protocol types are LoadHigh's seam, a wire format: review before building on them), `54c9d3e` (packing, model alias) and `6c5b029`. The fix's tests came from the reviewer's probes, but no fresh verifier ran them. Also do a short pass over the whole session range `36a03b3..HEAD`, as the tenth handoff suggested.
2. **The warm suite is over budget:** `python3 -I scripts/bench/test-times.py` gave a sum of 47.8 s wall and 106 s CPU, load average 10.9, run one binary at a time. Hook runs said 55-85 s under agent load. The biggest costs are `gbcli-build::music_equiv` (7.0 s wall, 31 s CPU), `gbcli-server` (3.3 s, 17 s CPU) and the CLI suite (3.2 s, 16 s CPU). Find what to move to pre-release or speed up (project/2026-11-06-test-suite-speed.md).
3. **Stage 10 continues:** `tzEhz3` (derived DMG version, `.dmg.png`) and `k5Kadv` (indexed art) are unblocked and parallel. Then `0racny` (joint conversion), which has a note: settle against rgbgfx `-c` how a short BG palette is padded and what OBJ slot 0 holds. Then `zcxvjc` and `7gGMeZ`, then the starter.
4. **k22J86**, the agent eval, once Stage 10's images land or before, as Ben prefers.
