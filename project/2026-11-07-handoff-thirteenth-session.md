# Handoff: 2026-11-07 (thirteenth session)

This handoff is current until the next commit after the one that adds it. Start with `job orient`, then read this. The previous handoff is [2026-11-07-handoff-twelfth-session.md](2026-11-07-handoff-twelfth-session.md).

## State of play

Ben answered four questions at the start, then was away; the session's context line was raised to 50% for this session only. Every agent commit was squash-merged after a read of its staged diff, through the pre-commit hook. **Nothing is pushed:** main is ahead of origin by every commit since before the twelfth session. Pushing waits for Ben.

**Ben's rulings this session** (in the GBC images plan, under question 3, and as job notes):
- **Override shades:** a `name.dmg.png` override's colors join the project-wide shade cascade.
- **`tile_map` on every build:** DMG builds and color art included, with `palette` empty when an image has no GBC version.
- **The protocol version:** a `version` op, not a field on `DoctorMade`.
- **The agent eval:** run tonight, in parallel with Stage 10.

**Landed:**

| Commit | Leaf | What |
|---|---|---|
| `8bd68e2` | sUOJQb | The `version` op; `PROTOCOL_VERSION` = 1 (`crates/protocol/src/version.rs`); `gbcli --version` goes through the op, with its text line unchanged. |
| `3c88e61` | RMi3Kk | `rgblink -p 0xFF`: every unused byte of a build is $FF, which makes the carried-flash rule's premise true. One shared git local-env list. |
| `8cfa480` | 0racny | The joint conversion (DMG and GBC tiles behind one map or frame list), plus the 2efed71 review's wire refusals. |
| `847d250` | k22J86 | Eval run 1's fixes: `help optimize` keeps its baseline (`build --out`); `measure --keys`; missing-.sym errors; a `project` verb class; `run --scale` reaches the audio views. |
| `e2da080` | htdhm9 | `serve` and `batch` start without a bundle and answer the ops that need none. |
| `20729c8` | zcxvjc | The GBC include API (`*TilesGbc`, `*Attrmap`, `*Palettes`, `ObjPalettesGbc`, `Palette<Name>`) and GBC previews. |
| `5701328` | k22J86 | Repeated rgbasm messages collapse (`last_line`, `count`); a remedy hint table. |
| `793da0d` | gbHEPa | A dual-mode starter. Under `--gbc` the build passes `rgbasm -D GBCLI_GBC=1`; the starter guards its color path with `IF DEF` and checks A and the header at run time. |
| `b486fc9` | k22J86 | Eval run 3's fixes: equiv names never-written bytes; a frame-budget summary; the piano roll at scale 2; a hint for a bare RAM symbol. |
| `eb0cf78` | 7gGMeZ | The `assets` op and verb (a project or a manifest, rgbgfx only); tilesets in `assets/tiles/`, read by `build` too. |
| `7858317` | YI5gQ6 | The GBC section of `help images`; GB Studio's key-green hint; four backlog entries. |
| `205f273` | k22J86, Fjy1RU | Eval run 4's fixes: frame-budget rows are lag-only (`--json` has every frame); a mid-frame start's first row is partial; equiv's verdict names what it compared. |
| `8f94018` | OxI70i, Fjy1RU | Review fixes: a bare serve keeps the no-bundle cause; the macro hint's place; equiv side B's warning; docs. |
| `866ad97` | 3MeEHM | Review fixes: a manifest's paths resolve against its folder; manifest remedies. |
| `44f9c45` | k22J86 | The eval's final run and summary. |
| `8ae4b66` | mAlE2F | Review fixes: per-sheet swap palettes `<Sheet>Palette<Name>`; palette label collisions; joint limits name the color image. Also adds the bespoke-core decision brief. |

**The agent eval (k22J86) is done:** five Sonnet runs, in [2026-11-07-agent-eval.md](2026-11-07-agent-eval.md).
- Every run completed.
- The gap ledger read `none` over 13 sessions.
- No stumble came from a core limit.
- Run 5 had no stumble from gbcli's errors or verbs.

The doc's summary carries the verdict on the audio views and the evidence for qfXJij.

