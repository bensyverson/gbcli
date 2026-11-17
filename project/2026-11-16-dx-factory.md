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
