# Vesper Swarm

A space shooter for the Game Boy: fly through a swarm at nightfall, dodge
rings of bullets, and bring down the warship at the end.

Vesper Swarm is one stage and a boss, written in RGBDS assembly by an AI
agent using [gbcli](../../README.md) as an ordinary user. The code, the
pixel art (drawn by Python scripts), the music and the checks below are all
the agent's. It runs on the original Game Boy and, built with `--gbc`, in
color on a Game Boy Color.

## Build and play

**Just want to play?** Download [`vesper-swarm.gb`](vesper-swarm.gb) and open it in any Game Boy emulator. It's one ROM for both machines: in color on a Game Boy Color, in four shades on the original Game Boy. Rebuild it with `scripts/examples/build-roms.sh`.

You need gbcli and RGBDS (`gbcli doctor` checks them). From this folder:

```sh
gbcli build                                   # build/shooter.gb
gbcli build --gbc --out build/shooter-gbc.gb  # the color build
gbcli run build/shooter.gb --session play --frames 600 --keys 'START@42 -START@44' --shot play.png
```

Any emulator plays `build/shooter.gb`.

## How to play

- **D-pad** flies the ship; **A** fires (hold it for autofire).
- Darts, weavers and pods come in waves for about 35 seconds; then the
  warship arrives. Its health shows as `B` and a number in the status bar.
- Three lives. A hit costs one and clears the bullets on screen; the ship
  blinks for two seconds while nothing can hit it.
- Score: dart 100, weaver 150, pod 500, warship 10,000.
- On the title screen, **SELECT** opens the sprite test (48 bullets
  bouncing); **START** goes back.

## What's technically notable

- **More sprites than the hardware draws.** The Game Boy holds 40 sprites
  and draws at most 10 on a line. Game code adds up to 64 to a list each
  frame; a multiplexer (`src/sprites.asm`) packs them into OAM forward on
  even frames and backward on odd ones, from a start that steps back every
  second frame. Whatever doesn't fit flickers instead of vanishing.
- **Parallax from mid-frame scroll splits.** Four bands of sky scroll at 1/8,
  1/4, 1/2 and 1 pixel a frame. An LY=LYC interrupt on lines 39, 87 and 119
  waits for that line's HBlank and only then writes SCX
  (`src/parallax.asm`), so no write lands while the line is drawn.
- **Music with sound effects.** Two hUGEDriver songs written as gbcli song
  text (`assets/music/`). The stage theme leaves channel 4 to the effects
  (`src/sfx.asm`): shot, hit and explosion.
- **Every image is drawn by a script.** `python3 -I tools/draw_art.py` draws
  all ten PNGs in `assets/`; `assets/palettes.txt` gives each its palette,
  so one set of tiles serves the DMG and, colored, the GBC.

## The claims, and the commands that check them

Run these from this folder after `gbcli build`. Each was rerun on the
final build.

**Both builds are clean.** `gbcli build` and `gbcli build --gbc --out
build/shooter-gbc.gb` end `errors=0 warnings=0`, after four `shared_shade`
image warnings (below).

**The art regenerates byte for byte.**

```sh
shasum assets/bg/*.png assets/tiles/*.png > build/art.sha
python3 -I tools/draw_art.py && shasum -c build/art.sha    # 10 OK
```

**A scripted run beats the boss; another reaches game over.**
`keys/win.keys` was recorded by `tools/bot.py`, an autopilot that plays
through gbcli and writes down what it pressed. gbcli is deterministic, so
the file replays the same game:

```sh
gbcli run build/shooter.gb --session win --frames 7000 --keys-file keys/win.keys \
    --until '[wState] == 2' --read '[wLives]' 2>/dev/null | grep -v key_unapplied
#  stop predicate … frame=4552 text="[wState] == 2"   (STAGE CLEAR, 3 lives, 029000 points)
gbcli run build/shooter.gb --session over --frames 3000 --keys 'START@42 -START@44' \
    --until '[wState] == 3' --read '[wLives]'
#  stop predicate … frame=914 text="[wState] == 3"    (no input: GAME OVER)
gbcli run --session over --frames 90 --keys 'START@70 -START@72' --read '[wState]'
#  wState=0: START on a result goes back to the title
```

The same `keys/win.keys` also clears the color build on a GBC
(`--model gbc`, frame 4552).

**No lag frames in play, at 60 frames a second.**

