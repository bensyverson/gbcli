# DX review: Teddy & Toys (examples/factory)

2026-11-16, twentieth session. This review reads [2026-11-16-dx-factory.md](2026-11-16-dx-factory.md), written by three game agents across five leaves, plus what the main thread hit while integrating their work. It ranks the stumbles by cause and recommends a fix for each. Each item was checked against the pinned binary (commit 74432d7, rebuilt for dispatch) and [backlog.md](backlog.md) before it was listed. Ben's decisions go at the end.

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
