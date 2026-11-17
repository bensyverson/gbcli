# DX review: Teddy & Toys (examples/factory)

2026-11-16, twentieth session. This review reads [2026-11-16-dx-factory.md](2026-11-16-dx-factory.md), written by three game agents across five leaves, plus what the main thread hit while integrating their work. It ranks the stumbles by cause and recommends a fix for each. Each item was checked against the pinned binary (commit 74432d7, rebuilt for dispatch) and [backlog.md](backlog.md) before it was listed. Ben's decisions go at the end.

> **Renamed, 2026-11-16:** the game's folder is now `examples/teddy-and-toys/` (Ben's call), so its ROMs build as `build/teddy-and-toys.gb` and `build/gbc/teddy-and-toys.gbc`. `examples/factory/` and `build/factory.gb` below are the names at the time.

**Ruling (Ben, 2026-11-16): an agent's misreading of gbcli is a flaw in the help or onboarding text.** If an agent was wrong about gbcli, or missed something the help does say, the text failed to reach it where it looked. So no stumble here is dismissed as "my misreading" or "already documented". Each one asks where the agent looked and what it should have found there. The first draft of this review dismissed four stumbles that way; they are now F-9, F-11, F-12 and F-13.

## gbcli: correctness

**F-1. The frame that turns the LCD off counts as lag, not as a screen change (about 6 min for the agent; still open).**
- **Observed:** replaying the bot's round (`python3 tools/bot.py`, then `gbcli run build/factory.gb --session lagprobe --from boot --frames 3700 --keys-file build/bot.keys --keys-since boot --frame-budget --json`, from `examples/factory`) gives `lag: 1`. The lag frame is 3666: `active_t 70204, idle_t 0, lag true, lcd_off false`. Frames 3667-3672 are `lcd_off true`.
- **Cause:** frame 3666 begins with the LCD on, and the game turns it off early in that frame (in VBlank) to draw the results screen. The classifier appears to mark a frame lcd_off only when the LCD is off from its start. `gbcli help budgets` says "A frame the LCD was off for (startup, a screen change) counts as lcd_off, not lag". A reader takes the frame that starts a screen change to be part of it.
- **Cost:** every proof in the game names this one frame as a known exception, and every brief had to say so.
- **Recommendation:** a frame in which the LCD was turned off and never halted while it was on counts as lcd_off. A frame that had the LCD on throughout and never halted stays lag. Pin it with a test ROM that turns the LCD off mid-frame. **Needs Ben** (question 1): it changes what `lag` means for existing callers.

**F-2. `build DIR --out FILE` reads FILE against DIR, not the working directory (main thread, about 2 min; still open).**
- **Observed:** from the repo root, `gbcli build examples/factory --gbc --out examples/factory/teddy-and-toys.gb` wrote `examples/factory/examples/factory/teddy-and-toys.gb`.
- **Cause:** `build --help` says `--src` is "relative to the project", but says nothing about `--out`. `--dir` is documented to read relative paths against itself; the positional DIR is not, yet it acts the same way.
- **Recommendation:** `--out` resolves against the working directory (or `--dir` when given), like every other CLI's output flag. The positional DIR names the project only. Document the rule on both flags. **Needs Ben** (question 2), since it changes behavior for a relative `--out`. `scripts/examples/build-roms.sh` passes absolute paths, so it is unaffected either way.

## gbcli: surprises a row could prevent

**F-3. A session lands in a parent's `.gbcli/` with nothing saying so (about 4 min for an agent, and a stale session left behind; still open).**
- **Observed:** an example's `.gbcli/` is gitignored, so a fresh worktree or clone of `examples/factory/` has none. gbcli walked up to the repo's `.gbcli/`, where a stale `rules` session then refused the next agent's run: `session "rules" runs examples/factory/build/factory.gb, not <worktree>/...`.
- **Today:** the game's `tools/gbcli.py` creates `.gbcli/` itself, and the gotcha (2026-11-07) records the trap.
- **Recommendation:** the row that creates a session names the folder it used (`session=rules created in /abs/.gbcli/`), so a misplaced session shows on first use. A walk-stopping marker was considered and isn't recommended: a project's `.gbcli/` is what already marks the root.

**F-4. A new session on a GBC-capable ROM starts as a DMG with no hint (main thread, about 5 min; still open).**
- **Observed:** `gbcli run build/gbc/factory.gbc --session lookc --frames 3400 --shot …` drew in grays. `show` says `machine model=dmg-b cgb=dual`, but only when asked.
- **By design:** DMG fidelity comes first, and a dual-mode ROM runs correctly on a DMG.
- **Recommendation:** when a session is created on a ROM whose header is dual or CGB-only without `--model`, print a note: `note model=dmg-b: this ROM supports color; --model gbc runs it as a GBC`.

