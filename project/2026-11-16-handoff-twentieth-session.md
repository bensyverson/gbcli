# Handoff: twentieth session (2026-11-16)

## What happened

- **Teddy & Toys shipped** (Stage 14, `hmDFvT`, closed). It's a sixth example game and the first from Ben's own design: a toy-factory arcade game from his sketch, built by three game agents using gbcli as users. It lives in `examples/teddy-and-toys/` (renamed from `examples/factory/` at Ben's request), with the ROM `teddy-and-toys.gb`. It's in `examples/README.md` and `scripts/examples/build-roms.sh`.
  - The design and every ruling are in [2026-11-16-teddy-and-toys-plan.md](2026-11-16-teddy-and-toys-plan.md): the controls table, Elle, the playtest and art-review rulings.
  - The game is built to be tuned: every gameplay number is in `include/tuning.inc`, and `python3 tools/make.py` rebuilds both ROMs and runs the 36-rule check in seconds.
  - Reference photos (Elle, the gag glasses) are untracked in `local/` and must never be committed.
- **The DX review** is [2026-11-16-examples-dx-review-factory.md](2026-11-16-examples-dx-review-factory.md): F-1 to F-13 and Ben's decisions.
  - **Ben's ruling there binds future reviews:** an agent's misreading of gbcli is a flaw in the help or onboarding text. It's also in auto-memory.
- **Stage 15** (`5ot0G5`) holds the six fixes from that review, imported under `9b5RBA`:
  - Dm8G8G: the LCD-off frame counts as lcd_off, not lag.
  - UltxXY: `build --out` resolves against the working directory.
  - NQKwil: session-folder and color-ROM notes.
  - sL0Oa8: `shot` pixel fingerprint and screen data, with `--crop`.
  - rKKR0I: seven help and onboarding texts.
  - YZth2Z: per-channel loudness and filmstrip crop.
- **An issue tree exists now:** "gbcli issues" (`wPySBI`, kind issue), created at Ben's go-ahead. Its one issue is nPIeLL: the warm suite read 35-37 s in the hook on every commit this session, though none changed Rust. It is not investigated.

## Where to start

- `job orient` still points at h6YERB (store parity nits, available since 2026-11-10). Stage 15's leaves are all available and independent.
- **Carving Stage 15.** rKKR0I owns the help topics and verb help. NQKwil, sL0Oa8, YZth2Z, UltxXY and Dm8G8G each change one help line next to their behavior. Either reserve the help files to rKKR0I's agent and have the others hand back their line as an integration note, or run rKKR0I last. Dm8G8G changes what `lag` counts, so it needs a test ROM that turns the LCD off mid-frame.
- **Pinned example binary:** `/Users/ben/.cache/gbcli-examples/target/release/gbcli`, built at 74432d7 for the game agents. Never use `target/release` (LoadHigh runs it).

## Traps met this session

- **After squash-merging an example game, rebuild its `build/` before any screenshots.** The review tool shot a stale ROM and showed Ben old art. `tools/review_shots.py` now refuses a ROM older than its sources.
- **`git mv` of a game folder carries its untracked `.gbcli/`.** Its sessions still name the old ROM path, so clear `.gbcli/sessions` after a rename.
- **A new session defaults to the DMG**, even for a `.gbc`. Pass `--model gbc` to see color. NQKwil adds a note for this.
- **The rest is in the gotchas.** A game folder without `.gbcli/` puts its sessions in the repo's `.gbcli/` (the 2026-11-07 entry, extended); the game's `tools/gbcli.py` now creates the folder.
