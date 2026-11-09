# DX notes: the shooter (Vesper Swarm)

Developer-experience notes from the agents building `examples/shooter/` with gbcli as users (gbcli at commit 4381e6a, the pinned release binary). One dated section per session, appended. Stumbles are ranked by the time they cost.

## 2026-11-08, Shooter 1: the multiplexer and the parallax spike (agent shooter14)

**What I built.** `gbcli new examples/shooter`, then a test scene: 48 bullets (more than OAM's 40), 16 of them on one line against the hardware's 10, each with its own tile number so OAM views can tell them apart. A sprite multiplexer (`src/sprites.asm`): game code adds up to 64 sprites a frame to a list; the flush fills shadow OAM forward from a start S on even frames and backward from S-1 on odd frames, stepping S back by 11 after each backward frame. A parallax sky (`src/parallax.asm`): four background bands at 1/8, 1/4, 1/2 and 1 px a frame, split by LY=LYC interrupts on lines 39, 87 and 119 whose handler waits for STAT mode 0 before writing SCX, plus the window as a fixed status bar from line 136. The game is named Vesper Swarm.

**Evidence** (all from `examples/shooter/`, binary `/Users/ben/.cache/gbcli-examples/target/release/gbcli`):

```sh
gbcli build                                         # errors=0 warnings=0
gbcli run build/shooter.gb --session mux --frames 120 --checkpoint warm
python3 -I tools/oam_coverage.py --session mux --frames 120
#  objects=48 frames=120 drawn_some_frame=48 drawn_per_frame_min=30
#  drawn_per_frame_mean=34.0 longest_undrawn_run=2 never_drawn=none
gbcli run --session mux --at warm --frames 600 --frame-budget
#  frame_budget frames=600 lag=0 active_min=25456 active_max=28900 active_mean=25760.0
gbcli run --session mux --at warm --frames 2 --raster
#  rSCX writes at line 39, 87 and 119 every frame (into=620 of 912 ticks), and
#  the top band's in VBlank at line 145
gbcli run build/shooter.gb --session split --frames 120 --checkpoint warm
gbcli break StatSplit.write --session split --if '([rSTAT] & 3) != 0 || (ly != 39 && ly != 87 && ly != 119)'
gbcli run --session split --frames 600              # never stops: every split write is in mode 0 on its line
gbcli break StatSplit.write --session split --if '([rSTAT] & 3) == 0'
gbcli run --session split --frames 1 --read ly --read '[rSTAT] & 3'   # control: stops, ly=39, mode 0
```

`tools/oam_coverage.py` steps the session a frame at a time and reads `gbcli view oam --json` after each: an object counts as drawn when OAM has it on screen and `dropped_by_line_limit` is false. The first multiplexer (a forward/backward flip with S stepped *on*) left some bullets undrawn for 8-10 frames in a row; the tool found it, and stepping S *back* (so the sprites one frame leaves out lead the next) brought the worst run to 2.

**Where gbcli helped** (only what saved real time):

- `gbcli new`: a starter with OAM DMA in HRAM, a VBlank handler, shadow OAM, hUGEDriver wired and a GBC path that already builds. I changed the program, not the plumbing. Probably an hour saved.
- `gbcli view oam --json` computes `dropped_by_line_limit` per object. Without it I would have had to model the DMG's 10-per-line OAM scan myself to prove anything about flicker.
- `--frame-budget` answers "any lag frames?" in one row, and `break ADDR --if EXPR` with `[rSTAT]` and `ly` in the expression turned "every write lands in HBlank" into a run that must not stop.
- `gbcli help hardware` gives mode 3's length range (172-289 dots) and what lengthens it, on one page.

**Stumbles, by cost:**

1. **No way to prove an HBlank write from `--raster` alone (about 20 min).** `gbcli run --session mux --at warm --frames 2 --raster` prints `raster_write reg=rSCX value=$1D pc=$09CE into=620` and then `raster_unsupported feature=ppu_mode_events`. `into=620` ticks is 310 dots: past mode 3 on an empty line (80 + 172 = 252 dots) but not past the worst case (80 + 289 = 369), so the number can't prove the write missed mode 3 on a line with sprites. I expected a `mode=` column on each write (the help's own traps say "write in mode 0"). Workaround: a conditional breakpoint on the write instruction that stops if `[rSTAT] & 3` is non-zero, plus a control breakpoint that does stop. It works, but a user has to invent it; a racer-style per-line game has no single write instruction to break on.
2. **Every rebuild meant deleting the session (about 10 min over the session, 4 rebuilds).** After `gbcli build`, `gbcli run --session mux --frames 1` fails: `rom_changed: the ROM changed under the saved state: RAM symbol wSprRot: 00:C1C4 -> gone; …`. `gbcli sessions rm mux` then refuses: `session "mux" has 1 named checkpoint(s): warm`; `--with-checkpoints` works. I expected one flag to restart an existing session from boot on the new ROM (`run --from boot` only applies "when there is no head"). `build --session NAME --restore CP` is the recipe, but it refuses for the same reason whenever the code moved, which during a spike is every build.
3. **No multi-frame OAM view (about 10 min).** Proving "every object is drawn in some frame" needs OAM per frame over many frames; gbcli shows one frame per `view oam`. I wrote `tools/oam_coverage.py`, which calls gbcli twice a frame (120 frames: 240 processes, a few seconds). An `--oam-log` on `run` (like `--audio-log`) or `view oam --frames N` would make the criterion one command.
4. **`dropped_by_line_limit` is one flag per object (5 min).** A sprite dropped on some of its 8 lines but drawn on others (a bullet straddling a crowded line shows as a fragment in the screenshot) reads as dropped. My tool therefore under-counts what is drawn, which is the safe direction, but the view doesn't say which lines dropped it.
5. **The profiler names the STAT handler `VBlankVector+$8` (2 min).** `gbcli run --frames 60 --profile` printed `routine name=VBlankVector+$8 addr=$0048`: the STAT vector's section had no label of its own, so the nearest earlier label won. My fault, fixed with a label, but naming by section (`STAT vector`) would have been clearer.
6. **`--frame-budget` from boot counts the start-up frames as lag (2 min).** `lag=2` at frames 0 and 1 (boot handoff and the LCD-off init). Correct, but the first reading looks like a failure; I measure from a checkpoint past start-up.

