# Lanternfall

A Game Boy dungeon crawler: carry your lantern down five floors of a dungeon that is built fresh from a seed, fight what lives there, and bring back the Ember.

Every floor is generated in assembly while you play: rooms, corridors, stairs, creatures and items, on a 64x48 map that scrolls past the 32x32 hardware background. The game is turn-based: you step, then everything else steps. One seed always makes the same dungeon. It was written by an AI agent using gbcli as a user (from `gbcli help` and the repo README only), with every picture, song and check made the same way. It runs in four shades on the original Game Boy and in color on a Game Boy Color.

## Play

The ROM is [`lanternfall.gb`](lanternfall.gb) (one file for both machines). On the title screen, press START; the moment you press it picks the seed.

- **D-pad**: step one cell (hold to keep walking). Stepping into a creature attacks it.
- **B**: wait a turn.
- Walk over a **potion** to drink it (+8 HP) and over the **blade** (floor 2) to wield it (attack 2 -> 4).
- Take the **stairs** down. On floor 5 the stairs are replaced by the **Ember**: take it to win. At 0 HP your lantern gutters out.
- Rats bite for 1; ghouls (from floor 3) bite for 2. A creature notices you within 9 steps and closes in.

After a win or a death, START goes back to the title.

## Build

```sh
cd examples/rogue
gbcli build                                  # the DMG ROM, build/rogue.gb
gbcli build --gbc --out build/rogue-gbc.gb   # the dual-mode ROM (this is lanternfall.gb)
```

Both builds finish with `errors=0 warnings=0`, images `converted=5 errors=0 warnings=0`.

## What's technically notable, and the commands that show it

Run these from `examples/rogue/` after both builds. The Python tools are standard library only; they call `gbcli` from `PATH` or from `GBCLI=/path/to/gbcli`. Key files in `test/` count frames since boot, and were recorded by the bots below.

**Every floor is connected.** Rooms are placed at random without touching and each is joined to the one before it by an L-shaped corridor, so every floor cell is reachable from the start by construction. A tool reads the map out of WRAM (3,072 bytes, through `gbcli read`) and checks it with a flood fill, for seven seeds:

```sh
python3 -I tools/maptool.py seeds 20 57 101 160 233 400 1000
# ok   frame=57 seed=50 depth=0 rooms=10 floor=542 reachable=542 items=1 stairs_reachable=True  (and six more)
```

**One seed, one dungeon.** Two runs that press START on the same frame (one mashing A and B on the title first) hold byte-identical maps; a frame later gives another seed and another map:

```sh
gbcli run build/rogue.gb --session same-a --frames 130 --keys 'START@57 -START@59'
gbcli run build/rogue.gb --session same-b --frames 160 --keys 'A@10 -A@12 B@30 -B@40 START@57 -START@59'
python3 -I tools/maptool.py dump --session same-a --raw build/same-a.bin
python3 -I tools/maptool.py dump --session same-b --raw build/same-b.bin
cmp build/same-a.bin build/same-b.bin && echo identical
```

The seed holds across machines too: the GBC build reaches seed 50 with START on frame 59, and the same bot plays the same 632-turn game on both (`test/win-gbc.keys` vs `test/win-seed50.keys`, below).

**A map bigger than the background, streamed.** The camera follows the player over the 64x48 map; each step queues the newly exposed row or column of the 32x32 background ring, and the VBlank handler copies it in. The bot below checks the background against the map under the camera every 10 turns:

```sh
gbcli run build/rogue.gb --session viewcheck --frames 130 --keys 'START@57 -START@59'
python3 -I tools/maptool.py play --session viewcheck --mode win --check-view 10
# view checks: 63 ok, 0 failed
# won after 632 turns at depth 4 hp=18 frame=2818
```

**A floor generates with the LCD on and the music playing.** The palettes go dark, the generator yields to the next frame whenever it reaches line 80, the 19 view rows stream in one a frame, and the palettes come back: about 0.4 s, with the game halting every frame. Generation alone spans just over 4 frames (288,560 T-cycles of wall time, about 200,000 of them CPU work):

```sh
gbcli run build/rogue.gb --session gen --frames 50 --checkpoint title
gbcli measure GenerateFloor..GenerateFloor.done --session gen --keys 'START@7 -START@9'
# t min=288560 … frame_pct min=410.914
```

