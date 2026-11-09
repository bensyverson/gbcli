# Lantern

A first-person maze game for the Game Boy: find three keys and get out before
your lamp runs dry.

Lantern is a raycaster written in RGBDS assembly. Every view is cast one ray
per 2-pixel column into a WRAM buffer laid out as the view's tiles. The
buffer is then copied into VRAM during VBlank only. It was built by an agent
using [gbcli](../../README.md) as an ordinary user, and every claim below
comes with the gbcli command that checks it. Built with gbcli at commit b30becb
and RGBDS 1.0.4.

Play the ROM: [`lantern.gb`](lantern.gb), in any Game Boy emulator (in color on a Game Boy Color).

## Play

```sh
gbcli build                      # build/raycaster.gb, for the Game Boy
gbcli build --gbc                # build/gbc/raycaster.gbc, colored on a Game Boy Color
gbcli run build/raycaster.gb --session play --frames 120 --keys 'START@10 -START@12' --shot maze.png
```

- **START** leaves the title.
- **Left and right** turn. **Up and down** walk; you slide along walls.
- Keys are the glowing white blocks: walk into one to take it.
- The exit door is the light gray block on the maze's edge. Walk into it holding all
  three keys.
- The lamp's oil bar empties over 6,400 frames (about 107 s). When it is gone, so are
  you.
- **START** after either ending returns to the title.

## What's notable

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
  It matches `gbcli shot` pixel for pixel at 12 poses, on both machines.
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
the HUD and the music), the same walk measures 3.80 frames a view on the DMG (15.8
views a second) and 2.08 on the GBC.

## The commands that prove it

Run them from this folder; `gbcli` is the gbcli binary.
`python3 tools/verify.py --gbcli gbcli` reruns them all, in fresh sessions, and says
which hold (about 30 s).

**Both builds are clean:** each prints `errors=0 warnings=0`.

```sh
gbcli build
gbcli build --gbc
```

**The walls are right.** Twelve poses: seven walks, and five set with `--poke`
to face each key and the door. Each is compared pixel for pixel with the reference,
which also checks that the view on screen was cast from where the player stands. Both
print `ok 12 of 12 views`.

```sh
gbcli run build/raycaster.gb --session ray --frames 120 --keys 'START@10 -START@12' --checkpoint start
python3 tools/check_poses.py --gbcli gbcli --session ray
gbcli run build/gbc/raycaster.gbc --session color --model gbc --frames 120 --keys 'START@10 -START@12' --checkpoint start
python3 tools/check_poses.py --gbcli gbcli --session color
```

**Speed.** Every view casts within 280,000 T (4 frames); this exits 1 if one
doesn't. The second command prints the cast and the view period on both machines.

```sh
gbcli measure MainLoop..MainLoop.waitCopy --session ray --at start --iterations 25 --frames 2000 --keys-file keys/walk.keys --max 280000
python3 tools/bench.py --gbcli gbcli ray color
```

**It can be won, and lost.** The bot plays from the checkpoint through gbcli: it
plans paths, turns and walks, and won in 108 gbcli calls and about 4 s. It records its
keys in `keys/win.keys`, which replays the win in one run on either machine. The lose
run walks a little, then waits for the lamp. Back at the title, START starts a fresh
game.

```sh
python3 tools/bot.py --gbcli gbcli --session ray
gbcli run --session ray --at start --frames 4060 --keys-file keys/win.keys --expect '[wState] == 1'
gbcli run --session ray --at start --until '[wState] == 2' --frames 7000 --keys-file keys/lose.keys
gbcli run --session ray --frames 30 --keys 'START@5 -START@7' --expect '[wMode] == 0'
gbcli run --session ray --frames 60 --keys 'START@5 -START@7' --expect '[wMode] == 1' --expect '[wKeys] == 0'
```

**No VRAM write lands outside VBlank.** Break wherever the PPU's mode isn't VBlank
at four points: the end of OAM DMA, of the HUD update, of the view copy, and the
LCDC flip. Add the dropped-write watches from `gbcli help traps` on each set's last
byte, then play the whole winning game. Nothing stops it.

```sh
gbcli run build/raycaster.gb --session proof --frames 120 --keys 'START@10 -START@12' --checkpoint start
gbcli break VBlank.oamDone --if '([rSTAT] & 3) != 1' --session proof
gbcli break VBlank.hudDone --if '([rSTAT] & 3) != 1' --session proof
gbcli break CopyChunks.copied --if '([rSTAT] & 3) != 1' --session proof
gbcli break ShowView.flipped --if '([rSTAT] & 3) != 1' --session proof
gbcli watch write '$97FF' --if '[$97FF] != new' --session proof
gbcli watch write '$87FF' --if '[$87FF] != new' --session proof
gbcli run --session proof --at start --frames 4060 --keys-file keys/win.keys --expect '[wState] == 1'
```

**The music leaves channel 1 to the effects.** Over a whole game, every write to
channel 1 comes from `SfxTick`. The second command decodes the first key's chime
(C6, E6, G6, C7).

```sh
gbcli run --session ray --at start --frames 4060 --keys-file keys/win.keys --audio-summary --audio-channel 1
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
- `src/title.asm`: the title screen.
- `src/music.asm`, `src/sfx.asm`, plus hUGEDriver: the music and the sound effects.
- `src/tables.asm` and `src/maze.asm`: generated.
- `assets/`:
  - `bg/title.pixels` and `tiles/font.pixels`: generated pixel text.
  - `music/*.song`: gbcli song text, by hand.
  - `palettes.txt`: the image palettes.
- `tools/`: the generators, the reference renderer (`raymath.py`, `reference.py`,
  `check_poses.py`), `bench.py`, `bot.py` and `verify.py`. Python standard library
  only.
- `keys/`:
  - `walk.keys`: the scripted walk.
  - `win.keys`: the bot's game.
  - `lose.keys`.
  - `bench.keys`: equiv's frame-locked bench, entered by holding SELECT at power-on.

The view's math lives in two places, `tools/raymath.py` and the ROM; the reference
check is what keeps them in step. hUGEDriver is public domain, and
`include/hardware.inc` is gbdev's (CC0).