**Missing:** a PPU mode per raster write; restarting a session on a rebuilt ROM; an OAM log over a run.

**`gbcli gaps`:**

```
gap feature=ppu_dot count=2
gap feature=ppu_mode_events count=2
```

## 2026-11-08, Shooter 2: the stage (agent shooter14)

**What I built.** The game loop on top of the spike (`src/scene.asm` is gone): a title screen, the stage, and the two result screens, switched with the LCD off in VBlank (`src/game.asm`); the ship, 2 px a frame, with autofire on A and a 4x4 hitbox (`src/player.asm`); up to 24 enemy bullets in 16 directions at two speeds, with aimed shots, fans and rings (`src/bullets.asm`); three enemy kinds (a dart that fires one aimed shot, a weaver on a sine, a pod that stops and fires two rings), a timeline of 65 spawns over the first 35 seconds, and a 24x24 boss with 48 HP that rides up and down, alternates an aimed fan of five with a fast ring, and adds aimed shots below half HP (`src/enemies.asm`); score in BCD, three lives, a status bar on the window, and a font drawn from a 1bpp table (`src/font.asm`). Placeholder sprite tiles are in `src/gfx.asm`. Getting hit costs a life and clears the bullets on screen. Stage length: the boss arrives at stage frame 2340; the bot's run clears at frame 4617 since boot (about 77 s).

