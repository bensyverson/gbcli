# Teddy & Toys

A toy-factory game for the Game Boy. Grab matching halves off a moving belt, snap them into toys, and send them off before the clock runs out.

Elle works one shift: one minute on the clock, between two belts that run opposite ways. Parts ride past on the front belt. She picks up two halves that belong together, snaps them into a toy, turns around and sets the toy on the Finished belt behind her. There is no winning or losing. When time runs out you see how many toys you made and your best score, which the cartridge's battery keeps. The game was designed by Ben, from his own sketch. AI agents built it with gbcli, working as users, from `gbcli help` and the repo's README alone. It runs in four shades on the original Game Boy and in color on a Game Boy Color.

## Play

Open `build/teddy-and-toys.gb` in any Game Boy emulator, or `build/gbc/teddy-and-toys.gbc` for color (it runs on a DMG too). Press START on the title.

| Facing | Holding | Press | What happens |
|---|---|---|---|
| parts belt (Down) | a free hand | A, with a piece in front of you | Picks it up, if it matches the half you hold |
| parts belt | one half | B | Puts it back on the belt, where it keeps moving |
| parts belt | two halves | B | Puts back the half you grabbed last |
| either belt | two halves | B+A together | Snaps them into the finished toy |
| Finished belt (Up) | a finished toy | B | Sets it on the Finished belt: one toy made |
| anything else | | A or B | Nothing, with a buzz |
| either belt | anything | Left or Right | Elle steps along the belt, up to 24 px each way from her spot |
| anytime in a round | | START | Pauses: a menu with Continue and Quit |

- **Turning.** Down turns Elle to face the parts belt (you see her face). Up turns her to the Finished belt (you see her back). A turn takes a moment, and buttons do nothing during it.
- **Stepping.** Hold Left or Right and Elle shuffles along the belt, with a scuffing sound, until she is 24 px from her spot. She can step facing either belt, but not mid-turn. She picks up what is in front of her where she stands, and a toy she sets on the Finished belt lands there too.
- **Pausing.** START stops everything (the clock, the belts, Elle and the music) with a short blip, and opens a menu over the score box. Up and Down choose; A or START picks; B continues. The cursor starts on Continue, so START, START pauses and carries on. Quit goes back to the title: no results screen, and the round's score isn't kept as a best.
- **Halves.** Either half can come first. The slinky's two halves are the same piece, so any two coils make a slinky.
- **B+A** is a chord. Press them together, or one just after the other.
- **Missed pieces** cost nothing. When you hold one half, its partner is never far away.
- **The toys** are listed top to bottom in the key on the right: teddy bear, train, squishy, slinky, gag glasses and crayons.
- **The clock** ticks each second for the last 10 seconds, and on a Game Boy Color its face turns red. The music speeds up then too. It is back to its normal speed on the title and in the next round.
- **When time runs out,** the results screen shows your score and your best, with "New best!" when you beat it. START goes back to the title, which shows your best too, and START there begins the next shift.
- **On a Game Boy Color** each toy has its own colors, on the belts, in Elle's hands and in the key, and the title is pink and cream.

## Build

