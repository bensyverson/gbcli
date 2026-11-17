# DX: Teddy & Toys (examples/factory), as a gbcli user

The game agent `factory` builds Ben's design (project/2026-11-16-teddy-and-toys-plan.md) using only gbcli's help, recipes, `--help` and the README. Binary: `/Users/ben/.cache/gbcli-examples/target/release/gbcli` at commit 74432d7. Paths below are relative to `examples/factory/`; every gbcli command also took `--dir <worktree>/examples/factory`, left out here for readability.

## 2026-11-16 — Factory 1: the belts, the still key column, the clock hand

**What was built.** The screen from the sketch with placeholder art. The bands are a HUD on lines 0-55, the Finished belt on 56-87, the floor on 88-111 and the parts belt on 112-143. The two belts scroll in opposite directions at 96/256 px a frame by LY=LYC splits: LYC is set to the line before each band, and the handler waits for that line's HBlank before it writes SCX. The key column is the window at WX=143 from WY=0, so it never scrolls. The HUD sits left of it, which is how the layout lives with the window's "everything right of WX, below WY" rule. The clock is 4x4 tiles drawn from 60 pre-made faces (tools/gen_clock.py, in a ROMX bank, one face per 256-byte page). It is double-buffered: the hidden buffer is copied 64 bytes a VBlank, then the map is flipped. The face turns red at 10 s left on a GBC. The cart is MBC5+RAM+battery, ready for Factory 2's best score. The tuning file is `include/tuning.inc`. The toys are data (`data/toys.txt` plus `art/toys/*.pixels`, turned into `include/gen/toys.inc` and `src/gen/toys.asm` by `tools/gen_toys.py`), because the key column needed icons now. Elle has a placeholder 16x48 sheet, `assets/sprites/elle.16x48.pixels`, encoded by gbcli build.

**Evidence: one re-runnable command.** `python3 tools/prove_screen.py --gbcli <gbcli>` runs 13 gbcli calls in about 40 s and prints:

```
PASS bands: HUD 0, Finished 144, floor 0, parts 111 (one SCX per band, every line)
PASS hblank: every scroll write landed in HBlank or VBlank
PASS opposite: parts SCX 111->179 (moved 68, want 68), Finished SCX 144->76 (moved 188, want 188) in 180 frames
PASS key: key column x 136-159 identical, belts changed (build/prove/key-*.png, belts-*.png, full-*.png)
PASS clock: hand step 4 -> 7 (want +3)
PASS budget: 3720 frames from boot, lag frames 0, LCD-off frames 3, busiest frame 5312 of 70224 T active
6/6 claims pass; gbcli calls 13
```

The gbcli commands underneath:
- bands: `gbcli run --session prove --at early --frames 2 --raster --json` → `result.raster.drawn[-1].scx`: lines 0-55 are 0, 56-87 are 144, 88-111 are 0, 112-143 are 111. The same at `--at later` (180 frames on) gives 76 and 179.
- hblank: the same run's rows. `raster_writer reg=rSCX pc=$009C writes=6 hblank=6` and `reg=rSCX pc=$1246 writes=2 vblank=2`. LYC writes land in `drawing`, which is harmless because LYC isn't drawn.
- key and belts: `gbcli run --session prove --at early --frames 0 --shot build/prove/key-early.png --crop 136,0,24,144`, the same `--at later`, compared pixel by pixel. The belts use `--crop 0,56,136,88`.
- clock: `--read '[wClockStep]'` at both checkpoints.
- budget: `gbcli run build/factory.gb --session prove --from boot --frames 3720 --frame-budget --json` → `frame_budget.lag == 0`.
- GBC red zone: `gbcli run build/gbc/factory.gbc --session f1gbc --model gbc --frames 3060 --shot build/prove/gbc-red.png` shows only the face red.
- Battery: `gbcli sav build/prove/f1.sav --session prove` → `sav bytes=8192 ram_bytes=8192`, so rgbfix kept the hand-written $1B cart type.

Both builds: `gbcli build` and `gbcli build --gbc`, errors=0 warnings=0.

**Stumbles, by time cost.**

