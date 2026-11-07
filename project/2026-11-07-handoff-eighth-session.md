# Handoff: 2026-11-07 (eighth session)

This handoff is current until the next commit after the one that adds it. Start with `job orient`, then read this. Earlier handoffs: [2026-11-07-handoff.md](2026-11-07-handoff.md) (seventh) and [2026-11-06-handoff.md](2026-11-06-handoff.md).

## State of play

Ben was present at the start and for the battery-save ruling. Every commit below was squash-merged from an agent worktree after a read of its diff, through the pre-commit hook. No worktrees or agents are left running.

- **The review queue from the seventh session is cleared.** Two independent reviewers covered `e3ca2be` (the build loop's fixes and server ops) and `1a4bfdc` (gbcli-music). Container `O0rtXX` is closed.
  - `19f5ff8` fixed the build loop's findings:
    - Each tool runs in its own process group, so the time limit kills the whole group.
    - Remedies are complete and shell-quoted.
    - An `--out` that collides with its own `.sym` or `.map` is refused.
    - Stale staging directories are swept, using lock files.
    - The server checks `--restore` before building, and the CLI keeps no copy of that check.
  - `efc57b8` fixed the music crate's findings:
    - Every song is normalized the way hUGETracker loads it: uneven orders are padded with P0 and missing patterns are created.
    - The text form refuses `-` in orders.
    - The piano roll follows hUGEDriver's 7xx and Exx.
    - `song` warns about subpattern rows past 31.
    - Numbers are parsed strictly.
- **Ben's battery-save ruling** is recorded in design §6 ("Ruling, 2026-11-07"). Battery RAM lives in checkpoints. An explicit power cycle keeps battery RAM and the RTC. A `.sav` is written or read only on request, in SameBoy's format, with no host time on import. His lens for such questions: choose what is most ergonomic for agents; `.sav` compatibility is a bonus for end users.
- **Landed, newest last:**
  - `8c1f5de` (shsW4F, pa2Jwt), onboarding:
    - `help hardware`: every figure cites Pan Docs, and `scripts/help/check-doc-links.py` checks the links.
    - The `opcode` verb.
    - A dev-loop recipe, which a test runs line by line.
    - `help traps`, with four demo ROMs and an RGBDS syntax pair.
    - The colliding help topics are renamed `rgbds` and `checkpointing`.
  - `a066271` (OU9rOL), battery saves and mappers:
    - The `power-cycle` and `sav` verbs.
    - ROM header warnings.
    - Mapper tests; the docboy MBC3 clock failures are explained as SameBoy gaps.
    - A review found that `power-cycle` dropped breakpoints and watches; that is fixed in `9149767`. The two lesser findings are leaf `l1ql7D`.
  - `eb61ea3` (WdnjHI), images in `build`:
    - The conventions are `assets/bg` and `assets/sprites`, the sprites as 8x16 sheets with `name.WxH.png` frames.
    - gbcli checks the DMG limits itself before running rgbgfx.
    - A preview is always written.
    - The program gets `images.asm` and `images.inc`.
    - DMG only. It does not use `-c dmg`, which refuses transparency and non-gray art.
  - `4aa3996` (XStY5L, k56pRp, jXavVc), run fixes:
    - Region offsets in conditions are hex.
    - A targeted run that locks up fails with the new code `target_not_reached` (exit 3). The stop-reason table is in design §11.
    - A run warns when the LCD is turned off outside VBlank, through an opt-in core LCD-off log.
  - `1ffe1f6` (pukTN6, left open), the music build loop:
    - The emitter ports hUGETracker's RenderSongToRGBDSAsm, with labels scoped to the descriptor.
    - `build` converts `assets/music/*.song|*.uge`.
    - `new` vendors hUGEDriver **a3cbd0ce**, not 727ab8e; the research doc and design §12 are corrected.
    - **Every `new` project now has music**: a player, the driver and `theme.song`.
- **Decisions made without Ben (two-way doors):**
  - Colliding help topics are renamed.
  - The server owns the restore checks.
  - Uneven orders are refused in text and normalized on read.
  - Subpattern rows are all stored, with a warning about the unplayable ones.
  - Region offsets are hex everywhere.
  - `target_not_reached` is a new code. A caller's own breakpoint or watch that fires first stays exit 0 with a warning; revisit this if agents misread it.
  - One LCD-off warning per run.
  - Music is in every starter, with no `--music` flag.
  - The image conventions above.

## Needs Ben

- **`pukTN6` (labeled `decision`):** how to get hUGETracker's own `.asm` export, the reference for criteria Nt1 and r4P. The options are in the leaf's last note:
  1. Recommended: fetch the hUGETracker 1.0.11 macOS release and run its `uge2source` (an x86_64 binary, so Rosetta) as a corpus step.
  2. Build `uge2source` with FPC and Lazarus.
  3. Use hUGEDriver's `sample_song.asm` as a single-song reference.

  Close the leaf once its comparison runs.

## Next, in order

1. **Review before building on it.** All of these are unreviewed:
   - `eb61ea3`: images, about 3,000 lines with tests.
   - `1ffe1f6`: music emitter and build, which vendors a driver.
   - `4aa3996`: the core trait plus a SameBoy LCD-status callback. That is FFI, so it is a review trigger.
   - `8c1f5de`: onboarding and `opcode`.
   - The review baseline in AGENTS.md is still 2026-11-07T04:55:00Z. Labeled `reviewed`: U2rLOt, 4BCyGD, oYZJFg, OU9rOL. `job ls --status done --leaves --without-label reviewed --since 2026-11-07T04:55:00Z` lists the rest.
2. `pukTN6`'s reference export, once Ben rules.
3. `l1ql7D`: bound `--rtc-advance`, and let a power cycle keep a HuC3 or TPP1 battery. The probes are in the session scratchpad; the leaf names the path, which may not survive, but the findings are restated in the leaf.
4. `3LC83k` (structured reads like `oam[0]`), then the agent eval `k22J86`.

## Suite time

The hook warned on every commit: 33–174 s against the 30 s budget. The worst runs came while other agents compiled. The last commit, `1ffe1f6`, took 65 s with no agent of ours running, but the load average was still 15–43 from other sessions, and the figure includes the hook's own compile.

Re-time on an idle machine before moving tests to the pre-release suite, with `python3 -I scripts/bench/test-times.py` (gotchas, 2026-11-07). New this session: the music fuzz full-size runs are pre-release.