From this folder, with gbcli and RGBDS installed (see the repo's [README](../../README.md)):

```sh
mkdir -p .gbcli                   # keeps gbcli's sessions in this folder (it is gitignored)
python3 tools/make.py             # regenerate, build both ROMs, run the rules check
```

`tools/make.py` regenerates the toy, scene and clock includes, then runs `gbcli build` and `gbcli build --gbc` (both report `errors=0 warnings=0`) and the rules check. Every tool takes `--gbcli PATH` if gbcli isn't on `PATH`. The `.gbcli/` line matters inside the gbcli repo. gbcli looks for the nearest `.gbcli/` above the working directory, and without one here your sessions land in the repo's. The tools create it themselves.

## What's technically notable

- **Two belts scroll opposite ways on one background.** The background is one layer with one horizontal scroll register (SCX). A STAT interrupt on LY=LYC fires on the line before each band. Its handler arms the next split, then waits for that line's HBlank to write the band's SCX, so every line of a band draws with the band's scroll (`src/belts.asm`). The HUD and floor keep SCX 0. Each belt's position is 8.8 fixed point, so speeds are sub-pixel.
- **The key column never scrolls.** It is the window layer, placed at the right edge from line 0 down, so the belts' lines scroll under it. The window can only cover the region right of WX and below WY, which is why the HUD sits to the left of the column (`include/layout.inc`).
- **Pieces are background tiles, not sprites.** A belt is 16 slots of 16 px, and the hardware scroll moves the pieces. The only objects are Elle and what she holds, at most 6 on a line. A slot is rewritten only while it is hidden, under the key column or off the edge of the screen, or in VBlank when Elle takes a piece or sets one down. On a Game Boy Color the same write sets the slot's colors in VRAM bank 1 (its toy's palette, or the belt's when empty).
- **Six toys, five palettes.** A GBC has eight background palettes and eight for objects. The scene, the clock and the wall take three. Each toy palette keeps the belt's gray and the ink, so a piece still sits on its belt and shows the belt's lines behind it. That leaves five for the toys, so the teddy bear and the gag glasses share the warm one (cream and tan). The same numbers serve the belts, the key column and Elle's hands; her denim top and skirt now share one palette to make room (`src/colors.asm`).
- **One tile set, two uses.** Color 0 is the belts' dark gray on the background and clear on objects, so the same piece tiles work on a belt and in Elle's hands. A second copy, with the belt's lines composited behind each clear pixel, is generated by `tools/gen_toys.py` from the scene art, so a piece on a belt shows the lines behind it.
- **Elle steps; everything she holds moves with her.** Her position is a variable (`wElleX`), and each held object sits at a fixed offset from it, so the drawing order below holds wherever she stands. The slots "in front of her" on both belts are worked out from the same position every frame.
- **Drawing order on both machines.** Facing away, Elle is drawn over what she holds. A Game Boy Color draws the lower OAM index on top. The original Game Boy draws the smaller X on top and breaks ties by index. So her objects go first in OAM, and each held column that overlaps her shares her column's X: the toy hides behind her and the halves peek out at her sides. Facing the camera it is the other way round: what she holds comes first in OAM, and a held column overlaps her only from the left, so its X is the smaller one (`src/play.asm`, `ElleDraw`; `include/layout.inc`).
- **The clock hand is 60 pre-drawn faces.** `tools/gen_clock.py` draws one face per hand position. When the hand moves, the VBlank handler copies the next face into a hidden tile buffer, 4 tiles a frame, then flips the map to it, so the hand never tears.
- **Pause holds the song's place.** While paused, the VBlank handler skips the music driver's tick, so the song stops on its row. NR51 lets only channel 1 reach the speakers, so the notes left sounding go quiet and the pause blip plays. Continue restores NR51 and the song carries on from the same row. The menu's text is set in tiles when a round starts, so opening and closing it only rewrites map entries in VBlank (`src/pause.asm`).
- **The music speeds up by changing the driver's tempo.** hUGEDriver plays a row every `ticks_per_row` ticks, one tick a frame. When the clock turns red the game sets it to `MUSIC_RED_TICKS_PER_ROW` (5, against the song's 6: 20% faster). The driver checks for an exact match, so the switch waits for a frame where its tick count is below the new value (`src/music.asm`; `src/hUGEDriver.asm` exports the two variables, its only change).
- **Sound effects sit on top of the music.** The tune (`assets/music/theme.song`, gbcli song text, played by hUGEDriver) never uses channel 1. The effects play there at volume 15, against the music's 4 and below. The jingle when a toy lands on the Finished belt runs up C6 D6 E6 G6 A6 to a ringing C7.
- **Big text from the small font.** The results screen draws its lines at twice the font's size by doubling each glyph's tile into four (`src/bigtext.asm`).
- **The best score survives power-off,** in MBC5 battery RAM behind a "TOYS" mark (`src/save.asm`).

## Tuning

Every gameplay number is in **`include/tuning.inc`**, each with its unit and a comment. After any change, run the one command that rebuilds and rechecks everything:

```sh
python3 tools/make.py
```