A blind input script (hold A, sweep the ship top to bottom) died on the first weaver that crossed its column, so the winning run is recorded by an autopilot: `tools/bot.py` drives a fresh session four frames at a time, reads the ship, enemies, boss and bullets from RAM with `--read`, scores nine moves over the next 20 frames, and writes each key change at its frame since boot to `keys/win.keys`. The emulator is deterministic, so replaying the file plays the same game.

**Evidence** (from `examples/shooter/`):

```sh
gbcli build                                                    # errors=0 warnings=0
python3 -I tools/bot.py --out keys/win.keys                    # result=stage_clear frame=4618 lives=3 (16 s)
gbcli run build/shooter.gb --session win --frames 6000 --keys-file keys/win.keys \
    --until '[wState] == 2' --read '[wLives]' --read 'u16[wStageT]'
#  stop predicate … frame=4617 text="[wState] == 2"; wLives=3, wStageT=4574
gbcli run --session win --frames 20 --shot build/win.png       # STAGE CLEAR, SCORE 020750
gbcli run build/shooter.gb --session over --frames 3000 --keys 'START@42 -START@44' \
    --until '[wState] == 3' --read '[wLives]'
#  stop predicate … frame=1039 text="[wState] == 3"; wLives=0 (no input: game over)
gbcli run build/shooter.gb --session budget --frames 6000 --keys-file keys/win.keys \
    --until '[wState] == 2' --frame-budget --json > build/budget.json
python3 -I tools/budget_peak.py build/budget.json
#  frames=4618 lag=5 lag_frames=0 1 40 41 4616
#  busiest_frame index=3389 active_t=46308 share=65.9%
#  busiest_window start=3389 frames=60 mean_active_t=33476 share=47.7% lag=0
```

The five lag frames are start-up (0, 1) and the two screen changes, which turn the LCD off on purpose (40, 41: title to stage; 4616: stage to result). No frame of play lags; the busiest, in the boss fight with 34 sprites in the list, uses 66% of the frame.

**Where gbcli helped:**

- Determinism plus `--keys-file`: a recording made four frames at a time over 1,150 separate `gbcli run` calls replays bit-for-bit as one run. That turned "a scripted run beats the boss" from hand-tuning a key script into writing a bot once.
- Held keys persist across runs (`--keys 'UP@0'`, then a run with no keys, still reads `hPad=$40`). Not documented that I could find, but it is what made the bot's chunked driving simple.
- `--until '[wState] == 3'` with `--read` at the stop: each criterion is one command.
- `--read` takes `u16[wBullets+8]`, so the bot reads 8.8 fixed-point values directly; 170 reads a call cost nothing noticeable (the whole bot run took 16 s).

**Stumbles, by cost:**

1. **`key_unapplied` floods the output (about 15 min, and a lot of context).** A run that stops (`--until`, `--frames`) before the keys file ends prints a `warning:` line naming every remaining event and then one `key_unapplied event=…` row per event, on stdout. My first stopped run over a 6,000-frame keys file printed ~320 lines; a mid-run screenshot at frame 3400 printed 290 more. I expected one summary row (`keys_unapplied count=290 first=DOWN@3402`). Workaround: `| grep -v key_unapplied` and `2>/dev/null` on every such run.
2. **A failed `--read` throws the whole stop away (5 min).** The same run ended with `gbcli: uninitialized_read: uninitialized read of wBossHp ($C1ED)` and exit 3: the stop and the other reads printed, but as an error. `wBossHp` is first written when the boss arrives, so reading it earlier is a fair question with a fair answer ("never written"). I expected that one read to say so and the rest to succeed. Workaround: `--allow-uninit` in the bot.
3. **A keys file can't be continued from a checkpoint (10 min).** To measure only the gameplay frames I wanted `run --at play --keys-file keys/win.keys` to pick up the recording where the checkpoint is. Offsets count from the run's start, so the rest of the file would replay shifted. Workaround: measure the whole run from boot and filter the lag frames in `tools/budget_peak.py`. A `--keys-since boot` (offsets from the time base, like `--since`) would fix it.
4. **`--frame-budget`'s summary row counts transition frames (5 min).** `frame_budget frames=4617 partial=1 lag=5 … active_max=70204 busiest=40`: the busiest frame it names is the LCD-off frame of a screen change, so the summary can't answer "the busiest stretch of play". I parse `--json` instead (`tools/budget_peak.py`). Excluding LCD-off frames from `busiest` (or listing them as `lcd_off` rather than `lag`) would make the row useful as-is.
5. **Garbage on the title screen's top row (5 min, my bug).** `wHudDirty` was never cleared, so VBlank copied uninitialized WRAM to the window. gbcli's uninitialized-read error exists for `--read`; a warning when the *program* reads never-written WRAM would have pointed straight at it.
6. **Rebuilds still cost a session each (2 min this time).** Same as session 1's second stumble: after each rebuild, `run --at title` refuses with `rom_changed`, so it is `sessions rm NAME --with-checkpoints` and a fresh run.