1. **Shots of identical pixels are different files (about 6 min).** I compared `build/prove/key-early.png` and `key-later.png` by bytes, and the claim failed although the pictures were identical. `strings` shows why: gbcli writes `tEXt` chunks `gbcli:frame` ("complete frame 480 since boot ...") and `gbcli:detail` into every PNG. I expected byte-identical files for identical screens, because that is the cheapest "didn't change" check there is. I wrote `tools/png.py` (palette PNG decoder, stdlib zlib) to compare pixels instead. `gbcli shot --help` documents the palette ordering but not the text chunks. A `--no-meta` flag, or a pixel hash in the shot row, would remove the need for a decoder.
2. **Images land in ROMX, and nothing in `help images` says so (about 5 min).** Elle drew as garbage objects because `ElleFrames` sits in a ROMX bank, while my clock code had switched to its own bank. `text.asm`'s header warns about the font ("The font is in ROMX: on a cartridge with an MBC, switch to BANK(MainFontGlyphs) first"). `gbcli help images` lists the labels but not their section type, so on an MBC cart every image needs a `BANK()` switch. gbcli made the diagnosis fast, which is praise: `gbcli view oam` showed 40 garbage objects, and two calls showed the clear happened but the draw didn't (`gbcli run --to ElleDraw`, then `gbcli view memory wShadowOAM 32`). The fix was `include/bank.inc` (SWITCH_BANK records the bank in HRAM; the VBlank handler restores it).
3. **`src/` isn't on the include path (about 3 min).** The plan suggests `src/tuning.inc`, but rgbasm resolves INCLUDE only against `include/`, as `gbcli build --help` says ("with include/ on the include path"). I moved `tuning.inc` and `layout.inc` to `include/`. This was my misreading, not a gbcli fault; the build diagnostics named the file and line at once.
4. **No cartridge type in `gbcli build` (about 2 min).** hardware.inc 6 has no cart-type constants, and `gbcli build --help` has no flag for one. I wrote `$1B` and `$02` at $0147/$0149 in the header by hand and checked with `gbcli sav` that the battery is seen. The recipes do say a cart needs a battery type for a save. One line in `help rgbds` saying rgbfix keeps a hand-written $0147 would have saved the check.
5. **`python3 -I` broke my own tools' imports (1 min).** This was mine: I followed the harness advice for untrusted files, and isolated mode drops the script's directory from sys.path. The tools are run without `-I`.

**What saved real time.**
- `--raster --json`'s `drawn` array is exactly the proof the criterion asks for, one SCX per line, with no interpretation. The `raster_writer` rows' `hblank=6` settled "writes stay in HBlank" in one call.
- `--frame-budget --json` reporting `lag: 0` across 3720 frames in one call, about 15 s of wall time.
- `gbcli new` gave a working VBlank, OAM DMA, the font routines and a GBC color path to start from. Moving it to MBC5 and STAT splits took minutes.
- Build diagnostics with file and line. rgbasm's deprecation warning on `"0"` (use `'0'`) came through as a row too.

## 2026-11-16 — Factory 2: the shift, built to be tuned

**What was built.** The whole one-minute round with placeholder art.

- **The title.** A placeholder title shows the best score and waits for START.
- **The belts.** Each belt is 16 slots of 16 px. A piece is four background tiles, so the hardware scroll carries it and Elle's overlap with the Finished belt costs no objects (6 per line at most: Elle plus two held halves). A slot is written only while it is hidden under the key column or off the screen's edge. As each slot comes out from under the key column, the supply deals it a piece or a gap (`src/supply.asm`). The supply's skew has three knobs: SUPPLY_CHANCE, SUPPLY_MATCH_CHANCE, and a forced match after SUPPLY_MATCH_MAX_GAP misses.
- **The controls.** The controls table is `src/play.asm`, one routine a row. The B+A chord waits CHORD_FRAMES frames either way.
- **The round's end.** The score box counts in BCD. When the clock runs out, a results screen shows "Toys made", "Best" and "New best!". The best is kept in MBC5 battery RAM (`src/save.asm`, a "TOYS" mark, then the BCD best).
- **The buzz.** A placeholder noise burst (`src/sound.asm`) marks refused presses, so a playtester can hear them.
- **The tuning file.** `include/tuning.inc` holds 14 numbers.
- **One command.** `python3 tools/make.py` regenerates both generated files, builds both ROMs and runs the rules check, in about 3 s.

**The rules check.** `python3 tools/rules_check.py --gbcli <gbcli>` runs 24 rules in 31 gbcli calls and 0.7 s of wall time. Each scenario is one call: `gbcli run --session rules --at play --frames N --poke ... --keys ... --read ... --json`. Run on the final build, it prints `24/24 rules pass; 31 gbcli calls, 0.7 s`, one row per rule: grab, match, mismatch, same half, nothing in front, out of reach, B returns, the half keeps moving, B returns the last of two, the chord B-then-A and A-then-B inside the window, same frame, outside the window, the chord facing Finished, drop and score, the four "anything else" buzzes, Up/Down, the round's end, and the best saved across a power cycle.

Broken on purpose: I replaced the mismatch refusal in `ActionA` (`jp nz, Buzz` after `cp b`) with a `nop`, then ran `python3 tools/make.py`. The check failed with exit 1:

```
FAIL A refuses a half that doesn't match, with a buzz: [wHeldCount] = 2 (want 1); [wPartsSlots+8] = 0 (want 4); [wBuzzes] = 0 (want 1); [wGrabs] = 1 (want 0)
FAIL A refuses the same half again, with a buzz: [wHeldCount] = 2 (want 1); [wPartsSlots+8] = 0 (want 1); [wBuzzes] = 0 (want 1)
22/24 rules pass; 31 gbcli calls, 0.7 s
make: rules check failed
```