It regenerates what depends on the tuning file (the clock's faces), builds both ROMs and reruns the rules check, which reads its numbers from the same file. It takes a few seconds.

| Constant | Unit | What it does | Now |
|---|---|---|---|
| `ROUND_SECONDS` | seconds | The length of a round | 60 |
| `CLOCK_STEPS` | hand positions | How many pre-drawn faces the hand steps through in a round. Must divide `ROUND_SECONDS * 60`, at most 64 | 60 |
| `CLOCK_RED_SECONDS` | seconds | Seconds left when the clock face turns red (GBC), starts ticking and the music speeds up | 10 |
| `MUSIC_RED_TICKS_PER_ROW` | frames a row, 1-255 | The music's speed once the clock is red. The song's own is 6 (`ticks_per_row` in `assets/music/theme.song`); fewer is faster | 5 |
| `BELT_PARTS_SPEED` | 1/256 px a frame | The front (parts) belt's speed: 256 is one pixel a frame | 144 |
| `BELT_PARTS_DIR` | `BELT_LEFT` or `BELT_RIGHT` | Which way the parts belt runs | `BELT_LEFT` |
| `BELT_FINISHED_SPEED` | 1/256 px a frame | The back (Finished) belt's speed | 144 |
| `BELT_FINISHED_DIR` | `BELT_LEFT` or `BELT_RIGHT` | Which way the Finished belt runs | `BELT_RIGHT` |
| `SUPPLY_CHANCE` | out of 256 | The chance that a slot arriving on the parts belt brings a piece | 192 |
| `SUPPLY_MIN_GAP` | slots | Empty slots forced after each piece: the piece spacing | 1 |
| `SUPPLY_MATCH_CHANCE` | out of 256 | While Elle holds one half, the chance that a new piece is its partner | 96 |
| `SUPPLY_MATCH_MAX_GAP` | pieces | After this many new pieces that don't match, the next one does | 3 |
| `PICKUP_REACH` | px, 0-8 | How far a piece's middle may be from Elle's middle for A to grab it | 6 |
| `CHORD_FRAMES` | frames, 1+ | The B+A window: the second button up to this many frames minus one after the first still assembles (1 means the same frame only) | 5 |
| `BUZZ_COOLDOWN` | frames | After a buzz, further refusals stay silent this long | 12 |
| `TURN_FRAMES` | frames, 0+ | How long a turn takes, with A and B ignored (0 makes it instant) | 8 |
| `ELLE_STEP_RANGE` | px, 0-38 | How far Left and Right step Elle each way from her home spot. Past 38 her hands would leave the screen or reach the key column, and the build stops | 24 |
| `ELLE_STEP_SPEED` | px a frame, 1+ | How fast she steps while Left or Right is held | 1 |
| `SHUFFLE_FRAMES` | frames, 1+ | While she steps, the shuffle sound repeats this often. It never cuts off another effect | 8 |
| `BELT_LEFT`, `BELT_RIGHT` | names | The two values for the `_DIR` constants above, not numbers to tune | 0, 1 |

Other things to edit:

- **The toys** are data. `data/toys.txt` lists them in key order, with their names, halves and art file. The art is in `art/toys/*.pixels` (gbcli pixel text, 48x16 each: half A, half B, the finished toy). A toy whose second half is `same` is one piece used twice, like the slinky. `tools/make.py` regenerates the include.
- **The toys' GBC colors** are in `src/colors.asm`: `ToyColors` holds the five palettes and `ToyPalettes` says which toy uses which, in `data/toys.txt`'s order. A new toy needs a line there too; the build stops if the count doesn't match.
- **The scene** is `art/scene.pixels`. Elle is `assets/sprites/elle.16x48.pixels`, and the music is `assets/music/theme.song`. The screen's layout (band rows, Elle's position, the key column's x) is in `include/layout.inc`; changing it means changing the art too.

## Checking the claims

Each claim below has the command that proves it, rerun on the final build (2026-11-17). The commands run from this folder; `gbcli` stands for your gbcli binary. The tools wrap the same gbcli calls and print PASS or FAIL per claim:

```sh
python3 tools/make.py --prove     # builds, rules check, then prove_screen.py and prove_sound.py
python3 tools/bot.py              # a bot plays a whole round, then checks the saved best
python3 tools/review_shots.py     # screenshots for a human, in build/review/
```

**Both builds are clean.**

```sh
gbcli build && gbcli build --gbc
# build rgbds=1.0.4 target=dmg sources=20 errors=0 warnings=0
# build rgbds=1.0.4 target=gbc sources=20 errors=0 warnings=0
```

**Every rule in the controls table holds.** `python3 tools/rules_check.py` runs one scripted scenario per rule. Each is one `gbcli run --session rules --at play --frames N --poke ... --keys ... --read ...` call from the same checkpoint. The rules cover the table, the turn, the slinky, the drawing order facing each way, the round's end, the loop back to the title, the saved best and the title showing it after a power cycle. They also cover pausing (the menu, Continue three ways, Quit with no best kept, a pause mid-turn, everything holding still), the music's speed-up and its reset, and stepping (both limits, no step mid-turn, reach and drop from where she stands, the drawing order at each limit). It prints `67/67 rules pass; 82 gbcli calls, 3.5 s`.

**The belts move opposite ways, each at its tuned speed.** Start a round, then read both scrolls 180 frames apart:

```sh
gbcli run build/teddy-and-toys.gb --session readme --from boot --frames 300 \
  --keys 'START@60 -START@62' --checkpoint early --read '[wPartsX+1]' --read '[wFinishedX+1]'
# read expr=[wPartsX+1] value=131 ...   read expr=[wFinishedX+1] value=124 ...
gbcli run --session readme --at early --frames 180 --read '[wPartsX+1]' --read '[wFinishedX+1]' --read '[wClockStep]'
# read expr=[wPartsX+1] value=232 ...   (+101: 180 x 144/256)
# read expr=[wFinishedX+1] value=23 ...   (-101)
# read expr=[wClockStep] value=6 ...      (the hand moved 3 steps in 3 s)
```

**Each band draws with its own scroll, written in HBlank.**

```sh
gbcli run --session readme --at early --frames 2 --raster
# raster_write reg=rSCX value=$7C pc=$1C3F ... mode=hblank   (line 55: the Finished belt)
# raster_write reg=rSCX value=$00 pc=$1C3F ... mode=hblank   (line 87: the floor)
# raster_write reg=rSCX value=$83 pc=$1C3F ... mode=hblank   (line 111: the parts belt)
```

`tools/prove_screen.py` checks the same thing with `--raster --json` on every line: one SCX per band, every write in HBlank or VBlank.

**The key column stays still while the belts move.**

```sh
gbcli run --session readme --at early --frames 0 --shot key-early.png --crop 130,0,30,144
gbcli run --session readme --at early --frames 180 --shot key-later.png --crop 130,0,30,144
```

The two crops are identical pixel for pixel (compare the pixels, not the files: gbcli's PNGs carry the frame in a text chunk), while the same crops of the belts differ. `tools/prove_screen.py` compares them: `PASS key: key column x 130-159 identical, belts changed`.

**No lag frames.** The bot's round replays frame for frame from its key log:

```sh
python3 tools/bot.py              # writes build/bot.keys
gbcli run build/teddy-and-toys.gb --session replay --from boot --frames 3666 \
  --keys-file build/bot.keys --keys-since boot --frame-budget --read 'u16[wScore]'
# read expr=u16[wScore] value=18 hex=$12 ...   (12 toys, in BCD)
# frame_budget frames=3666 lag=0 lcd_off=11 ... active_max=14836 ...
```

The busiest frame uses about 21% of a frame. The frame where the round ends and the results screen is drawn with the LCD off (3667) counts as one lag frame. It is a screen change, not play. `prove_screen.py` makes the same check over a whole round from boot. A GBC run that pauses, moves the menu's cursor, continues and steps Elle both ways also shows `lag=0`:

```sh
gbcli run build/gbc/teddy-and-toys.gbc --model gbc --session budget --from boot --frames 400 --frame-budget \
  --keys 'START@60 -START@61 START@120 -START@121 DOWN@140 -DOWN@141 UP@150 -UP@151 START@170 -START@171 RIGHT@200 -RIGHT@240 LEFT@250 -LEFT@320'
# frame_budget frames=400 lag=0 lcd_off=12 ... active_max=14816 ...
```

**The best score survives a power cycle.**

```sh
gbcli run --session readme --at early --frames 8 --poke 'wRoundLeft=3,0' --poke 'wScore=$07,0' \
  --read 'u16[wBest]' --read '[wNewBest]'
# read expr=u16[wBest] value=7 ...   read expr=[wNewBest] value=1 ...
gbcli power-cycle --session readme
# power_cycle battery=head bytes=8192 ...
gbcli run --session readme --frames 30 --read 'u16[wBest]' --read '[wLine+6]' --shot title.png
# read expr=u16[wBest] value=7 ...   read expr=[wLine+6] value=23 ...   (the title's "Best: 7": glyph 23 is "7")
```

`title.png` shows "Best: 7" under the toy belt. The rules check asserts the same (the composed line, and its tiles on the map), and `build/review/title.png` and `title-gbc.png` show the title after a round that scored 12.

**START pauses: everything holds still, and only the effects' channel plays.**

```sh
gbcli run --session readme --at early --frames 20 --keys 'START@1 -START@2' \
  --read '[wGameState]' --read '[$FF25]' --read 'u16[wRoundLeft]' --read 'u16[wPartsX]'
# read expr=[wGameState] value=2 ...   read expr=[$FF25] value=17 hex=$11 ...   (NR51: channel 1 only)
# read expr=u16[wRoundLeft] value=3366 ...   read expr=u16[wPartsX] value=33696 ...
gbcli run --session readme --at early --frames 120 --keys 'START@1 -START@2' --read 'u16[wRoundLeft]' --read 'u16[wPartsX]'
# read expr=u16[wRoundLeft] value=3366 ...   read expr=u16[wPartsX] value=33696 ...   (100 frames later: the same)
```

The rules check also holds the music driver's row and tick still across a pause, and checks Continue (A, B, or START again) and Quit.

**The music speeds up when the clock turns red.**

```sh
gbcli run --session readme --at early --frames 2 --read '[hUGE_ticks_per_row]'
# read expr=[hUGE_ticks_per_row] value=6 ...   (the song's tempo)
gbcli run --session readme --at early --frames 30 --poke 'wRoundLeft=91,2' --read '[hUGE_ticks_per_row]'
# read expr=[hUGE_ticks_per_row] value=5 ...   (603 frames left, red at 600: MUSIC_RED_TICKS_PER_ROW)
```

**Elle steps 24 px each way, no further.**

```sh
gbcli run --session readme --at early --frames 60 --keys 'RIGHT@1 -RIGHT@58' --read '[wElleX]' --read '[wSfxLast]'
# read expr=[wElleX] value=84 ...   read expr=[wSfxLast] value=10 ...   (home 60 + 24; the shuffle sound)
gbcli run --session readme --at early --frames 60 --keys 'LEFT@1 -LEFT@58' --read '[wElleX]'
# read expr=[wElleX] value=36 ...
```

**The toys are in color on a Game Boy Color, and the DMG reads as before.** `python3 tools/review_shots.py` writes the screenshots to `build/review/`: `round-gbc.png` (the belts, her hands, the key), `held-gbc.png` (each toy in her hands), `title-gbc.png`, `pause-gbc.png` and `step-gbc.png`, beside the DMG's `round-dmg.png`, `title.png`, `pause-dmg.png` and `step-dmg.png`. The DMG build draws the same tiles in the same shades as before; the colors are GBC attributes and palettes only.

**Effects play on channel 1, louder than the music, and scoring plays a rising jingle.**

```sh
gbcli run --session readme --at early --frames 40 --poke 'wToy=1' --poke 'wFacing=1' \
  --poke 'wFinishedX=0,4' --poke 'wFinishedSlots+4=0' --keys 'B@2 -B@3' \
  --audio-log --audio-channel 1 --piano jingle.png
# ... NR12 $F1: square 1 envelope 15 down, pace 1
# ... NR14 $87: square 1 trigger, length off, period 1923 = C6 +3.4c (1048.58 Hz)
# ... NR14 $87: ... period 1936 = D6 -6.5c ..., then E6 and G6 (the text rows stop at 20;
#     --json has every write: A6 and C7 follow); jingle.png shows all six as a rising staircase
```

`tools/prove_sound.py` checks that the music never writes channel 1 and starts its channels at volume 4 and below, while every effect starts at 12 or 15. It also checks that the jingle is 6 rising notes, and that its loudest moment is 3.6 times the music's (RMS, from `--wav --channels each`).

## Files

- `src/`: the game. `main.asm` holds start-up, the main loop and the round; `belts.asm` the scroll splits and slots; `play.asm` the controls and Elle; `pause.asm` the pause menu; `supply.asm` the parts supply; `clock.asm`; `results.asm` the title and results screens; `sound.asm` the effects; `music.asm` the music and its tempo; `save.asm`; `colors.asm` the GBC palettes.
- `include/tuning.inc`: every gameplay number. `include/layout.inc`: where things are on screen and in tile memory.
- `data/toys.txt`, `art/`: the toys and the scene, which `tools/gen_toys.py` and `tools/gen_scene.py` turn into `src/gen/` and `include/gen/`.
- `tools/`: `make.py` (the one command), the generators, `rules_check.py`, `prove_screen.py`, `prove_sound.py`, `bot.py` and `review_shots.py`. Python 3 standard library only.

The agents' notes on building it with gbcli are in [project/2026-11-16-dx-factory.md](../../project/2026-11-16-dx-factory.md).
