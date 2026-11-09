# Findings: what the racer and the shooter taught us about gbcli

2026-11-08, fourteenth session, leaf `unOF1c` (review-ab). **Ben ruled on 2026-11-08; the decisions are at the end.**

Two Opus agents each built a vertical slice with gbcli as users: Switchback (`examples/racer/`) and Vesper Swarm (`examples/shooter/`). Each finished all four of its leaves in one long session, with gbcli pinned at `4381e6a`. The integrator rebuilt both games and replayed both recorded wins, on the DMG and on the GBC. Sources: [2026-11-08-dx-racer.md](2026-11-08-dx-racer.md) and [2026-11-08-dx-shooter.md](2026-11-08-dx-shooter.md), written blind to each other, plus Ben's own finding mid-run (both agents write Python).

**The strongest signal is convergence.** Two agents, working independently on different games, hit the same seven things. Those come first, ranked by the time they cost across both games.

## Hit by both agents

1. **No PPU mode on raster writes** (about 1 h 20 min together; each agent's #1).
   - **What happened:** `--raster` prints `into=` ticks and then `raster_unsupported feature=ppu_mode_events`, so "every scroll write lands in HBlank" had no direct check.
   - **The workaround:** both agents found a conditional breakpoint on the write instruction (`--if '([rSTAT] & 3) != 0'`). Nothing in the help suggests it for this check.
   - **What the workaround caught:** two real late writes in the racer (a mid-line write at frame 7, and a mode-2 write on a line with 10 objects).
   - **Worse:** on the racer, the `[rSTAT]` condition itself failed the run as `unsupported (SideEffectFreeIo): PPU registers are unavailable while mode-3 cycles are batched`, while `show` reported a breakpoint stop. That's the exact case the check exists for, reported ambiguously.
   - **The ledger:** this is the first eval with a non-empty `gbcli gaps`: `ppu_mode_events`, `ppu_dot` and `side_effect_free_io`.
   - **Recommendation:**
     - **(a) now:** a recipe and a `help traps` line for the breakpoint check.
     - **(b) a spike:** can the SameBoy adapter record the PPU mode at each raster write, and evaluate `[rSTAT]` in a condition without the batching refusal? If yes, add `mode=` to `raster_write`.
     - **(c) only if (b) fails:** take it to the bespoke-core un-park condition. Today the gaps were worked around, not blocking, so the condition doesn't hold yet.
2. **Color art doesn't build for the plain DMG target** (about 45 min plus two art redesigns).
   - **What happened:** only `--gbc` derives a color image's DMG version, and a `.dmg.png` is an error without `--gbc` (Ben's ruling on question 9). So a game that must build clean both ways is pushed to indexed art.
   - **The help misleads:** "derives its DMG version" reads as if both builds can use it.
   - **Recommendation:** let a plain `build` use the same derived DMG version, and the `.dmg.png` override, so one project builds both ways. **Needs Ben:** it reverses question 9's ruling, whose premise was that a stray `.dmg.png` would be silently ignored. Under this change it wouldn't be ignored.
3. **The frame budget counts LCD-off frames as lag** (about 30 min, and a Python script in each game).
   - **What happened:** startup and screen changes turn the LCD off on purpose, yet every from-boot run says `lag=5`-`10`, and `busiest` names a loading frame.
   - **Recommendation:** a per-frame `lcd_off` flag in `--json`. The summary row then counts LCD-off frames separately from lag and leaves them out of `busiest`. Both budget scripts become unnecessary.
4. **`key_unapplied` floods a stopped run** (about 20 min, and a lot of context).
   - **What happened:** a run that stops before its keys file ends prints every leftover event twice: in a `warning:` line, then as one row each. That was 150-320 lines.
   - **Recommendation:** one row on stdout (`keys_unapplied count=290 first=DOWN@3402`), with the full list in `--json`.