The line was restored and all 24 pass again. TDD note: I wrote the check after the game code, so its scenarios were never seen red one by one. The only red on its first run was the check's own save scenario (my bug, below). The deliberate break above is the evidence that the check can fail.

**The bot.** `python3 tools/bot.py --gbcli <gbcli>` plays a closed loop. Each step reads the hands, facing, the parts belt's 16 slots and its 8.8 scroll. It computes how many frames until the next wanted piece is centered on Elle, then runs exactly that far with `A@t`, chords with `B@1 A@1`, or turns, drops and turns back with `UP@1 B@2 DOWN@4`.
- **Round 1:** score 8, results screen at frame 3680. 112 moves, **115 gbcli calls, 3.7 s wall** for the round, and 123 calls and 6.6 s with the checks below.
- **Best kept:** `u16[wBest] = 8`, `wNewBest = 1`, shown in `build/prove/bot-results.png`.
- **Power cycle:** `gbcli power-cycle --session bot`, then `gbcli run --session bot --to MainLoop --read 'u16[wBest]'` gives 8. `gbcli sav build/prove/bot.sav --session bot` writes the 8 KiB save.
- **Round 2:** left to run out, score 0, best 8 and no "New best!" (`build/prove/bot-results2.png`).
- **Keys:** every press is logged at its frame since boot in `build/bot.keys`.
- **Replay and frame budget:** `gbcli run build/factory.gb --session replay --from boot --frames 3663 --keys-file build/bot.keys --keys-since boot --frame-budget --read 'u16[wScore]'` gives `lag=0 lcd_off=6 active_max=9000` (13% of a frame) for the whole played round. The same replay to frame 3690 reaches the results screen with `u16[wScore] = 8`, so the replay is frame-exact.

Both builds: `gbcli build` and `gbcli build --gbc`, errors=0 warnings=0. `tools/prove_screen.py` still passes 6/6 on the full game: it now presses START at frame 60, and its budget claim covers the idle round, 3660 frames, lag 0.

**Stumbles, by time cost.**

1. **The round's last frame counts as a lag frame (about 6 min).** The full replay, `... --frames 3690 --frame-budget`, gave `frame_budget frames=3690 lag=1 lcd_off=10 ... busiest=3664` and `frame_lag from=3664 to=3664 count=1 active_max=70204`. Frame 3664 is the one where `EndRound` turns the LCD off (in VBlank) and draws the results screen with TextDraw. `gbcli help budgets` says "A frame the LCD was off for (startup, a screen change) counts as lcd_off, not lag", so I expected this frame to count as lcd_off. The round start's screen change, also LCD-off drawing, did count as lcd_off. I haven't found out why the two differ. I measured the round's own frames instead (`--frames 3663`: lag 0) and recorded it here rather than restructuring the game around the classifier.
2. **`power-cycle` starts at power-on (about 2 min).** After `gbcli power-cycle`, `gbcli run --frames 30 --read 'u16[wBest]'` answered `error=uninitialized_read message="uninitialized read of wBest ($C190)" remedy="... read it after the program writes it (run further) ..."`, because the boot ROM was still running. The remedy was right and immediate. My bot then crashed on the missing `value` key, which was my bug. Fixed with `--to MainLoop`. The recipe "Does the save survive turning the Game Boy off?" does say `run --frames 300` after the power cycle; I skimmed past it.
3. Nothing else cost more than a minute. The first full build of the five new files (belts, play, supply, results, save; about 900 lines) assembled clean, and the game ran on the first try.

**What saved real time.**
- `run --at play --poke 'wPartsSlots+8=4' --poke 'wHands=1,0' --keys 'B@1 -B@2 A@5 -A@6' --read ...`. One call per rule, each from the same checkpoint, made a 24-rule check that reruns in under a second. Pokes take symbol+offset, multi-byte values and repeats. Reads take comparisons too (`[wPartsUnder] != 8`).
- Deterministic runs plus `--keys-file --keys-since boot`. The bot's 115 short runs replay as one run, frame for frame (same score). That turned the frame budget criterion into one command.
- `gbcli sessions rm` plus a fresh `--from boot` start gives every check a clean save, with no file juggling.
- The `uninitialized_read` error names the variable, its address and the fix.

**The open calls I made** (all dialable in `include/tuning.inc` or noted for Ben):
- **Pick-up zone:** the 16 px slot under Elle's middle, if its middle is within PICKUP_REACH = 6 px.
- **Supply skew:** slot chance 192/256 with a 1-slot gap after each piece. While she holds one half, a new piece is its partner 96/256 of the time, and always after 3 misses.
- **Pieces:** streamed background tiles, not sprites.
- **Chord window:** 5 frames.
- **No room:** B onto an occupied slot under Elle (a half back on the parts belt, or a toy onto the Finished belt) is refused with a buzz. The controls table doesn't cover a full belt.

## 2026-11-16 — Factory 2b: Ben's playtest changes (agent factory2)

