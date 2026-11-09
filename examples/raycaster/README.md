# Lantern

A first-person maze game for the Game Boy: find three keys and get out before
your lamp runs dry.

Lantern is a raycaster written in RGBDS assembly. Every view is cast one ray
per 2-pixel column into a WRAM buffer laid out as the view's tiles. The
buffer is then copied into VRAM during VBlank only. It was built by an agent
using [gbcli](../../README.md) as an ordinary user, and every claim below
comes with the gbcli command that checks it. Built with gbcli at commit b30becb
and RGBDS 1.0.4; the floating keys and the retuned lamp were added with gbcli
at commit 70163ab, after a play-test.

Play the ROM: [`lantern.gb`](lantern.gb), in any Game Boy emulator (in color on a Game Boy Color).

## Play

```sh
gbcli build                      # build/raycaster.gb, for the Game Boy
gbcli build --gbc                # build/gbc/raycaster.gbc, colored on a Game Boy Color
gbcli run build/raycaster.gb --session play --frames 120 --keys 'START@10 -START@12' --shot maze.png
```

- **START** leaves the title.
- **Left and right** turn. **Up and down** walk; you slide along walls.
- Keys float in the air, bobbing when near: brass on a Game Boy Color, light gray
  and white with a black outline on a Game Boy. Walk through one to take it.
- The exit door is the light gray block on the maze's edge. Walk into it holding all
  three keys.
- The lamp's oil bar empties over 9,600 frames (160 s). Each key you take tops it up
  by a quarter (40 s), never past full. When the oil is gone, so are you.
- **START** after either ending returns to the title.

### Why the lamp lasts this long

The first version gave the lamp 6,400 frames and no refills. The bot won with 37% to
spare, but it knows the map. A play-tester who didn't know it got one key before
the lamp died. `tools/human.py` plays like a person who is lost. It follows the
left-hand wall, so it walks into every dead end on its way and back out. It stops
to look at every bend and fork, and it overshoots each turn and corrects it. That
walk is 212 cells and takes 10,626 frames, far longer than the old lamp's 6,400.
On the old build it ran dry holding two keys (the run is in the project's DX
notes, `project/2026-11-09-dx-raycaster.md`).

Doubling the lamp alone would leave the bot two-thirds of it and no pressure. So
there are two changes. The lamp lasts 9,600 frames, half again the old one. And
each key refills a quarter, which pays back a player who keeps finding things. A
lost player can draw on up to 16,800 frames. The left-hand walk wins with 70 of 128
pixels left (54%), and the bot with 117. A player could wander about half again as
long as the left-hand walk and still get out. The map, the keys' places and the
walking and turning speeds are unchanged.

## What's notable

- **The keys are objects, placed in 3D.** Each key still in the maze is turned into
  the camera's frame (depth and sideways offset, four signed multiplies), placed by
  a table of pixels per cell for its depth, and drawn as one of four sizes of an
  8x16-object sheet (`assets/sprites/key.24x16.pixels`, built by `gbcli build`). Each
  of its objects is hidden where the wall in its middle column is nearer, from a
  depth the ray loop keeps per column, so a key goes behind a corner in 8-pixel
  steps. There is one shadow OAM page per view tile set. A key's objects reach the
  screen in the same frame as the walls they were cast with.
- **The view is 128x64 pixels, 64 rays, flat-shaded.** It uses 8.8 fixed point,
  1,024 angle units a turn, and a DDA with one specialized loop per quadrant. A
  quarter-square table does the multiplies, and a table maps distance to wall height.
  A 2-pixel column is a pair of bits in a tile row.
- **Tiles, not pixels.** A view is 128 tiles in two VRAM sets: $8000 and $9000,
  each with its own BG map. The game draws into the set not on screen, then flips
  LCDC bits 4 and 3 in VBlank, so a half-drawn view is never shown. Only *edge* tiles
  (where a wall starts) are drawn and copied. All-ceiling, all-floor and all-stone
  tiles point at 20 shared tiles through the map, and only the map rows that changed
  are copied.
- **All VRAM writes happen in VBlank,** after OAM DMA. A copy chunk starts only while LY
  is below 152. That is checked over a whole game (below).
- **It casts while it copies.** There are two WRAM buffers: the main loop casts the
  next view while the VBlank handler copies the last.
- **A host reference renderer** (`tools/raymath.py`, `tools/reference.py`) uses the
  ROM's own integer steps and tables (`tools/gen_tables.py` writes `src/tables.asm`).
  It matches `gbcli shot` pixel for pixel at 16 poses, on both machines. It also
  places the keys and checks them against OAM (`gbcli view oam`).