5. **A rebuild costs the session** (about 15 min, in every session of both games).
   - **What happened:** `rom_changed` refuses the old head and checkpoints. `sessions rm` then refuses while checkpoints exist. The racer also found that `build DIR --session S` looks for the session in the cwd, not in DIR.
   - **Recommendation:** one verb, or a flag on `run` (say `--restart`), that boots an existing session afresh on the new ROM and keeps its breakpoints. Also, `build DIR` finds sessions in DIR. The shape is a design question for its leaf.
6. **`shared_shade` warnings are noisy and can't be resolved** (about 15 min).
   - **What happened:** four 700-character warnings repeat on every build. With many palettes the remedy can't apply: on a DMG every palette's lightest color is white. The racer also saw the plain and `--gbc` builds disagree about the same art.
   - **Recommendation:** collapse to one row per shade, and stop warning on colors from different named palettes. Sharing a shade across palettes is how a DMG version works.
7. **The profiler picks the wrong name** (about 10 min).
   - **What happened:** `$0048` became `VBlankVector+$8`, and WRAM code became hUGEDriver's `end_zero`.
   - **Recommendation:** name the hardware vectors (`STAT vector`), and prefer a code label over a data or driver symbol at the same address.

## Hit by one agent

8. **ROM pressure** (racer, about 30 min). `--gbc` roughly doubles image ROM, and generated images and songs float to any bank, which makes MBC1 impractical. **Recommendation:** a help note now. Defer a way to place generated data in a bank until a game needs it.
9. **No views across frames** (shooter, about 15 min). OAM per frame took a script calling gbcli twice a frame, and `--audio-log` has no filter. **Recommendation:** `run --oam-log` (like `--audio-log`) and a channel filter on `--audio-log`.
10. **`--dir` doesn't anchor relative paths** (racer, minor but on every call). **Recommendation:** fix, so `--dir` anchors the ROM path too.
11. **No 8x8 sprite sheets** (shooter, about 10 min). The sheets make only 8x16 objects, so the shooter used tilesets and loaded OBJ palettes by hand. **Recommendation:** sheets with 8x8 frames (`hero.8x8.png`).
12. **Smaller items:**
    - an uninitialized `--read` throws away the whole stop instead of failing that one read;
    - keys-file offsets can't count from a time base when started from a checkpoint;
    - no palette RAM view;
    - song note names are an octave above scientific pitch, and the help doesn't say so;
    - both builds write `build/NAME.gb`;
    - `dropped_by_line_limit` doesn't say which lines dropped an object;
    - held keys persisting across runs works but isn't documented.

    **Recommendation:** fix the read error, the doc lines and the palette view, and backlog the rest.

## Ben's finding: both agents write Python

The scripts are explained in the DX docs.