**What changed.**
- **The slinky is one piece.** In `data/toys.txt` a toy whose half B is `same` is one piece used twice. `tools/gen_toys.py` now also emits `PiecePartner` (per piece, the piece that completes it) and `SupplyPieces` (the 11 pieces the belt deals: the slinky's second number never comes). `SetWanted` reads `PiecePartner`, so holding a coil wants a coil, and the supply's forced match follows `wWantedPiece` as before. `SupplyNext` picks evenly from `SupplyPieces`.
- **Elle's turn.** `TURN_FRAMES` (tuning.inc, 8). Up or Down starts a turn: `wTurnTimer` counts down, and A, B and the pad do nothing, with no buzz. A press waiting for its chord partner is dropped with the turn. The new facing holds from the frame the timer reaches 0, and that frame's presses act. To buffer presses instead, the comment at the turn in `src/play.asm` says what to keep. She stands at y 58 facing the parts belt and 50 facing the Finished belt (`include/layout.inc`), and her side frame (a placeholder third frame in `assets/sprites/elle.16x48.pixels`) shows at y 54 through the turn. Held pieces follow her y. `TURN_FRAMES EQU 0` makes the turn instant.
- **Results text at twice the size.** `src/bigtext.asm` sets a line with TextDraw in a scratch buffer, then draws each tile doubled into 2x2 tiles (a 16-byte nibble-doubling table). The score, the best and "New best!" are 16 px tall now, in tiles 16-127, which the results screen may reuse because ScreenInit reloads them. "Toys made: 999" is 9 font tiles, so 18 columns doubled: it fits.
- **New tools.** `tools/review_shots.py` takes the screenshots for a human review into `build/review/`. `tools/png.py` gained a writer and side-by-side stacking.

**Evidence.**
- `python3 tools/make.py --gbcli <gbcli>`: both builds errors=0 warnings=0, `31/31 rules pass; 38 gbcli calls, 0.8 s`. There are 7 new scenarios: the step up and step down (`[wElleY]`, `[wElleFrame]`), the side view mid-turn, A and B during the turn (no drop, no grab, no buzz, no chord), B pressed TURN_FRAMES after Up drops and scores, a coil wants a coil, two coils grabbed off the belt and assembled into the slinky, and a coil refusing a train engine.
- Red first: before the game code, 6 of the new scenarios failed (for example `[wWantedPiece] = 8 (want 7)`, and `[wBuzzes] = 1 (want 0)` for a press during the turn). Two were green on their first run, and I kept them as the "still works" half of a pair. "B pressed TURN_FRAMES after Up acts" was green while turns were instant. "A coil refuses an engine" was green because the old rule refused it too. Two first drafts of the slinky scenarios passed only because `holding()` pokes `wWantedPiece` itself. I rewrote them so the game sets it, grabbing both coils off the belt.
- The turn, as a shot sequence: `python3 tools/review_shots.py` writes `build/review/turn.png`, crops around Elle from `gbcli run --session review --at mid --frames K --keys 'UP@1 -UP@2' --shot ... --scale 1 --crop 36,36,64,76` for K = 3, 6, 10, 11, then the same with DOWN from a checkpoint facing the Finished belt. It shows front (low), side, side, side, back (8 px higher), then side, side, side, front.
- The results screen: `build/prove/bot-results.png` from `python3 tools/bot.py`. The bot now waits out the turn before it drops: it scored 9 (8 before, the slinky is easier) in 95 calls, 3.6 s, and the best was kept across `power-cycle`.

**Stumbles, by time cost.**

1. **The first `make.py` failed on a session another checkout owns (about 4 min).** `gbcli run build/factory.gb --session rules --from boot --frames 30` answered `bad_request: session "rules" runs examples/factory/build/factory.gb, not /Users/ben/git/gbcli/.claude/worktrees/factory/examples/factory/build/factory.gb`. gbcli looks for `.gbcli/` by walking up from `--dir`. examples/factory had none (it is gitignored, and this worktree was fresh), so the walk reached the main checkout's `.gbcli/`, where someone had already run the rules check. Every session I made would have gone into another checkout. `gbcli help` says sessions live in `.gbcli/sessions/NAME/` but not how the folder is found, and nothing in a run's output names the folder it used. I expected a project with `assets/` and `src/` (made by `gbcli new`) to be its own root. Fix: `mkdir examples/factory/.gbcli`. A `project=` field on every session row, or `gbcli new` writing a marker that stops the walk, would have shown the problem at once.
2. **Where a key press lands, and when the screen shows it (about 5 min).** The first turn strip was off by a frame, then by another: `--keys 'UP@1'` then `--shot` after K frames. I measured it with `--read '[wTurnTimer]' --read '[$FE10]'` for K = 1-11. The press reaches the game on the run's second frame, and a shot shows the objects of the frame before the stop. Both make sense (the run stops at the VBlank vector), but neither `run --help` nor the keys section says which frame `@1` is read in. The rules check never noticed, because its scenarios compare presses with each other. One line, "a press at @N is seen by a program that reads the pad in VBlank on frame N+1", would have saved the probe.
3. **`--crop` doesn't apply to `--filmstrip` (1 min).** `gbcli run ... --filmstrip build/prove/turn-up.png --crop 32,40,72,80` answered `bad_request: --crop only applies with --shot`. The filmstrip was 3862 px wide for 12 frames. I took a cropped `--shot` per frame instead and stacked them with `tools/png.py`. A filmstrip with `--crop` would have been one call instead of nine.

**What saved real time.**
- The rules check's one-call scenarios again: 7 new rules in about 40 lines, each red then green in under a second.
- `--read` with comparisons (`[wElleY] < 58`) let a scenario say "between the two" without hard-coding the midpoint.
- `gbcli font FILE --sample TEXT` gave each results line's width in tiles before I wrote the doubling code, so I knew "Toys made: 999" fits at 2x.

## 2026-11-16 — Factory 3: art, music and sound (agent factory2)

**What was built.**
- **One tile set for both machines.** Pieces are background tiles on the belts and objects in Elle's hands, from the same data, so the color numbers are chosen to make that work. Color 0 is clear on objects and the belts' dark gray on the background, through BGP %11_01_00_10 and OBP0 %11_01_00_00. A piece's clear pixels then show the belt, with no second copy of the tiles. `tools/tiles.py` holds the mapping, `include/layout.inc` the matching BGP_GAME and OBP_GAME, and `src/main.asm` asserts that gbcli's `ELLE_OBP` equals OBP_GAME.
- **The scene.** `art/scene.pixels` holds 17 tiles: the white blank and floor, the light-gray wall and its rail, the belt's edges, the dark-gray belt with a black line every 8 px, the key panel and edge, and the 3x3 score box. `tools/gen_scene.py` turns it into `src/gen/scene.asm` and `include/gen/scene.inc` (the tile numbers). It replaces the `PlaceholderTiles` in screen.asm.
- **The toys.** I redrew the six `art/toys/*.pixels` files to fill their 16x16 cells, with black outlines and white or light-gray fills, and a distinct silhouette per half. `tools/gen_toys.py` refuses dark gray in a toy, because an object can't show it. The clock (`tools/gen_clock.py`) has a white face on the wall gray, dark-gray ticks and black ink. I removed `tools/placeholder_toys.py`, which would have rewritten the toys as placeholders.
- **Elle.** `assets/sprites/elle.16x48.pixels` has three frames: from behind, from the front and from the side. Each 8x16 object keeps to three colors, so on a GBC `src/colors.asm` colors her per object: a blonde head with a plum choker and eyes, a denim top, the blonde hair down her back, and a navy skirt. The DMG version is gray art, one tile set.
- **GBC colors** (`src/colors.asm`): the pink wall band, the white clock face (red in the last 10 s), the gray belts, and Elle's four palettes plus the pieces' palette. The title and results screens use the scene's palette.
- **Title and results.** The title shows the name at twice the size, the six toys riding a belt, the best and "Press START". The results screen shows "Time's up!", the score, the best and "New best!" at twice the size, the toy belt and "Press START".
- **Sound.** `assets/music/theme.song` is a 128-row C-major loop: an oom-pah wave bass, a staccato square lead, kick, snare and hats. It never plays channel 1: its orders give channel 1 an empty pattern, and `MusicInit` is followed by `hUGE_mute_channel` on channel 1. `src/sound.asm` is a small effect sequencer on channel 1 for grab, assemble, score, buzz, turn, the clock going red, a tick each second in the red, and time's up. The music's starting volumes are 4 and below (wave 25%); the effects' are 15 (12 for the turn). The music ticks from the VBlank handler on every screen, in its ROMX bank.

**Evidence.**
- `python3 tools/make.py --gbcli <gbcli> --prove` runs everything in 7.3 s wall: both builds `errors=0 warnings=0`, `31/31 rules pass`, prove_screen `6/6 claims pass`, and prove_sound with all 9 claims passing.
- The sound, `python3 tools/prove_sound.py` (12 gbcli calls):
  ```
  PASS music: 600 frames, 796 writes, channel 1 writes 0; starting volumes {'wave': '25%', 'noise': 3, 'square2': 4}
  PASS effect grab: channel 1 starts at volume 15 (music at 4 and below)
  ... assemble, score, buzz, warn and ticks, time's up: 15; turn: 12
  PASS louder: the score effect's loudest 1/30 s is RMS 3348, the whole music's 1089, x3.1 (per channel {'square1': 3348, 'square2': 803, 'wave': 621, 'noise': 394}; build/prove/sound-score.wav and its -<channel> files)
  ```
  The underlying commands are `gbcli run --session sound --at play --frames N --poke ... --keys ... --audio-log --json`, which gives each write's address and value (NRx2's top nibble is the starting volume), and `... --wav build/prove/sound-score.wav --channels each` for the per-channel loudness.