**Missing:** a short form of `key_unapplied`; per-read errors; keys offsets from a time base.

**`gbcli gaps`:**

```
gap feature=ppu_dot count=2
gap feature=ppu_mode_events count=2
```

## 2026-11-08, Shooter 3: art, music and sound (agent shooter14)

**What I built.** `tools/draw_art.py` (standard library only) draws ten PNGs: the sky as four band images (`assets/bg/sky0-3.png`, 256 px wide, each tiling every 64 or 32 px so its band can scroll and wrap: stars, snowy peaks, forest, ground), the title screen (`assets/bg/title.png`: logo, ship, a planet), and five 8x8 tile sheets (`assets/tiles/font.png`, `ship.png`, `enemies.png`, `bullet.png`, `boss.png`). All of it is indexed art: `assets/palettes.txt` pins the DMG shades with a dmg line and assigns each image a palette whose four colors fall one to each shade, so one set of tiles serves both machines and every `*_BGP` is `%11_10_01_00`. On a GBC the sky gets one BG palette per band (the attribute map is set by band rows), text its own, and sprites five OBJ palettes (ship, enemy, shot, boss, an all-white hit flash) picked by OAM bits 0-2, which the DMG ignores. Screen art is loaded with the LCD off when the screen changes: the sky's 53 tiles or the title's 105 share tiles $70-$FF. Two songs as gbcli song text: `assets/music/title.song` (all four channels) and `assets/music/stage.song` (channels 1-3; channel 4 left empty and muted in the driver with `hUGE_mute_channel`). Three effects on channel 4 (`src/sfx.asm`): shot, hit, explosion, with priority. The placeholder font and sprites in asm are gone.

**Evidence** (from `examples/shooter/`):

```sh
python3 -I tools/draw_art.py
shasum assets/bg/*.png assets/tiles/*.png > build/art1.sha
python3 -I tools/draw_art.py && shasum -c build/art1.sha      # 10 files, all OK
gbcli build                                   # errors=0, images converted=10 warnings=4 (shared_shade, see below)
gbcli build --gbc --out build/shooter-gbc.gb  # errors=0, same 4 warnings
gbcli run build/shooter.gb --session audio --frames 640 --piano build/title-roll.png --audio-log
#  audio since=boot writes=340: the title theme on squares 1-2, wave and noise
gbcli run build/shooter.gb --session stage --frames 300 --keys-file keys/win.keys
gbcli run --session stage --frames 600 --audio-log --piano build/stage-roll.png > build/stage-audio.txt
grep NR43 build/stage-audio.txt | sort | uniq -c   # 73 shots ($0B), 2 explosions ($67)
grep 'NR4[1-4]' build/stage-audio.txt | sed 's/.*pc=//' | cut -d' ' -f1 | sort | uniq -c
#  every channel-4 write in the stage is from SfxPlay ($1595-$159E): the music never touches it
python3 -I tools/bot.py && gbcli run build/shooter.gb --session win --frames 7000 \
    --keys-file keys/win.keys --until '[wState] == 2'   # still clears: frame 4762, 3 lives
```