- **Art** (prescribed by the brief). Music has an agent-writable text format; images don't. Neither agent complained, and both produced good art this way. **Recommendation:** backlog a pixel-art text format, to un-park if a later eval shows art scripts costing real time.
- **Data tables** (the racer's road geometry). Normal practice. No change.
- **Post-processing gbcli's output** (budget peaks, OAM coverage). These disappear with items 3 and 9.
- **Closed-loop input bots.** Both agents built one, and both rank it as what helped most. Determinism plus `--read` turned "a scripted run wins" into writing a bot once. **Recommendation:** document the pattern as a recipe now. A built-in input driver (through `serve`) can wait until a third game shows it's needed.

## Also from planning and review

- **`gbcli new` makes no `.gbcli/` folder,** so a project nested under another one saves its sessions there. **Recommendation:** have `new` create it.
- **The `assets`/`build` summary row counts only toolchain diagnostics,** beside an `images` row that counts image problems. **Recommendation:** the help says so.
- **Leaf `zqarpL`:** a manifest's remedies name a `dmg` line that doesn't exist. It's already filed.

## What the brief got wrong

These are for the D/E/C briefs.
- **The 40% context line was too early** for a four-leaf chain. Ben's halfway rule fixed it.
- **"DMG first, also clean under `--gbc`" didn't say the two builds can disagree.** Build both from the start.
- **Inline `python3 -c` edits can be refused by the worktree guard,** and the stale code still built. Brief agents to write edit scripts to files.

## Decisions (Ben, 2026-11-08)

- **Item 2, color art: derive in both builds.** A plain `build` derives a color image's DMG version as `--gbc` does, and honors `.dmg.png`. This supersedes GBC plan question 9, marked in place there.
- **Items 1 and 3-12, and the planning and review items: accepted as recommended.** Each becomes a leaf under Stage 11's gbcli fixes. These go to the backlog: keys offsets from a time base, separate default output names per target, and `dropped_by_line_limit` per line (from item 12), plus a way to place generated data in a bank (item 8).
- **Art authoring: build a pixel-art text format now,** rather than backlog it. It is to images what song text is to music: agent-writable, readable, convertible both ways, and read by `build`. Game agents may also be encouraged, in their briefs, to use `woodcase`, Ben's vector design tool (`.pen` files rendered to PNG), for larger pieces. It is not to be recommended in gbcli's help, since it's a general vector tool and not part of gbcli.

## The work

Imported into `job` on 2026-11-08 under `SQhTqx` (Stage 11's gbcli fixes); the tracker is the plan from here on. Leaves are grouped by the files they touch, so they can run in parallel. One shared file, the CLI's help golden (`crates/gbcli/tests/cli/golden/help.txt`), will conflict between leaves; the integrator re-blesses it at merge.

```yaml
tasks:
  - title: 'PPU mode at raster writes: a spike, and the HBlank recipe'
    ref: ppumode
    desc: |
      Why: both example-game agents' costliest stumble (project/2026-11-08-examples-dx-review.md, item 1). `--raster` prints `into=` ticks then `raster_unsupported feature=ppu_mode_events`, so "every scroll write lands in HBlank" has no direct check. Both agents invented a conditional breakpoint on the write instruction (`gbcli break PC --if '([rSTAT] & 3) != 0'`), which caught two real late writes in the racer. On the racer that same condition failed the run with `unsupported (SideEffectFreeIo): PPU registers are unavailable while mode-3 cycles are batched`, while `show` reported a breakpoint stop: the exact case the check exists for, reported ambiguously.

      What: (1) A recipe in `gbcli recipes` and a line in `gbcli help traps` for checking that a write lands in HBlank, whatever the spike finds. (2) A spike, answered in a dated finding: can the SameBoy adapter record the PPU mode (and the dot, if cheap) at each raster write, and can a condition read `[rSTAT]` without the batching refusal (for example by evaluating STAT from the adapter's own mode tracking, or by unbatching around breakpoints that have conditions)? If yes, implement `mode=` on `raster_write` and fix the condition so it either evaluates or refuses before it stops, never both. (3) Re-check the bespoke-core un-park condition (project/backlog.md, A bespoke Rust emulator core) against the result, and say in the finding whether it now holds.

      Seams: crates/sameboy (the adapter, vendored SameBoy patches), crates/engine (raster observation, conditions), crates/protocol (RasterWrite), CLI render and help. The gotchas file explains the vendored-SameBoy patch workflow (scripts/vendor-sameboy.sh).

      Out of scope: mid-mode-3 accuracy itself.
    criteria:
      - the HBlank recipe and traps line exist, with a test that the recipe's commands parse
      - a dated finding answers the spike's two questions with evidence, and says whether the bespoke-core un-park condition holds
      - if feasible, raster_write carries the mode (test against a ROM writing in HBlank and in mode 3), and a [rSTAT] condition never both errors and stops (regression test from the racer's case)
  - title: "run's output: lcd_off in the frame budget, one-row key_unapplied, per-read errors, --dir anchoring"
    ref: runout
    desc: |
      Why: four run-output stumbles from the example games (project/2026-11-08-examples-dx-review.md, items 3, 4, 10, 12). Both agents wrote Python to filter the frame budget.

      What:
      - Frame budget: a per-frame `lcd_off` flag in `--json`; the summary row counts LCD-off frames apart from lag (e.g. `lag=0 lcd_off=7`) and leaves them out of `busiest`/`active_max`. The help topic `budgets` says so.
      - key_unapplied: a stopped run with leftover key events prints one row (`keys_unapplied count=N first=KEY@FRAME`) instead of a warning listing them plus one row each; `--json` keeps the full list.
      - An uninitialized `--read` (or any one failing read) reports that read's error in its row and keeps the stop and the other reads, rather than failing the run with exit 3 (`--allow-uninit` keeps its meaning for the values).
      - `--dir` anchors relative paths given to the command (the ROM path at least), and `gbcli build DIR --session S` finds S under DIR.
      - Docs: `gbcli recipes` gains the closed-loop input bot pattern both agents used (drive a session a few frames at a time with `--read`, record key changes, replay with `--keys-file`); the help says held keys persist across runs.

      Seams: crates/engine (frame budget), crates/server (run handling), crates/cli (render, args, recipes, help), crates/protocol. Keep goldens honest.
    criteria:
      - frame budget lcd_off flag and summary, with a test where startup and a screen change are LCD-off and lag=0
      - one keys_unapplied row in text output, full list in --json (golden)
      - a failing read keeps the stop and the other reads (test)
      - --dir anchors the ROM path, and build DIR finds sessions in DIR (tests)
      - the bot recipe and held-keys line exist
  - title: 'Sessions survive a rebuild, and gbcli new makes .gbcli/'
    ref: sessions
    desc: |
      Why: every session of both games paid for rebuilds (project/2026-11-08-examples-dx-review.md, item 5). After `gbcli build`, `run --session S` fails `rom_changed` (RAM symbols gone), `sessions rm S` refuses while checkpoints exist, and `run --from boot` applies only when there is no head. Separately, `gbcli new` makes no `.gbcli/`, so a project nested under another `.gbcli/` saves its sessions there.

      What: one documented way to boot an existing session afresh on the rebuilt ROM, keeping its breakpoints and watches and setting old checkpoints aside (kept in history or dropped, the agent's call, stated in help), with the `rom_changed` error's remedy naming it. `gbcli new` creates the project's `.gbcli/` (already gitignored by the starter? check). The shape (a `run` flag such as `--restart`, or a verb) follows project/agents/cli-design.md; if it would change the session or bundle format, stop and report before building it.

      Seams: crates/server or wherever sessions and `rom_changed` live, crates/cli, the starter template for `new`.
    criteria:
      - after a rebuild that moves RAM symbols, one command restarts the session on the new ROM, keeping breakpoints (test)
      - rom_changed's remedy names it (golden)
      - gbcli new makes .gbcli/, and a run inside the new project saves its session there even under another .gbcli/ (test)
  - title: 'Images: color art in a plain build, quieter shared_shade, 8x8 sprite sheets, help notes'
    ref: images
    desc: |
      Why: image stumbles from the example games (project/2026-11-08-examples-dx-review.md, items 2, 6, 11, and notes).

      What:
      - Ben's ruling (2026-11-08, superseding GBC plan question 9; recorded in project/2026-11-07-gbc-images-plan.md under question 9): a plain `build` derives a color image's DMG version exactly as `--gbc` does, and honors `name.dmg.png`. So one project builds clean both ways. Update `help images` and the design doc.
      - shared_shade: collapse the project-level warnings to one row per shade, and don't warn on colors from different named palettes sharing a shade (that is how a DMG version of many palettes works). Within one image or tile it still warns.
      - 8x8 sprite sheets: `assets/sprites/NAME.8x8.png` makes 8x8 objects (today sheets make 8x16 only, `obj_pair_8x16`), with OBP and OBJ palette data as sheets have.
      - Help notes: `--gbc` color art roughly doubles image ROM; the `assets`/`build` summary row counts toolchain diagnostics while the `images` row counts image problems.
      - Also take leaf zqarpL (a manifest's remedies name a dmg line that doesn't exist), already filed with its reproduction.

      Seams: crates/build/src/images (derive.rs, shared.rs, sheet.rs, pipeline.rs), crates/protocol (images), crates/cli help images, goldens. Rulings in project/2026-11-07-gbc-images-plan.md bind (one color one shade everywhere; reorder-by-OBP; shades_unpinned).
    criteria:
      - a color background over 4 colors builds clean with plain build and with --gbc, and a .dmg.png is used by both (tests)
      - many-palette projects give no cross-palette shared_shade warning, and within-image sharing still warns (tests)
      - an 8x8 sheet builds into 8x8 objects with its palette data (test)
      - zqarpL's criteria pass
  - title: 'Observations: profiler names, an OAM log, an audio-log channel filter, a palette RAM view'
    ref: observe
    desc: |
      Why: view stumbles from the example games (project/2026-11-08-examples-dx-review.md, items 7, 9, 12). The shooter called gbcli twice a frame from Python to get OAM over 120 frames.

      What:
      - Profiler names: hardware vectors read as vectors (`$0048` as the STAT vector, not `VBlankVector+$8`), and at an address with several symbols a code label beats a driver's or data symbol (the racer's WRAM `VLoop` came out as hUGEDriver's `end_zero`).
      - `run --oam-log` (like `--audio-log`): per frame, the objects on screen and which were dropped by the 10-per-line limit; `--json` for every field.
      - `--audio-log` filters by channel (`--channel 4` or similar) and can print a summary (writes per register and per writer PC).
      - A palette RAM view (`gbcli view palettes`): BGP/OBP0/OBP1, and on a GBC the 8 BG and 8 OBJ palettes as RGB555 and hex.
      - `gbcli help music` says song note names are an octave above scientific pitch (C-3 sounds as C2, 65 Hz).

      Seams: crates/engine (profile, views), crates/server, crates/cli render and help, crates/protocol. Keep inspection side-effect free.
    criteria:
      - profiler names for a vector and a shared-address label (tests)
      - --oam-log over N frames gives each frame's objects and drops (test, golden)
      - audio-log channel filter and summary (tests)
      - view palettes on DMG and GBC (tests)
  - title: 'Design a pixel-art text format (decision for Ben)'
    ref: pixdesign
    labels: [decision]
    desc: |
      Why: Ben's ruling (2026-11-08, project/2026-11-08-examples-dx-review.md, Decisions): build a pixel-art text format now. Music has song text that agents write and read; images don't, so both example-game agents wrote Python PNG writers.

      What: a dated design doc proposing the format and its tooling, for Ben to approve before it's built. It should be to images what gbcli song text is to music (read `gbcli help music` and crates/music for the song text's conventions: `format gbcli-song 1` first line, a line's first word says what it is, `gbcli song FILE --text/--uge/--piano`, build reading .song in assets/music). Cover: the syntax (a legend of characters to colors or shades, rows of pixels, frames or tiles for sheets, comments), how it maps to every image kind and size convention the build has (bg, sprites NAME.WxH, tiles, .dmg versions, indexed art with assigned palettes), round-trip conversion both ways with a verb (`gbcli pixels`? the doc proposes), whether build reads the text directly alongside PNGs, error rows with line and column, how large images stay readable and editable by an agent (a 256x256 background), and what it deliberately doesn't do (it is not a drawing program; vector work belongs to other tools, which gbcli's help doesn't recommend). Include two worked examples and the open questions for Ben.

      Seams: read crates/build/src/images and crates/music to fit the existing pipeline; propose, don't build.
    criteria:
      - a dated design doc with syntax, mapping to every image kind, the verb, build integration, errors, two worked examples and Ben's open questions
  - title: 'Build the pixel-art text format'
    ref: pixbuild
    blockedBy: [pixdesign]
    desc: |
      Placeholder: implement the format as Ben approves it in the pixdesign leaf's doc. Detail this leaf from that doc before dispatching.
```