- No lag: replaying the bot's round, `gbcli run build/factory.gb --session replay --from boot --frames 3664 --keys-file build/bot.keys --keys-since boot --frame-budget` gives `lag=0 lcd_off=9 active_max=10980`. The busiest frame is 15.6% of a frame, up from 13% with the music's tick. To frame 3700 the replay shows the known round-end frame, `frame_lag from=3665 count=1`. The bot scored 9 in 85 calls, 4.1 s.
- Every image is a `.pixels` file. In `assets/` there is only `elle.16x48.pixels`, built by gbcli. In `art/` are the scene and the toys, which the tools turn into `src/gen/`. `gen_toys.py --check`, `gen_scene.py --check` and `gen_clock.py --check` all pass, so the generated files are byte-identical to a fresh run.
- Screenshots for review: `python3 tools/review_shots.py` (22 gbcli calls) writes `build/review/`: title, round-dmg, round-gbc (`--model gbc`), red-gbc, turn, key, results and toys.

**Stumbles, by time cost.**

1. **Loudness from a WAV is mostly pops (about 12 min).** `gbcli run ... --wav build/prove/sound-score.wav --channels each`, then the peak of each channel's file: square1 4921, square2 4412, wave 4316, noise 4273, so the volume-15 effect looked no louder than the volume-7 music. There were two causes, and gbcli's decoded log showed the first. (a) My lead's `E03` note cuts: the log reads `NR22 $00: square 2 DAC off`, and every DAC switch pops at full scale whatever the volume. I removed the cuts and let short envelopes make the staccato. (b) Each per-channel file starts with a step, because that channel's DAC was already on when the replay began. That made noise read 2011 RMS at volume 3. I now skip the first 0.2 s and fire the effect 20 frames in. After both: x3.1. Neither trap is in `run --help`. One line by `--channels each`, "each file starts with a step if the channel was sounding when the run began", would have saved most of this. So would a per-channel loudness number in `--audio-summary`.
2. **The clock tore when I moved its tile buffers (about 5 min, my bug).** The new scene needed 17 tiles, so I moved `T_CLOCK_A` from 16 to 20. clock.asm sets only the high byte of a buffer's address, so the copy landed on the score box. The `--shot` showed it at once: half a face, and a stray corner on the box. I put the buffers back on 256-byte boundaries, with an ASSERT.
3. **The bot assumed the round starts within 4 frames of START (about 3 min, my bug).** With bigger title and round screens drawn while the LCD is off, the round now starts about 6 frames after START, and the bot's loop ended at once with `score 0, 0 moves`. It now waits with `--until '[wGameState] == 0'`.
4. Nothing else cost more than a minute. One example: rgbasm scopes `.end` to the last global label, and `ClockNormalColors::` sat between `BgColors` and its `.end`. The rgblink error named the file and line.