**Decisions made without Ben (two-way doors), each recorded where it applies:**
- **`GBCLI_GBC`:** `build --gbc` passes `rgbasm -D GBCLI_GBC=1`, and `images.inc` no longer defines it. The starter guards with `IF DEF` plus its run-time check. The earlier "IF DEF from images.inc" brief was wrong: a project without images has no include.
- **Flash padding:** fixed by making the premise true (`rgblink -p 0xFF`), not by changing the rule.
- **Version 1:** `PROTOCOL_VERSION` 1 is the protocol as of this session's last commit, since it was never published. That covers three changes: the diagnostics collapse, the piano roll's default scale, and `assets/tiles/` reading. Design §5 says so.
- **Swap palettes:** each indexed sheet gets `<Sheet>Palette<Name>` for every named palette, reordered by its OBP. This is the form of Ben's question-4 ruling.
- **Bare serve:** starts Bare, carries the store's no-bundle cause, and names the session.
- **Manifest paths:** `gbcli assets --manifest` resolves relative paths against the manifest's folder; the wire stays relative to the server's cwd.
- **Frame-budget rows:** lag-only by default. There is no every-frame flag; `--json` has every frame.
- **Tilesets:** `build` reads `assets/tiles/` too, with `<Name>Attrs` per tile under `--gbc`.

**Reviews this session.** Each reviewer was given the code, not the authors' reports. Every confirmed finding was fixed:
- **`343e7b5`/`75445cf`:** 1 medium (flash padding) → 3c88e61.
- **`2efed71`:** 2 medium → 8cfa480. One needs Ben's decision (d8FHRl).
- **`bbc8de1`:** nothing lost, except twelve warm samples removed by intent.
- **`8bd68e2`/`e2da080`/`5701328`:** 1 medium and 4 low → 8f94018.
- **`847d250`/`b486fc9`/`3c88e61`:** 2 medium and 3 low → 8f94018 and 205f273.
- **`eb0cf78`:** nothing serious, 1 medium and 3 low → 866ad97.
- **`8cfa480`/`20729c8`/`793da0d`:** nothing serious in the core, 1 medium and 4 low → 8ae4b66.

**The suite** was over 30 s on most hooks (30-50 s), always while agents or LoadHigh's gbcli jobs were running (see Mistakes). It has not been re-timed on an idle machine; do that first, with `python3 -I scripts/bench/test-times.py`.

## Mistakes, so the next session doesn't repeat them

- **Release builds swapped LoadHigh's binary.** `cargo build --release` in the main checkout replaced `target/release/gbcli` five times while LoadHigh's sessions were running `serve` and `batch` from it, so they saw mid-night behavior changes. Gotcha added; build eval binaries with `--target-dir` outside the checkout.
- **A brief built on a false premise:** the gbHEPa brief's `IF DEF(GBCLI_GBC)`. The agent caught it, and its pushback is what made the fix right.

## In flight at handoff

Nothing. No agents are running, and no worktrees are left. **review-fixes13** verified the review-fix commits (205f273, 8f94018, 866ad97, 8ae4b66, 7858317): nothing serious. Its four small items were fixed in this handoff's commit (leaf zrNEBy):
- the RAM-section hint names an included file's own line;
- colliding palette labels only warn when no image is converted;
- two doc nits.

Left as designed: a run started at the boot handoff counts its first frame as whole, though on a CGB it is about 2% short (the probe's table is on zrNEBy).

## Needs Ben

- **qfXJij, the bespoke core: spike or park?** The evidence and a recommendation (park, with an un-park condition) are in [2026-11-07-bespoke-core-decision-brief.md](2026-11-07-bespoke-core-decision-brief.md).
- **d8FHRl:** an indexed sheet's colors shift when color art is added and `palettes.txt` has no `dmg` line. Options (a) warn (recommended), (b) refuse, (c) document. It's in the leaf.
- **A note, no decision needed:** for `vram_tiles` a tileset counts as one of the backgrounds, so a font beside a background isn't added to it (as designed: engine budgets are the program's). Say if you want it counted.
- **Push:** main is ahead of origin by every commit since before the twelfth session.
- **For LoadHigh:**
  - the `version` op;
  - `serve` without a bundle;
  - the `assets` op with a manifest;
  - `tile_map` on every build;
  - per-sheet swap palettes;
  - `GBCLI_GBC` now comes from `rgbasm -D`, not `images.inc`, and is no longer in the .sym.

## Next, in order

1. **Re-time the suite** on an idle machine, and move tests if it's still over 30 s.
2. **Ben's two decisions:** qfXJij, then d8FHRl. With d8FHRl, Stage 10 (NRCs8I) closes; F1KJNF, its review, is done.
3. **Unreviewed:** only zrNEBy's small fix, in this handoff's commit. Review it with the next batch.