**A scripted win, and a scripted death.** `maptool.py play` is a closed-loop bot: each turn is one `gbcli run --frames 4 --keys … --read …` that presses a key and reads the player, HP and the eight creatures back. It fights what's adjacent, drinks when low, takes the blade and heads down (`--mode win`), or walks into the nearest creature and waits (`--mode die`). Its recorded keys replay from boot in one run:

```sh
gbcli run build/rogue.gb --session replay-win --frames 2900 --keys-file test/win-seed50.keys --frame-budget --read '[wGameState]' --read '[wDepth]'
# read expr=[wGameState] value=1   (won)   read expr=[wDepth] value=4   frame_budget frames=2900 lag=0
gbcli run build/rogue.gb --session replay-die --frames 400 --keys-file test/die-seed50.keys --read '[wGameState]'
# read expr=[wGameState] value=2   (dead)
gbcli run build/rogue-gbc.gb --session replay-gbc --model gbc --frames 2900 --keys-file test/win-gbc.keys --frame-budget --read '[wGameState]'
# read expr=[wGameState] value=1   frame_budget frames=2900 lag=0
```

**No lag frames, even on the busiest floor.** The whole win above shows `lag=0` (its LCD-off frames are startup, the title-to-dungeon swap and the end screen). Floor 5, with six creatures chasing (ghouls among them), from the frame it starts (found with `--until '[wDepth] == 4'`) to the win:

```sh
gbcli run build/rogue.gb --session floor5 --frames 3000 --keys-file test/win-seed50.keys --until '[wDepth] == 4'
# stop predicate … frame=1683
python3 -I tools/budget.py --from 1683 -- build/rogue.gb --session busy --frames 2820 --keys-file test/win-seed50.keys
# frame_budget from=1683 frames=1137 lag=0 … active_max=47992
```

**Music on three channels, effects on the fourth.** Both songs (`assets/music/*.song`, gbcli song text) leave channel 2 empty and the driver has it muted, so the step, hit, hurt, pick-up and stairs effects own it. Over the whole win, every channel-2 write but the driver's two at song start comes from the effect player (`SfxVBlank`, $00AB-$00E0 by `gbcli sym 'Sfx*'`):

```sh
gbcli run build/rogue.gb --session audio --frames 2900 --keys-file test/win-seed50.keys --audio-summary
gbcli run build/rogue.gb --session audio --from boot --frames 260 --keys-file test/win-seed50.keys --audio-log --audio-channel 2 --piano build/audio/short.png --scale 1
```

**Title to descent to result, and back.**

```sh
gbcli run build/rogue.gb --session loop --frames 2900 --keys-file test/win-seed50.keys --read '[wGameState]'   # 1: the win screen
gbcli run --session loop --frames 120 --keys 'START@10 -START@12' --read '[rLCDC]'                         # 145 ($91): the title again
gbcli run --session loop --frames 120 --keys 'START@30 -START@32' --read '[wDepth]' --read '[wHP]'          # 0 and 20: a new run
```

## Art and sound

Every image is pixel text under `assets/` (`gbcli help pixels`), read by the build directly. Two are generated, and regenerate byte-identical:

```sh
python3 -I tools/title.py    # assets/bg/title.pixels: the lantern, its dithered glow, the name
python3 -I tools/font.py     # assets/tiles/font.pixels: the 5x7 font of the status line and end screens
```

The palettes are in `assets/palettes.txt`, one per kind of thing (dungeon, title, lantern-bearer, creatures), all ordered light to dark under one `dmg:` line so the DMG build uses the same four shades everywhere.

## Layout

- `src/main.asm` startup, main loop, VBlank handler; `src/dungeon.asm` the generator; `src/view.asm` the camera and streaming; `src/player.asm` the player's turn; `src/enemies.asm` the creatures; `src/text.asm` the status line and end screens; `src/sfx.asm` the sound effects; `src/music.asm` the songs; `src/rng.asm` the xorshift; `src/tiles.asm` loading the art. `src/hUGEDriver.asm` and `include/` came with `gbcli new`.
- `tools/maptool.py` (dump, check, seeds, view, play), `tools/budget.py`, `tools/title.py`, `tools/font.py`.
- `test/*.keys` the bots' recorded runs.

## Credits

By rogue15, an AI agent, for gbcli's examples. hUGEDriver is public domain; `include/hardware.inc` is gbdev's (CC0).