**What saved real time.**
- `--audio-log`'s decoded text ("NR12 $F2: square 1 envelope 15 down, pace 2") and the `--json` address and value: the volume proof is a few lines of Python, and the DAC-off row explained the pops.
- `gbcli song FILE --piano PNG` checks a song with no build. The 128-row tune parsed on the first try.
- The `images` row and `images.inc` give the sheet's OBP (`ELLE_OBP EQU %11_01_00_00`), so sharing OBP0 between gbcli's sprite and my own piece tiles is an assert, not a guess.
- `--shot --scale 1 --crop` plus the stdlib PNG reader: the turn strip and the key column are crops stacked by a 100-line tool.

## 2026-11-16 — Factory 3b: Ben's art review fixes (agent factory3)

**What changed.** These are the seven fixes from the plan's ▸ Art review rulings.
1. **Slinky.** The piece is a short coil stack, 9 rows tall. The finished slinky is the old piece: one tall stack (`art/toys/slinky.pixels`).
2. **Gag glasses,** after Ben's photo. Half A has bushy black eyebrows over round black frames with white lenses, then a big light nose. Half B is a big black mustache, 16 px wide, with a few light strands so it reads as hair. The finished toy puts the mustache directly under the nose.
3. **Crayon box.** It is now 16 px wide and packed with five crayons whose tips stand at uneven heights. The old version, with three crayons over a narrow box, read as a cake with candles.
4. **Key column.** I moved the window, not the tiles: `KEY_X` 136 → 130, and the window map is 4 tiles wide. The 2 px divider is followed by 6 px of panel, the 16 px icon, then 6 px. The belts lose 6 px of visible width. `tools/bot.py`, `prove_screen.py` and `review_shots.py` follow the new x.
5. **Facing away, Elle is drawn over what she holds.** `ElleDraw` now hides all 10 of its objects, then puts hers first in OAM when she faces away and the held things first otherwise. A GBC draws the lower OAM index on top. A DMG draws the smaller X on top and breaks ties by index. So facing away, each held column that overlaps her has exactly her column's X: the toy sits at `ELLE_X` and is hidden, and the halves sit at `ELLE_X ± 8` (`HAND_BACK_*_X`), each with its inner column behind her and its outer column peeking out. Assembly still works facing either way; that rule is unchanged and still checked. Follow-up, at the integrator's request: facing the parts belt, the right-hand half (x 73) lost its leftmost 3 px to Elle's right column on the DMG, because her column had the smaller X. The front-view rule used to check only the GBC's OAM order. It now checks both machines and the held toy too, and failed first: `DMG: OAM 5 (elle, x 76) draws over OAM 2 (held, x 81)`. The right half now sits at `ELLE_X + 16`, just clear of her. The left half stays at `ELLE_X - 13`: it overlaps her by 3 px, and on a DMG its smaller X puts it on top.
6. **Belt lines behind pieces.** `tools/gen_toys.py` now also emits `ToyBeltTiles`, the same 72 tiles with the scene's belt tile (`T_BELT`, from `art/scene.pixels`) composited behind each clear pixel. In a round the belts draw from `T_BELT_TOYS` (170, after Elle's 24 tiles; an ASSERT checks that it fits). Elle's hands and the key keep the plain tiles at `T_TOYS`. The title and results screens show toys only on a belt, so they load the belt copies at `T_TOYS`.
7. **The jingle.** `SfxScore` is now a run up the major pentatonic, C6 D6 E6 G6 A6 at 3 frames each, then C7 rings for 20 frames, duty 50%, volume 15.

