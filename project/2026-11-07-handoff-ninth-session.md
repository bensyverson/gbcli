# Handoff: 2026-11-07 (ninth session)

This handoff is current until the next commit after the one that adds it. Start with `job orient`, then read this. The previous handoff is [2026-11-07-handoff-eighth-session.md](2026-11-07-handoff-eighth-session.md).

## State of play

Ben was present at the start. He approved running hUGETracker's `uge2source` when it was built from source. Every commit below was squash-merged from an agent worktree after a read of its staged diff, and went through the pre-commit hook. No worktrees or agents are left running.

- **The review queue from the eighth session is cleared.** Three fresh reviewers covered `eb61ea3` (images), `1ffe1f6` (the music build) and `4aa3996` + `8c1f5de` (run fixes, onboarding). They confirmed 20 findings, filed under container `y7kaUM` (now closed) and fixed:
  - `cd55e6f` (JAzaVy), images:
    - Colors are counted in RGB555, as rgbgfx reads them.
    - New limits: `frame_span`, `frame_grid` and `romx_bank`.
    - The rgbgfx run for sprite sheets drops `-N`.
    - Every generated symbol is checked for collisions.
    - `.PNG` in any case is accepted.
  - `d5c4442` (dwM7t6), run fixes and onboarding:
    - On SGB, the LCD-off warning sees one transition per write.
    - `run --to` an address holding an illegal opcode now reaches it: execution filters are checked before the illegal-opcode check.
    - `opcode` takes exactly what rgbasm 1.0.4 takes.
    - The help's description of ticks is corrected.
    - LCD-off frame numbers match stop records.
    - A `%` region offset is refused.
  - `949470b` (pruzN6, pukTN6), the music build:
    - A ROM-only link past bank 1 fails the build, naming the sections. The check covers any section, not only songs.
    - 6xx works: `routines:` is a table of routine addresses, as in hUGEDriver's `song.asm`.
    - Routine labels are global, so their local labels scope.
    - Dotted stems are handled, and every global symbol is checked for clashes.
- **pukTN6 is closed: our emitter matches hUGETracker's own export.**
  - Rosetta isn't installed on this Mac, so the 1.0.11 release's x86_64 binary can't run.
  - `scripts/music/export-reference.sh` (`21780eb`) builds `uge2source` natively from hUGETracker 0623f3f7, the 1.0.11 commit. It uses FPC 3.2.2 (`brew install fpc`, now installed here) and Lazarus 3.0's LazUtils, the same pair hUGETracker's CI uses. The only change is an empty `Dialogs` unit, which `codegen.pas` imports but never calls.
  - All 15 fetched samples give identical sound-register writes: 600 frames in the suite, and the full length in the pre-release suite.
  - So does the CC0 test song, now `every_effect_but_6xx.song`, which uses every effect but 6xx, against its committed export.
  - **The suite now needs those exports.** Run `scripts/music/export-reference.sh` after the corpus fetch; AGENTS.md says so.
- **Also landed:**
  - `e65e509` (l1ql7D, 3LC83k):
    - `--rtc-advance` is capped at 1024 days, with checked addition in the shim.
    - A power cycle carries the core's own battery state (`Core::battery_state`), so HuC3 and TPP1 carts can be power-cycled.
    - Structured OAM reads: `oam[N]` reads as fields, and `oam[N].y|x|tile|attr` is a byte in reads and conditions. `ReadValue.value` is now optional.
  - `0cc6a6a` (FPQsnp): fixes from a second review of the three fix commits.
    - 16-bit PNGs are scaled as libpng scales them.
    - A fail-closed `rgbgfx_agrees` check compares rgbgfx's tile data with gbcli's plan, pixel by pixel.
    - The `opcode` lookup refuses missing commas, and accepts `high()`/`low()` and constant expressions where rgbasm does.
- **Decisions made without Ben (two-way doors):**
  - The ROM-only bank check is a general build check, and the starter stays ROM-only.
  - 6xx departs from hUGETracker's RGBDS export (whose 6xx is broken) and follows its preview path.
  - RGB555 near-duplicates are merged, as rgbgfx does, not refused.
  - Structured OAM reads are built rather than struck from the design. They are scoped to values; poke, watch and patch still take `oam:N`.
  - The RTC advance is capped at 1024 days.
  - The engine's tests now need RGBDS on `PATH`, for the opcode lookup's rgbasm cross-check.

## Needs Ben

- **Shade spreading for images** (design §12, the JAzaVy update): when two colors would share a DMG shade, every color is spread lightest to darkest. So two near-whites become white and black, and a sprite drawn in black plus #101010 renders #101010 as white. Should the spreading be gentler, or only warn? The help now describes the current rule.
- **Rosetta**, optionally: `softwareupdate --install-rosetta --agree-to-license` would let the official release binary serve as a second check. It isn't needed; the native build is hUGETracker's own code.

## Next, in order

1. **Review `0cc6a6a`** (FPQsnp), which is unreviewed. It touches a wire format (the `ImageLimit::RgbgfxAgrees` variant) and adds `crates/engine/src/disasm/expr.rs` and `crates/build/src/images/agree.rs`. Everything else since the baseline is labeled `reviewed`.
2. `XJV3UL`, low findings from the review of `e65e509`:
   - The 1024-day bound's "no reading lost" claim holds for MBC3 only, not TPP1 or HuC3.
   - Say that `oam` fields read raw OAM.
   - Stop printing `palette` in CGB rows.

   Probes are in this session's scratchpad. The leaf restates them.
3. The agent eval, `k22J86`. Stage 6 is complete apart from `XJV3UL`.
4. The flash-chip model, `L00RuF`.

## Suite time

The hook warned on every commit: 38–91 s against the 30 s budget. The load average was 17–34 throughout, from this session's agents and other sessions. The music agent measured its own additions at about 3 s. `scripts/bench/test-times.py` summed 48.6 s at load 34. **Re-time on an idle machine** before moving anything to the pre-release suite (gotchas, 2026-11-07). If it is still over budget, the first candidates are the 15 per-sample `music_equiv` tests at 600 frames each and the rgbasm cross-check (about 2 s).