**Where gbcli helped:**

- The image pipeline's problem rows are exact: the first build named every color, its pixel count and the tiles to fix, and the shade each color took. Choosing palettes so each color lands on its own shade took two builds, because the rows say which shade each color fell to.
- `images.inc` constants (`SKY1_TILES`, `SKY1_HEIGHT`, `TITLE_WIDTH`, `PaletteForest`) meant no tile counts or offsets are hand-maintained; redrawing a band changes its tile count and the code follows.
- `gbcli song FILE` checks a song in milliseconds with line and column (`` `A-2` is not a note … notes are C-3 to B-8 ``).
- `--audio-log` decodes every write with the writer's PC, which proved the effects own channel 4 in one grep.

**Stumbles, by cost:**

1. **Color art can't be built for the DMG, so "DMG first, GBC too" forces indexed art (about 20 min).** My first sky was color art (12 colors, no palette assigned), as `gbcli help images` describes: "gbcli packs its GBC palettes and derives its DMG version". Plain `gbcli build` refused it: `problem limit=dmg_palette severity=error image=assets/bg/sky.png count=12 max=4 … redraw it in at most 4 colors`. Only `--gbc` derives the DMG version, and a `.dmg.png` is "an error without --gbc". So a game that must build both ways can't use color art at all. I expected plain `build` to derive the same DMG version `--gbc` does. Workaround: split the sky into one indexed image per band, so the GBC still gets four palettes.
2. **No 8x8 sprite sheets (about 10 min).** `assets/sprites/` makes 8x16 objects only (`obj_pair_8x16`), and the game runs 8x8 objects: bullets as 8x16 objects would double the line pressure the multiplexer fights. Workaround: sprite tiles as `assets/tiles/` tilesets, keeping every tile distinct so none is merged away and the tile numbers hold. Then I load the OBJ palettes by hand from the named-palette labels, since tilesets get no OBP or OBJ palette data.
3. **`shared_shade` warnings repeat in full every build and can't be resolved (about 10 min, plus output noise).** With ten palettes, the four project-level warnings ("the project draws #e0f8c8 …, #e8ecf8 … in one shade, shade 0") are true by design: on a DMG every palette's lightest color *is* white. The remedy ("declare each shade's color on the dmg line") can't apply to 10 colors per shade. Each is a 700-character line on every `build`, and a `grep sky2` on the build output matched all four. I expected a way to say "these palettes are meant to share shades", or one short row per shade. Workaround: `grep -v shared_shade`, and explain them in the README.
4. **`--audio-log` is unbounded (5 min, and a lot of context).** 640 frames of a four-channel song is 340 rows; I filtered with the wrong pattern and read them all. A `--channel 4` filter or a summary row (writes per register, per PC) would cover what I needed.
5. **Song notes and log notes differ by an octave (5 min).** `E-5` in the song text logs as `E4 -1.6c (329.33 Hz)`: hUGETracker names notes an octave higher than scientific pitch. The help says notes are "C-3 to B-8" but not that C-3 sounds as C2 (65 Hz). One line in `gbcli help music` would settle it.
6. **My old placeholder `ColorGbc` loaded palettes after the art did (5 min, my bug).** The GBC screenshot had a white sky and swapped ship and enemy colors; `view` has no palette RAM view I could find to check what the BG palettes held. I found it by reading my own code. A `gbcli view palettes` would have shown slot 0 holding the wrong colors.

**Missing:** DMG derivation for color art without `--gbc`; 8x8 sprite sheets; a palette RAM view; a filter on `--audio-log`.

**`gbcli gaps`:**

```
gap feature=ppu_dot count=2
gap feature=ppu_mode_events count=2
```

## 2026-11-08, Shooter 4: polish, the GBC build and verification (agent shooter14)