```sh
gbcli run build/shooter.gb --session budget --frames 7000 --keys-file keys/win.keys \
    --until '[wState] == 2' --frame-budget --json > build/budget.json
python3 -I tools/budget_peak.py build/budget.json
#  lag_frames=0 1 2 3 40 41 42 43 4551
#  busiest_frame … share=82.5%   busiest_window … share=55.2% lag=0
```

Every lag frame listed is one with the LCD off on purpose: start-up, then
the screen changes (to the title, to the stage, to the result), which load
tiles and maps with the screen dark. The busiest frame of play uses 82.5%
of the frame.

**More than 40 sprites at once.**

```sh
gbcli run build/shooter.gb --session peak --frames 4600 --keys-file keys/win.keys \
    --until '[wSprCount] >= 41' --read '[wSprCount]' 2>/dev/null | grep -v key_unapplied
#  stops at frame 1508 with 41 sprites in the frame's list (a pod's ring of 16)
```

**Every sprite gets drawn: the multiplexer.** The sprite test gives each of
its 48 bullets its own tile, so OAM shows which were drawn in each frame:

```sh
gbcli run build/shooter.gb --session mux --frames 160 --keys 'SELECT@42 -SELECT@44' --checkpoint test
python3 -I tools/oam_coverage.py --session mux --frames 120 --first 0xB0
#  objects=48 frames=120 drawn_some_frame=48 drawn_per_frame_min=30
#  longest_undrawn_run=2 never_drawn=none
gbcli run --session mux --at test --frames 600 --frame-budget
#  frame_budget frames=600 lag=0
```

No bullet goes undrawn for more than two frames in a row, with 48 sprites
and 16 of them on one line.

**The scroll splits land on their lines, in HBlank.**

```sh
gbcli run --session mux --at test --frames 1 --raster
#  rSCX written on lines 39, 87 and 119 (and in VBlank, line 145)
gbcli break StatSplit.write --session hb --rom build/shooter.gb \
    --if '([rSTAT] & 3) != 0 || (ly != 39 && ly != 87 && ly != 119)'
gbcli run --session hb --frames 7000 --keys-file keys/win.keys --until '[wState] == 2' 2>/dev/null | grep '^stop'
#  stops at the stage clear, never at the breakpoint: each split write was in
#  mode 0 on its line, through the whole winning run
```

The game was built with gbcli at `4381e6a`, whose `--raster` gave each write's line but not the PPU mode, so the breakpoint reads STAT at the write instruction. Since `7595c6a`, each `raster_write` also carries `mode=`, and every split write reads `mode=hblank`.

**The effects own channel 4 during play.**

```sh
gbcli run build/shooter.gb --session stage --frames 300 --keys-file keys/win.keys > /dev/null 2>&1
gbcli run --session stage --frames 600 --audio-log --piano build/stage-roll.png > build/stage-audio.txt
grep NR43 build/stage-audio.txt | sed 's/.*text="//' | sort | uniq -c   # shots ($0B), explosions ($67)
grep 'NR4[1-4]' build/stage-audio.txt | sed 's/.*pc=//' | cut -d' ' -f1 | sort | uniq -c
#  every channel-4 write comes from SfxPlay; the music plays on 1-3
gbcli run build/shooter.gb --session audio --frames 640 --audio-log --piano build/title-roll.png \
    | grep -o 'NR[1-4][1-4]' | cut -c1-3 | sort | uniq -c
#  64 NR1, 48 NR2, 48 NR3, 92 NR4: the title theme on all four channels
```

## Warnings, explained

None, in either build. With gbcli at `4381e6a`, both builds printed four `shared_shade` warnings, one per DMG shade: colors from different images falling to the same gray, which is by design (each image has its own GBC palette, and on the DMG every palette's lightest color is white). Since `372d772`, gbcli compares shared shades only among images of one named palette, so those warnings are gone. No image puts two of its own colors in one shade.

## Files

- `src/`: `main.asm` (start-up, VBlank, main loop), `game.asm` (screens,
  score, lives, pad), `player.asm`, `enemies.asm` (waves, boss),
  `bullets.asm`, `sprites.asm` (the multiplexer), `parallax.asm`,
  `gfx.asm` (loading the art), `font.asm` (text, status bar), `sfx.asm`,
  `music.asm`, `scene.asm` (the sprite test); hUGEDriver from `gbcli new`.
- `tools/`: `draw_art.py` (every image), `bot.py` (records
  `keys/win.keys`), `oam_coverage.py`, `budget_peak.py`.
- `assets/`: the images, `palettes.txt`, and the two songs.

Made with gbcli by an AI agent (Claude), November 2026. Notes on how the
tooling served it: `project/2026-11-08-dx-shooter.md`.