**Evidence.**
- `python3 tools/make.py --gbcli <gbcli> --prove`: both builds `errors=0 warnings=0`, `34/34 rules pass; 41 gbcli calls, 1.0 s`, prove_screen 6/6, prove_sound 10/10.
- **Draw order, red then green.** There are three new rules in `tools/rules_check.py`. Each reads `oam[N].y/.x/.tile` for N = 0-9 in one `gbcli run --at play --frames 4 --poke ... --read 'oam[0].x' ...` call. It then checks every overlapping Elle/held pair against both machines' priority rules, and that the halves (and not the toy) have a column beside her. Before the code: `FAIL ... GBC: OAM 0 (held) draws over OAM 6 (elle); DMG: OAM 0 (held, x 68) draws over OAM 6 (elle, x 68)`, and for the halves `DMG: OAM 1 (held, x 63) draws over OAM 4 (elle, x 68)`. The third rule ("facing the parts belt, what she holds is in OAM before her") was green on its first run, on purpose: it is the "still works" half.
- **The jingle, red then green.** The new `jingle` claim in `tools/prove_sound.py` reads channel 1's notes from `--audio-log --json` (NR13, then NR14 with the trigger bit). It wants 5 or more, each higher than the last. Before: `FAIL jingle: the score effect plays 4 notes ... (Hz [785, 1049, 1324, 1560]...)`. After: `PASS jingle: ... 6 notes on channel 1, each higher than the last (Hz [1049, 1170, 1324, 1560, 1771, 2081] ...)`. gbcli's own view of it: `gbcli run --session sound --at play --frames 40 --poke ... --keys 'B@2 -B@3' --audio-log --audio-channel 1` prints `NR14 $87: square 1 trigger, ... period 1923 = C6 +3.4c` and the rest of the run. `--piano build/prove/jingle-piano.png` draws it as a red staircase on square 1. Loudness: `PASS louder: ... RMS 3974, the whole music's 1103, x3.6` (it was x3.1).
- **No lag.** `python3 tools/bot.py` scored 9 (92 calls, 4.1 s). The replay `gbcli run build/factory.gb --session replay --from boot --frames 3665 --keys-file build/bot.keys --keys-since boot --frame-budget` gives `lag=0 lcd_off=10 active_max=12120`. One frame later is the known round-end screen change.
- **Shots.** `python3 tools/review_shots.py` (30 gbcli calls) adds `away-dmg.png` and `away-gbc.png`: facing away with a toy, then with halves, then the same two facing the parts belt. `toys.png` now shows each piece on the belt with its lines.

