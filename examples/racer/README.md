# Switchback

A Game Boy road race against the clock: drive one lap of a winding, hilly road before the sun goes down.

Switchback is a pseudo-3D racer for the original Game Boy, also in color on a Game Boy Color. The road curves and climbs because the program changes the screen's scroll position on every line as the screen is drawn, from an interrupt at the end of each line. Rival cars and roadside trees are sprites, drawn at four pre-scaled sizes as they come nearer. An AI agent built it with [gbcli](../../README.md) as an ordinary user: the code, the art (drawn by Python scripts), the music and the checks below.

## Play

**Just want to play?** Download [`switchback.gb`](switchback.gb) and open it in any Game Boy emulator. It's one ROM for both machines: in color on a Game Boy Color, in four shades on the original Game Boy. Rebuild it with `scripts/examples/build-roms.sh`.

```sh
cd examples/racer
gbcli build                                  # or gbcli build --gbc, for color
gbcli run build/racer.gb --session play --frames 600 --keys 'START@30 -START@32 A@40' --shot build/race.png
```

| Button | Does |
|---|---|
| START | start the race; after the result, back to the title |
| A or Up | accelerate |
| B or Down | brake |
| Left, Right | steer |

You have 50 seconds for a lap of about 42 seconds flat out. A rival you run into slows you down; a tree stops you dead. On the grass you can't go faster than a crawl, and on a bend the car slides outward.

## What's technically notable

- **A scroll write on every road line, in HBlank.** `src/road.asm`: the STAT interrupt fires at the start of each line's HBlank. The handler runs from RAM and writes SCX and SCY for the next line from two immediates first, done 92 dots after the interrupt (112 at worst). Then it patches those immediates for the line after next. On the car's lines the sprites are kept to at most 7 objects, so HBlank lasts at least 120 dots. The sky shares one scroll, set in VBlank: the interrupt is off over it, and the timer turns it on 4 lines above the horizon.
- **Hills, curves and stripes from tables.** `tools/tables.py` precomputes, per hill level, which road strip each line shows (a climb raises the horizon, a crest drops it). It also precomputes the sideways offset per curve level, and which BG row draws each strip in each stripe phase. Once a frame, `ComputeRoad` turns them into the next frame's scroll tables (double-buffered) with two loops that patch their own constants.
- **Objects sit on next frame's road.** `src/objects.asm` maps an object's depth to a strip, bisects the frame's hill profile for its line, and reads that line's SCX for its x. So rivals and trees follow the curves and vanish behind crests.
- **Music and effects together.** Two hUGEDriver songs written as gbcli song text by `tools/songs.py`. The race song leaves channel 2 to `src/sfx.asm`: an engine whose pitch follows the speed, countdown beeps, bumps, crashes, the finish and time up.
- **Every asset is generated.** `python3 -I tools/make_assets.py` rewrites every PNG, the palettes, both songs and the tables, byte for byte.

## The claims, and the commands that check them

Run these from `examples/racer`, after `gbcli build`. The keys files are in `tools/`: `win.keys` was recorded by `tools/driver_bot.py`, which drives a lap through gbcli and keeps to the center line.

**A scripted lap wins, and another run loses.**

```sh
gbcli run build/racer.gb --session win --keys-file tools/win.keys --until '[wState] == 2' --frames 3000 --read '[wTimeSec]'
#   stop predicate … frame=2715 … text="[wState] == 2"     (wState 2: won)
#   read expr=[wTimeSec] value=7                         (7 s left, BCD)
gbcli run build/racer.gb --session lose --keys-file tools/lose.keys --until '[wState] == 3' --frames 3400 --read '[wCrashes]'
#   stop predicate … frame=3157 … text="[wState] == 3"     (time up, after 24 crashes)
```

**START after the result goes back to the title** (wState 4):

```sh
gbcli run --session win --frames 90 --keys 'START@60 -START@62' --read '[wState]'
#   read expr=[wState] value=4
```

**Every scroll write lands in HBlank.** Stop the run if either write instruction is about to run while the PPU is in any mode but 0. Neither stops a whole race from boot:

```sh
gbcli break StatScxWrite --if '([rSTAT] & 3) != 0' --session hblank --rom build/racer.gb
gbcli break StatScyWrite --if '([rSTAT] & 3) != 0' --session hblank
gbcli run --session hblank --keys-file tools/win.keys --until '[wState] == 2' --frames 3000
#   stop predicate … frame=2715 … text="[wState] == 2"     (not "stop breakpoint")
```

**It never drops a frame in the race (60 fps).** The frame budget over the winning lap, summarized without the frames before the race starts (the title, and the LCD-off loads, which never halt and so count as lag):

```sh
gbcli run build/racer.gb --session budget --keys-file tools/win.keys --until '[wState] == 2' --frames 3000 --frame-budget --json > build/budget.json
python3 -I tools/budget_summary.py build/budget.json 40
#   frames 2676 after skipping 40: lag 0 []
#   active max 66296 (94.4%) at index 789, mean 58527 (83.3%)
```

**The same game in color.** After `gbcli build --gbc`, the same keys win on a GBC and, with the same ROM, on a DMG:

```sh
gbcli run build/racer.gb --session gbc --model gbc --keys-file tools/win.keys --until '[wState] == 2' --frames 3000 --read '[wTimeSec]'
#   stop predicate … frame=2718 …;  wTimeSec 7
gbcli run build/racer.gb --session gbc-on-dmg --keys-file tools/win.keys --until '[wState] == 2' --frames 3000
#   stop predicate … frame=2715 …
```

**The assets regenerate byte for byte:** `shasum assets/*/*.png`, then `python3 -I tools/make_assets.py`, then `shasum` again.

## Build warnings

None, in either build. The game was built with gbcli at `4381e6a`, where plain `gbcli build` printed four `shared_shade` warnings for sprite colors sharing a DMG shade with other images' colors. Since `372d772`, gbcli compares shared shades only among images of one named palette, so those warnings are gone. No single image holds two colors of one shade, so nothing inside a sprite or tile merges.

## Layout

- `src/main.asm`: startup, the title screen, the race loop, VBlank, the joypad, GBC colors.
- `src/road.asm`: the HBlank and timer interrupts and the scroll tables. `src/road_tables.asm` is generated.
- `src/drive.asm`: speed, steering, the track. `src/objects.asm`: rivals, trees, collisions. `src/race.asm`: the countdown, the clock, the HUD.
- `src/music.asm`, `src/sfx.asm`, and hUGEDriver (`src/hUGEDriver.asm`, `include/hUGE*.inc`, public domain).
- `tools/`: the asset generators (`make_assets.py` runs them all), the lap bot, the keys files and the budget summary.
- `include/hardware.inc`: gbdev's hardware.inc v6.0.0 (CC0).

Built with `gbcli new`; the developer-experience notes are in [project/2026-11-08-dx-racer.md](../../project/2026-11-08-dx-racer.md).