## gbcli: docs and small features

**F-5. Comparing two shots means decoding PNGs (about 6 min; un-parks a backlog entry).**
- **Observed:** identical screens make different files, because `shot` writes the frame into `tEXt` chunks. The agent wrote a PNG decoder (`tools/png.py`) to compare pixels.
- **Backlog:** "The screen's shade indices as data in `shot --json`" (2026-11-09) is to be un-parked when "a second agent decodes `shot` PNGs to compare pixels". This is that second agent.
- **Recommendation:** un-park it, and add a `pixels=` hash of the screen to the `shot` row (and to `--json`). The hash makes "didn't change" one string compare. The shade data serves library and web callers.

**F-6. Per-channel WAV loudness is mostly pops (about 12 min; still open).**
- **Observed:** a volume-15 effect and the volume-4 music had similar peaks. Every DAC switch pops at full scale. Each `--wav --channels each` file also opens with a step when its channel was already sounding as the run began.
- **Recommendation:** a sentence by `--channels each` in `run --help` about both, and a per-channel loudness figure (windowed RMS) in `--audio-summary`. The figure is what a "louder than the music" claim needs.

**F-7. `help images` doesn't say an image's data sits in a switchable ROM bank (about 5 min).**
- **Observed:** Elle drew as garbage after the game's clock code switched banks. Only the `romx_bank` limit row hints at it; the starter's `text.asm` says it for the font.
- **Recommendation:** one sentence in `help images`: each image is in a ROMX section, so on an MBC cart switch to `BANK(label)` before reading it.

**F-8. `--crop` doesn't apply to `--filmstrip` (1 min; nine calls instead of one).**
- **Recommendation:** accept `--crop` with `--filmstrip`, cropping every frame. Small and self-contained.

**F-9. Where a key press lands is in `help time`, not where the agent looked (about 5 min).**
- **Observed:** the agent probed frame by frame to line up a turn strip. `gbcli help time` says that a key token changes the buttons at the start of the run's frame N and that the stop shows the frame just drawn. The agent read `run --help` and the keys recipe, and neither says it or points there.
- **Recommendation:** `run --help`'s `--keys` and `--keys-file` lines carry the one-sentence rule ("K@N is seen by every joypad read after frame N's VBlank; a stop shows the frame just drawn") and point to `gbcli help time`. The bot recipe repeats it.

**F-11. Nothing says only `include/` is searched for INCLUDE (about 3 min).**
- **Observed:** the agent put `tuning.inc` in `src/` because the plan suggested it, and the build failed. `build --help` mentions "with include/ on the include path" in passing. The starter's README and `help rgbds` don't say where a project's own include files belong.
- **Recommendation:** the `gbcli new` README's layout section and `help rgbds` say it: put your own `.inc` files in `include/`, the only folder INCLUDE searches; or add `--include DIR` to the build. (The plan's suggestion was the main thread's error, and the plan doc now says so.)

**F-12. After `power-cycle`, the first read fails while the boot ROM runs (about 2 min).**
- **Observed:** `gbcli power-cycle`, then `run --frames 30 --read 'u16[wBest]'`, answered `uninitialized_read`. The remedy was right. The save recipe says to run 300 frames after a power cycle, but the agent went from `power-cycle --help`, which doesn't.
- **Recommendation:** the `power-cycle` row and `--help` say the machine is at power-on, with the boot ROM to run first, and give the idiom `run --to <your main loop>` before reading the program's state.

**F-13. `--audio-log` text stops at 20 rows (1 min).**
- **Observed:** the jingle's 36 channel-1 writes showed only C6 to G6, then `more`. `run --help` says the rows stop at 20, but the `more` row doesn't say how many are left or how to see them.
- **Recommendation:** the `more` row says how many rows are hidden and names `--json` (every write) or `--piano`. This applies wherever a text view truncates.

**F-10. No cartridge-type guidance for a battery save (about 2 min).**
- **Observed:** the agent wrote `$1B` and `$02` at $0147/$0149 by hand, then checked with `gbcli sav` that rgbfix kept them.
- **Recommendation:** one line in `help rgbds`: rgbfix keeps a cartridge type you write at $0147, and a battery save needs one with a battery, such as $1B (MBC5+RAM+BATTERY).