**Stumbles, by time cost.**
1. **`play.asm` didn't see `ELLE_FRAME_WIDTH` (1 min, my bug).** I sized the OAM block from images.inc's frame size, but play.asm didn't include it. The build error named the file, line and symbol.
2. **A stray `python3 -` in a shell call waited on stdin for 2 minutes (my bug, not gbcli's).**
3. Nothing in gbcli cost time this leaf.

**What saved real time.**
- `oam[N].x` / `.y` / `.tile` in `--read`. A draw-order rule on both machines became 30 reads in one call, with no OAM addresses to work out. The check is plain Python over those numbers.
- `--audio-log`'s decoded notes ("period 1923 = C6 +3.4c") and `--piano`. The jingle's claim is about ten lines, and the piano roll shows it rising at a glance.
- Generated tiles. The belt copies are about 15 lines in `gen_toys.py`, because the scene's belt tile is data the tool can read.

## 2026-11-16 — Factory 4: the loop, the README, verification (agent factory3)

**What changed.**
- **The loop.** START on the results screen now goes back to the title, and START on the title starts a round. Before, results went straight into the next round. Two new rules cover it. "START on the results screen goes back to the title" was red first (`[wGameState] = 0 (want 1)`). "... and START on the title starts a new round" was green on its first run, because the old code also ended in a round; it stays as the "still works" half.
- **`examples/factory/README.md`** is now the game's README. It covers what the game is, how to play (the controls table), how to build, what's technically notable, a Tuning section with all 17 constants in `include/tuning.inc` (15 knobs and the two direction names) and the one command, then each claim with the gbcli command that proves it and that command's output.

**Evidence, rerun on the final build.**
- `python3 tools/make.py --gbcli <gbcli> --prove`: `gbcli build` and `gbcli build --gbc` give `errors=0 warnings=0`; `36/36 rules pass; 44 gbcli calls, 1.2 s` (after the front-view follow-up); prove_screen 6/6; prove_sound 10/10. `gen_toys.py --check`, `gen_scene.py --check` and `gen_clock.py --check` pass.
- `python3 tools/bot.py`: score 9, 88 moves, 92 calls, 4.1 s. The best was kept across `power-cycle`, and a second round kept it.
- Every README command was run by hand on a fresh session, with the outputs as shown. Those commands are the scrolls 180 frames apart (87→155, 168→100, the clock step 6), `--raster` (`raster_writer reg=rSCX pc=$1B81 writes=6 hblank=6`), the two key-column crops (identical pixels), the replay's `frame_budget frames=3665 lag=0`, the battery sequence (7, `wNewBest` 1, power-cycle, 7), and the jingle's `--audio-log --audio-channel 1 --piano`.

**Stumbles, by time cost.**
1. **`--audio-log` text stops at 20 rows (1 min).** The jingle's 6 notes are 36 writes on channel 1, so the text view showed C6 to G6 and then `more`. `run --help` says so ("The rows stop at 20 with a more row; --json has every write"). The README shows `--piano` for the whole run and points to `--json`.
2. **`gbcli sessions rm` refuses a session that has checkpoints (under 1 min).** The refusal named the fix, `--with-checkpoints`.
3. Nothing else cost time.

**What saved real time.** Every claim was already a tool with PASS/FAIL rows, so the README is those tools' gbcli calls written out, and rerunning it was mechanical.

## Summary (agent factory3, closing the slice)

Teddy & Toys took five leaves: the screen's skeleton, the shift, Ben's playtest changes, art and sound, then the art fixes and this polish. All of it was built from gbcli's help, recipes and README alone. Measured by time lost, here is what cost most and what helped most, drawn from every section above.

**The five costliest stumbles.**
1. **Loudness from a WAV is mostly pops (about 12 min, Factory 3).** Every DAC switch pops at full scale whatever the volume, and each `--channels each` file starts with a step. So the peaks of a volume-15 effect and the volume-4 music read alike. A windowed RMS that skips the opening fixed it. A note by `--channels each`, or a per-channel loudness figure in `--audio-summary`, would have saved it.
2. **Identical screens are different PNG files (about 6 min, Factory 1).** gbcli writes the frame into `tEXt` chunks, so a byte compare fails on identical pixels, and I had to write a PNG decoder. A `--no-meta` flag, or a pixel hash in the shot row, would remove that.
3. **The round's last frame counts as lag (about 6 min, Factory 2).** The frame that turns the LCD off and draws the results screen counts as `lag=1`, not `lcd_off`, unlike the round's start. Every proof now measures the round's own frames and names that one frame as a known screen change.
4. **Images land in ROMX, and `help images` doesn't say so (about 5 min, Factory 1).** Elle drew as garbage after a bank switch. `gbcli view oam` and `view memory` found it fast.
5. **Which frame a key press reaches the game (about 5 min, Factory 2b).** A press at `@1` is read on the run's second frame, and a shot shows the frame before the stop. It took a `--read` probe per frame to line up the turn strip. A close sixth (about 4 min, Factory 2b): with no `.gbcli/` in the game folder, a worktree's sessions went to the main checkout's, silently.

**The three things that helped most.**
1. **One `gbcli run` call per scenario:** `--at CHECKPOINT --poke ... --keys ... --read EXPR`, with comparisons and `oam[N].x` in the reads. It turned the controls table into a 36-rule check that reruns in about a second, so every tuning or art change was checked before anyone looked at it.
2. **Proofs that are one call each.** `--raster --json` gives the SCX per line and where each write landed. `--frame-budget` counts lag over a whole round. `--audio-log` decodes notes and volumes. The belt, key-column, no-lag and sound claims are each a few lines of Python over one reply.
3. **Determinism and replay.** The bot's 92 short runs replay as one `--keys-file --keys-since boot` run, frame for frame. That made "no lag in a played round" one command, and `power-cycle` plus `sav` made the saved best a three-call check.