- **The GBC build** runs at double speed (the DMG build doesn't), in lamplight colors.
  The same ROM runs on both.

### The optimization rounds

Each round followed `gbcli help optimize`:

- the hot spot from `run --profile` over a scripted walk (`keys/walk.keys`);
- the change;
- cycles before and after with `gbcli measure`: *cast* is `MainLoop..MainLoop.waitCopy`
  and *period* is `ShowView..ShowView` (one view on screen to the next), both from
  `tools/bench.py`;
- a check that the picture didn't change: `gbcli equiv` in a frame-locked bench mode,
  or the reference renderer.

All figures are DMG T-cycles on the original test map (ray2). The full table, with
each profile line, is in the project's DX notes
(`project/2026-11-09-dx-raycaster.md`).

| # | Change | cast | period (frames) |
|---|---|---|---|
| 0 | Baseline: per-pixel read-modify-write, 160 bytes copied per VBlank | 547,179 | 20.32 |
| 1 | Whole rows per tile column, from 5 sorted spans | 328,460 | 17.32 |
| 2 | A second buffer: cast while copying | 347,943 | 13.00 |
| 3 | Copy only edge tiles; shared tiles via double-buffered maps | 348,926 | 4.88 |
| 4 | Quarter-square multiply | 325,228 | 4.56 |
| 5 | DDA in registers, one loop per quadrant | 295,731 | 4.20 |
| 6 | Write only edge-tile rows (measured on a copy that overran VBlank) | 232,406 | 3.72 |
| 7 | Fix: OAM DMA first, copy only while LY < 152 | 226,276 | 4.32 |
| 8 | Copy only changed map rows (gained 3.7%: stopped) | 234,543 | 4.16 |

From 20.3 to 4.2 frames a view on the DMG is 4.9 times faster. The target was 4
frames, and it was reached only by the unsafe round 6. In the finished game (the maze,
the HUD and the music), the same walk measured 3.80 frames a view on the DMG (15.8
views a second) and 2.08 on the GBC.

The floating keys cost 5% of the cast and 7% of the view rate on the DMG. Before and after,
with `tools/bench.py` on the same walk:

| Build | cast mean | cast max | period (frames) |
|---|---|---|---|
| DMG, key blocks | 249,718 | 275,092 | 3.80 |
| DMG, floating keys | 262,462 | 291,504 | 4.08 |
| GBC, key blocks | 245,645 | 255,976 | 2.08 |
| GBC, floating keys | 256,083 | 270,568 | 2.16 |

With all three keys in the maze, `PlaceKeys` takes 7,648 T to 20,440 T a view,
9,325 T on average (`gbcli measure PlaceKeys..DrawView` over the walk; the larger
figures include VBlank interrupts). A key
beyond the farthest wall costs only its depth. The ray loop keeps each column's
depth in 10 more M-cycles, 2,560 T a view. The rest of the period is VBlank's. A
view copied too late in VBlank for a second OAM DMA waits for the next VBlank to
flip, so that its keys and its walls change in the same frame. The worst view on
the walk now casts in 4.15 frames, past the 4-frame target.

## The commands that prove it

Run them from this folder; `gbcli` is the gbcli binary.
`python3 tools/verify.py --gbcli gbcli` reruns them all, in fresh sessions, and says
which hold (two or three minutes).

**Both builds are clean:** each prints `errors=0 warnings=0`.

```sh
gbcli build
gbcli build --gbc
```

**The walls and the keys are right.** Sixteen poses: seven walks, and nine set with
`--poke` to face each key and the door, every size of the key, and a key half behind
a wall. Each is compared pixel for pixel with the reference, except under the key
objects. The reference also checks that the view on screen was cast from where the
player stands. It checks the objects in OAM too: each key's size, place, tiles and
bob, and which objects a nearer wall hides. Both print `ok 16 of 16 views`. The
`--shot` runs save a key two cells ahead on each machine, for a look.

```sh
gbcli run build/raycaster.gb --session ray --frames 120 --keys 'START@10 -START@12' --checkpoint start
python3 tools/check_poses.py --gbcli gbcli --session ray
gbcli run build/gbc/raycaster.gbc --session color --model gbc --frames 120 --keys 'START@10 -START@12' --checkpoint start
python3 tools/check_poses.py --gbcli gbcli --session color
gbcli run --session ray --at start --frames 60 --poke wPosX=128,12 --poke wPosY=160,1 --poke wAngle=0,0 --shot key-dmg.png
gbcli run --session color --at start --frames 60 --poke wPosX=128,12 --poke wPosY=160,1 --poke wAngle=0,0 --shot key-gbc.png
```

**Speed.** Every view casts within 300,000 T (4.27 frames; it was 280,000 before the
keys); this exits 1 if one doesn't. The second command prints the cast and the view
period on both machines.

```sh
gbcli measure MainLoop..MainLoop.waitCopy --session ray --at start --iterations 25 --frames 2000 --keys-file keys/walk.keys --max 300000
python3 tools/bench.py --gbcli gbcli ray color
```

**It can be won, and lost.** The bot plays from the checkpoint through gbcli: it
plans paths, turns and walks, and won at frame 4,179 (`oil=117/128`) in 113 gbcli
calls and a few seconds. It records its keys in `keys/win.keys`, which
replays the win in one run on either machine. The lose run walks a little, then
waits for the lamp. On the title, a view still waiting to be shown (`wFlipReady`,
poked) leaves the title's LCDC alone. Back at the title, START starts a fresh game.

```sh
python3 tools/bot.py --gbcli gbcli --session ray
gbcli run --session ray --at start --frames 4180 --keys-file keys/win.keys --expect '[wState] == 1'
gbcli run --session ray --at start --until '[wState] == 2' --frames 10000 --keys-file keys/lose.keys
gbcli run --session ray --frames 30 --keys 'START@5 -START@7' --expect '[wMode] == 0'
gbcli run --session ray --frames 3 --poke wFlipReady=1 --expect '[rLCDC] == $91'
gbcli run --session ray --frames 60 --keys 'START@5 -START@7' --expect '[wMode] == 1' --expect '[wKeys] == 0'
```

**A lost human can win.** `tools/human.py` plays the left-hand walk described under
[Why the lamp lasts this long](#why-the-lamp-lasts-this-long), at a person's pace,
through gbcli. It prints `won at frame 10626; keys=3 oil=70/128 (54%)` and records
its game in `keys/human.keys`. Replayed on either machine, the game wins with at
least half the lamp left.

```sh
python3 tools/human.py --gbcli gbcli --session ray
gbcli run --session ray --at start --frames 10627 --keys-file keys/human.keys --expect '[wState] == 1' --expect '[wOilPx] >= 64'
gbcli run --session color --at start --frames 10627 --keys-file keys/human.keys --expect '[wState] == 1' --expect '[wOilPx] >= 64'
```

**No VRAM write lands outside VBlank.** Break wherever the PPU's mode isn't VBlank
at five points: the end of OAM DMA, of the HUD update, of the view copy, the LCDC
flip, and the end of the OAM DMA that shows a flipped view's keys. Add the
dropped-write watches from `gbcli help traps` on each set's last byte, then play the
whole winning game, and the lost human's. Nothing stops either.

```sh
gbcli run build/raycaster.gb --session proof --frames 120 --keys 'START@10 -START@12' --checkpoint start
gbcli break VBlank.oamDone --if '([rSTAT] & 3) != 1' --session proof
gbcli break VBlank.hudDone --if '([rSTAT] & 3) != 1' --session proof
gbcli break CopyChunks.copied --if '([rSTAT] & 3) != 1' --session proof
gbcli break ShowView.flipped --if '([rSTAT] & 3) != 1' --session proof
gbcli break ShowView.oamDone --if '([rSTAT] & 3) != 1' --session proof
gbcli watch write '$97FF' --if '[$97FF] != new' --session proof
gbcli watch write '$87FF' --if '[$87FF] != new' --session proof
gbcli run --session proof --at start --frames 4180 --keys-file keys/win.keys --expect '[wState] == 1'
gbcli run --session proof --at start --frames 10627 --keys-file keys/human.keys --expect '[wState] == 1'
```

**The music leaves channel 1 to the effects.** Over a whole game, every write to
channel 1 comes from `SfxTick`. The second command decodes the first key's chime
(C6, E6, G6, C7).

```sh
gbcli run --session ray --at start --frames 4180 --keys-file keys/win.keys --audio-summary --audio-channel 1
gbcli run --session ray --at start --frames 600 --keys-file keys/win.keys --audio-log --audio-channel 1
gbcli run build/raycaster.gb --session music --frames 900 --audio-summary --piano title-roll.png
```

**Every generated file regenerates byte for byte:** each prints `ok:`.

```sh
python3 tools/gen_font.py --check     # assets/tiles/font.pixels, include/charmap.inc
python3 tools/gen_title.py --check    # assets/bg/title.pixels
python3 tools/gen_maze.py --check     # src/maze.asm
python3 tools/gen_tables.py --check   # src/tables.asm
```

## What is here

- `src/main.asm`: startup, the main loop (cast, hand off), the VBlank handler,
  and the maze's video setup.
- `src/render.asm`, `src/cast.asm`, `src/draw.asm`: per-ray setup and the multiply;
  the DDA; the tile-column spans, the view's map and the copy list.
- `src/copy.asm`: the VBlank copy and the flip.
- `src/game.asm`, `src/player.asm`: rules, HUD, input and movement.
- `src/sprite.asm`: the floating keys, from the camera to OAM.
- `src/title.asm`: the title screen.
- `src/music.asm`, `src/sfx.asm`, plus hUGEDriver: the music and the sound effects.
- `src/tables.asm` and `src/maze.asm`: generated.
- `assets/`:
  - `bg/title.pixels` and `tiles/font.pixels`: generated pixel text.
  - `sprites/key.24x16.pixels`: the key in four sizes, pixel text by hand.
  - `music/*.song`: gbcli song text, by hand.
  - `palettes.txt`: the image palettes.
- `tools/`: the generators, the reference renderer (`raymath.py`, `reference.py`,
  `check_poses.py`), `bench.py`, `bot.py`, `human.py` and `verify.py`. Python
  standard library only.
- `keys/`:
  - `walk.keys`: the scripted walk.
  - `win.keys`: the bot's game.
  - `human.keys`: the lost human's game (`tools/human.py`).
  - `lose.keys`.
  - `bench.keys`: equiv's frame-locked bench, entered by holding SELECT at power-on.

The view's math lives in two places, `tools/raymath.py` and the ROM; the reference
check is what keeps them in step. hUGEDriver is public domain, and
`include/hardware.inc` is gbdev's (CC0).