## Not gbcli

- **Stale review shots** (main thread). After a squash merge, `examples/factory/tools/review_shots.py` shot the ROMs left in `build/` by an earlier build. Ben saw the old art and a toy drawn over Elle's back, which the current code fixes. The tool now refuses a ROM older than its sources (commit 10480a9).
- **The warm suite reads 35-37 s in the pre-commit hook** on commits that changed no Rust. It's filed as a leaf under 9b5RBA, since `job` has no issue tree.
- **`python3 -I` breaks a tool's sibling imports.** The habit comes from the harness's advice for running downloaded files, not from gbcli; the briefs now say to run the tools without `-I`.
- **F-3's and F-4's surprises are also help gaps.** `gbcli help` says sessions live in `.gbcli/sessions/NAME/` but not how the folder is found. Neither `run --help` nor `help hardware` says that a new session is a DMG unless `--model` says otherwise. The F-3 and F-4 leaf adds those sentences beside the rows.

## What the briefs got wrong

- **"make.py passes 24/24" was false in a fresh worktree** because of F-3. The next brief should say the tools create `.gbcli/`; they do now.
- **"Record key files late" didn't apply.** Every run started `--from boot` or at a checkpoint, and `--keys-since boot` replays from boot.
- **The plan's `src/tuning.inc` can't be included,** because only `include/` is on the include path. The plan offered it only as an example; the game uses `include/tuning.inc`.
- **The no-lag criterion needed an exception at the round's end** (F-1).

## Questions for Ben

1. **F-1:** count a frame that turns the LCD off (and never halted while it was on) as lcd_off, not lag? *Recommend yes,* pinned by a test ROM.
2. **F-2:** resolve `build --out` against the working directory (or `--dir`), not the positional project DIR? *Recommend yes.*
3. **F-3 and F-4:** a session-creation row naming the `.gbcli/` it used, and a note when a color-capable ROM starts as a DMG? *Recommend yes,* one leaf.
4. **F-5:** un-park the shade data in `shot --json`, plus a `pixels=` hash on the shot row? *Recommend yes.*
5. **F-6 to F-13:** two leaves? *Recommend yes:*
   - **help and onboarding text:** the WAV sentence (F-6), images in ROMX (F-7), the keys rule in `run --help` (F-9), the cart-type line (F-10), include files (F-11), the boot ROM after `power-cycle` (F-12), and truncated rows that say what's hidden (F-13);
   - **audio and capture:** per-channel loudness in `--audio-summary` (F-6) and `--crop` with `--filmstrip` (F-8).
6. **An issue tree in `job`** for discovered work, which today goes under the plan root? *Recommend yes.*

## Ben's decisions (2026-11-16)

- **Question 4 (F-5): yes, both.** `shot` gives a pixel fingerprint (`pixels=` on the row and in `--json`) and the raw screen as data in `--json`: shade indices on a DMG, colors on a GBC. Both honor `--crop`, so a caller can fingerprint or dump just a region, such as the key column or one belt. The backlog entry "The screen's shade indices as data in `shot --json`" is un-parked and removed from backlog.md.
- **Questions 1, 2, 3, 5 and 6: yes, as recommended** ("I trust your recommendations"). The F-1 classifier change, `build --out` against the working directory, the session-folder and model notes with their help sentences, the help-text leaf and the audio-and-capture leaf, and an issue tree in `job`. The leaves are in the block below, imported under root `9b5RBA` as Stage 15.
- **The game's folder is renamed** to `examples/teddy-and-toys/` (Ben).

## Stage 15: the fixes

Imported into `job` on 2026-11-16 under root `9b5RBA` as `5ot0G5` (leaves Dm8G8G, UltxXY, NQKwil, sL0Oa8, rKKR0I, YZth2Z); after import the tracker is the plan. Every leaf follows the ruling at the top of this doc: when a fix changes what a user sees, the help that names it changes in the same leaf. gbcli's own TDD rule applies (AGENTS.md): tests first, seen failing.