**What I did.** Closed the loop (title, stage, result, START back to the title, checked on both results); brought the sprite test back as a mode on the real ROM (SELECT on the title, `src/scene.asm`), so the multiplexer claims rerun on the shipped game; made the pods and the boss fire full 16-way rings and raised the bullet pool to 31, so play really goes past 40 sprites (41 at frame 1508); split each ring over two frames after the frame budget showed a 16-bullet ring frame at 99.2% of the frame; re-recorded `keys/win.keys`; wrote `examples/shooter/README.md`, whose every claim has its command, and reran all of them on the final build. The same keys file clears the GBC build on `--model gbc` too.

**Where gbcli helped:** `gbcli break ADDR --rom FILE` creates the session with the breakpoint already in it, so a check can cover a whole recorded run from boot (the HBlank check now spans the 4,552-frame winning run). `--frame-budget --json` caught the near-lag ring frame the moment the rings got bigger.

**Stumbles, by cost:**

1. **The frame budget's lag frames mix intent and failure (10 min).** After adding screen loads, `lag_frames=0 1 2 3 40 41 42 43 4551`: every one is an LCD-off frame of a screen change, which is fine, but nothing in the output says so. I checked each by hand against the state changes. A per-frame `lcd_off` flag (`run --frame-budget --json` has `active_t`, `idle_t`, `lag` only) would make "no lag in play" one filter.
2. **No game-object identity for OAM coverage (10 min).** In the real game every bullet shares one tile, so `tools/oam_coverage.py` can't tell them apart; I put the old test scene back behind SELECT rather than prove coverage in play. That's a reasonable design, but it means the 48-bullet proof is about the test mode, not the stage.
3. **Shell loops are refused in the worktree (2 min).** A `for n in 36 38` loop over two gbcli runs was refused by the harness, not gbcli; noted only because it shaped how I searched for the sprite peak.

**`gbcli gaps`:**

```
gap feature=ppu_dot count=4
gap feature=ppu_mode_events count=4
```

## Summary: the whole game (agent shooter14, sessions 1-4)

**The five costliest stumbles:**

1. **Proving "writes land in HBlank" had no direct command** (session 1, ~20 min). `--raster` gives the line and ticks into it, then `raster_unsupported feature=ppu_mode_events`; the tick count can't prove a write missed mode 3 on a line with sprites. The workaround, a conditional breakpoint reading `[rSTAT] & 3`, works only because my splits have one write instruction.
2. **Color art can't build for the DMG** (session 3, ~20 min). Plain `gbcli build` refuses an image with more than 4 colors and no palette; only `--gbc` derives the DMG version. A game that must build both ways is pushed to indexed art.
3. **Output floods: `key_unapplied`, `shared_shade`, `--audio-log`** (sessions 2-3, ~30 min together, and much of my context). A stopped run lists every unapplied key event twice; every build repeats four 700-character warnings I can't resolve; an audio log of 10 seconds is 340 rows. Each wants a one-row summary by default.
4. **Every rebuild costs the session** (all sessions, ~15 min together). `rom_changed` refuses the old head and checkpoints; `sessions rm NAME` refuses while checkpoints exist; there is no "restart this session from boot on the new ROM".
5. **No 8x8 sprite sheets** (session 3, ~10 min). The sprite pipeline is 8x16 only; 8x8 objects go through `assets/tiles/` with hand-loaded OBJ palettes and care that no tile is merged away.

**The three things that helped most:**

1. **Determinism with `--keys-file` and `--read`.** An autopilot driving 1,100+ separate four-frame runs, reading RAM each time, produced a keys file that replays bit for bit as one run, on the DMG and the GBC alike. That made "a scripted run beats the boss" a solved problem rather than a tuning job.
2. **`gbcli view oam --json` with `dropped_by_line_limit`.** The multiplexer's whole claim rests on it: without gbcli computing the 10-per-line drop, I'd have had to model the PPU's OAM scan myself.
3. **The image pipeline's problem rows and `images.inc`.** Exact colors, pixel counts, tiles and shades in each problem, and every tile count, size and palette as a constant, so the art could change freely without touching the code.