```yaml
tasks:
  - title: 'Stage 15: gbcli fixes from the Teddy & Toys DX review'
    ref: stage15
    desc: |
      Fixes from project/2026-11-16-examples-dx-review-factory.md, decided by Ben on 2026-11-16. That review ranks what three game agents (and the integrator) stumbled on while building examples/teddy-and-toys with gbcli as users. Ben's ruling there binds every leaf: an agent's misreading of gbcli is a flaw in the help or onboarding text, so each fix also changes the text where the agent looked. The six leaves touch different surfaces and can run in parallel, except that the help-text leaf (help) edits help topics the others may also touch; carve by file at dispatch.
    children:
      - title: 'A frame that turns the LCD off counts as lcd_off, not lag (F-1)'
        ref: f1
        desc: |
          Why: the frame budget calls the frame that starts a screen change a lag frame. Teddy & Toys' round end turns the LCD off early in frame 3666 (in VBlank) to draw the results screen; that frame reads lag true, lcd_off false, active_t 70204, while frames 3667-3672 read lcd_off. Every proof in the game had to name the frame as an exception. `gbcli help budgets` says "A frame the LCD was off for (startup, a screen change) counts as lcd_off, not lag", which a reader takes to include the frame that turns it off.

          What: a frame in which the LCD was turned off, and which never halted while the LCD was on, counts as lcd_off. A frame with the LCD on throughout that never halted stays lag. Update `help budgets` to say exactly that.

          Already decided (Ben, 2026-11-16, review question 1). Seams: the frame-budget classifier and its tests; `help budgets`; any golden that pins a frame-budget result. Reproduction: from examples/teddy-and-toys, `python3 tools/bot.py --gbcli <bin>`, then `gbcli run build/teddy-and-toys.gb --session lagprobe --from boot --frames 3700 --keys-file build/bot.keys --keys-since boot --frame-budget --json`.

          Risk: callers that count lag may see a number change; say so in the commit message.
        criteria:
          - a test ROM that turns the LCD off mid-frame, after a frame with no halt, reports that frame as lcd_off; one with the LCD on throughout and no halt reports lag (tests seen failing first)
          - the Teddy & Toys replay above reports lag 0 over 3700 frames
          - help budgets states the rule
      - title: '`build --out` resolves against the working directory, not the project DIR (F-2)'
        ref: f2
        desc: |
          Why: from the repo root, `gbcli build examples/factory --gbc --out examples/factory/teddy-and-toys.gb` wrote `examples/factory/examples/factory/teddy-and-toys.gb`. `build --help` says `--src` is relative to the project but says nothing about `--out`; the positional DIR acts like `--dir`, which is documented to read relative paths against itself.

          What: a relative `--out` resolves against the working directory, or against `--dir` when given. The positional DIR names the project only. `build --help` states the rule for `--out`, `--src` and `--include`.

          Already decided (Ben, 2026-11-16, review question 2). Seams: the build's path resolution (crates/build or the CLI adapter, wherever the decision lives; it belongs in a library crate), `build --help`, CLI goldens. scripts/examples/build-roms.sh passes absolute paths and must keep working.
        criteria:
          - a CLI test runs build with a positional DIR and a relative --out from another working directory, and the ROM lands at cwd/--out (seen failing first)
          - --dir still resolves relative paths against itself, by test
          - build --help states where --out, --src and --include resolve
      - title: 'Say which .gbcli/ a new session uses, and when a color ROM starts as a DMG (F-3, F-4)'
        ref: f34
        desc: |
          Why: two silent surprises. (F-3) A game folder's .gbcli/ is gitignored, so a fresh worktree or clone has none, and gbcli walked up to the repo's .gbcli/; a stale session there then refused the next agent's run, and nothing in any output named the folder in use. (F-4) A new session on a dual-mode or CGB-only ROM starts as a DMG unless --model says otherwise, so the integrator's first GBC shot was gray; `show` reports model= only when asked.

          What: the row that creates a session names the .gbcli/ folder it used (an absolute path). When a session is created without --model on a ROM whose header ($0143) is dual or CGB-only and the model is a DMG, a note row says the ROM supports color and names --model gbc. `gbcli help` says how .gbcli/ is found (the nearest one above the working directory or --dir), and `run --help` says a new session is a DMG unless --model says otherwise.

          Already decided (Ben, 2026-11-16, review question 3). DMG stays the default (DMG fidelity first); this adds a note, not a change of default. Seams: session creation in the engine or bundle crate, the CLI's rows, help text, CLI goldens.
        criteria:
          - creating a session prints the .gbcli/ folder it used, by CLI test
          - creating a session on a dual-mode ROM without --model prints the color note; with --model gbc, or on a DMG-only ROM, it doesn't (tests seen failing first)
          - gbcli help and run --help carry the two sentences
      - title: '`shot` gives a pixel fingerprint and the screen as data, both honoring --crop (F-5)'
        ref: f5
        desc: |
          Why: comparing two shots meant decoding PNGs. Identical screens are different files, because shot writes the frame into tEXt chunks. Teddy & Toys' agent wrote a PNG decoder (tools/png.py), the second agent to do so, which met the un-park condition of the backlog entry "The screen's shade indices as data in shot --json" (now removed from backlog.md).

          What: shot reports a fingerprint of the pixels (pixels= on its row and in --json), equal for equal pixels whatever the frame label. --json also carries the screen as data: shade indices (0-3) on a DMG and colors on a GBC, row by row. Both honor --crop, covering just the cropped region. The PNG keeps its tEXt chunks. shot --help documents the fingerprint (what it hashes) and the data's shape.

          Already decided (Ben, 2026-11-16, review question 4: "both seem attractive ... especially if the agent can crop the raw data output window"). Seams: the shot path in the engine (the decision belongs in a library crate, per the web-target ruling), the protocol type for the shot result (one shared struct), the CLI row, shot --help, the schema. This applies to run --shot too, wherever a shot is taken.

          Open, the builder's call: the hash function (stable across platforms and wasm32; no new dependency without asking), and whether the data is nested rows or a flat array with width and height.
        criteria:
          - two shots of the same screen at different frames give the same pixels= value, and a one-pixel change gives a different one, by test (seen failing first)
          - --json carries the screen's shade indices on a DMG and colors on a GBC, and --crop narrows both the data and the fingerprint, by test
          - shot --help and the schema describe both
      - title: 'Help and onboarding text where the Teddy & Toys agents looked (F-6, F-7, F-9 to F-13)'
        ref: help
        desc: |
          Why: Ben's ruling: an agent's misreading of gbcli is a flaw in the help or onboarding text. Each item below cost an agent time because the text it read didn't say it, even where another topic did.

          What, one change each:
          - F-6: by `--wav --channels each` in run --help: every DAC switch pops at full scale whatever the volume, and each per-channel file opens with a step when its channel was already sounding as the run began, so peak levels mislead (use a windowed RMS past the opening).
          - F-7: help images: each image's data sits in a ROMX section, so on an MBC cart switch to BANK(label) before reading it.
          - F-9: run --help's --keys and --keys-file lines carry the one-sentence rule (K@N is seen by every joypad read after frame N's VBlank; a stop shows the frame just drawn) and point to `gbcli help time`; the bot recipe repeats it.
          - F-10: help rgbds: rgbfix keeps a cartridge type written at $0147, and a battery save needs a type with a battery, such as $1B (MBC5+RAM+BATTERY).
          - F-11: the `gbcli new` starter README's layout section and help rgbds: a project's own .inc files go in include/, the only folder INCLUDE searches.
          - F-12: power-cycle's row and --help: the machine is at power-on and the boot ROM runs first, so run to the program's main loop (`run --to <label>`) before reading its state.
          - F-13: every text view that truncates (the audio log's "more" row first) says how many rows are hidden and how to see them (--json, or --piano for audio).

          Already decided (Ben, 2026-11-16, review question 5). Seams: crates/cli help topics and verb help, the starter template's README, recipes; CLI goldens that pin help text. The help topic links checker (scripts/help/check-doc-links.py) if a link is added.

          Out of scope: the per-channel loudness figure and filmstrip crop (leaf audio).
        criteria:
          - each of the seven texts is in place, and a CLI golden or test pins each changed help text
          - the truncated audio-log row names its hidden count and --json, by test (seen failing first)
      - title: 'Per-channel loudness in --audio-summary, and --crop with --filmstrip (F-6, F-8)'
        ref: audio
        desc: |
          Why: (F-6) proving "the effects are louder than the music" took 12 minutes of WAV analysis, because peaks are dominated by DAC pops and opening steps; a per-channel loudness figure is what such a claim needs. (F-8) `run --filmstrip ... --crop` answers "bad_request: --crop only applies with --shot", so a cropped strip of a turn took nine calls instead of one.

          What: --audio-summary reports each channel's loudness as a windowed RMS (and the mix's), ignoring DAC-switch transients and the opening step, with the window and method stated in run --help. --crop applies to every frame of --filmstrip.

          Already decided (Ben, 2026-11-16, review question 5). Seams: the audio summary in the engine's audio views, the filmstrip capture, the protocol result types, run --help, the schema.

          Open, the builder's call: the RMS window and how transients are excluded; say how in the help.
        criteria:
          - on a test ROM with a loud channel and a quiet one, --audio-summary ranks them by loudness despite DAC switches on the quiet one (seen failing first)
          - --filmstrip with --crop writes a strip of cropped frames, by test
```
