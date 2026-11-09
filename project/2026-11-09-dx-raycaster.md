# DX notes: the raycaster (Lantern), built as a gbcli user

Agent `ray`, 2026-11-09, binary `/Users/ben/.cache/gbcli-examples/target/release/gbcli`
built at b30becb, RGBDS 1.0.4. The game lives in `examples/raycaster/`. I learned
gbcli only from `gbcli help`, `gbcli help TOPIC`, `gbcli recipes`, `gbcli VERB --help`
and the repo README. Every command below runs from `examples/raycaster/` (I passed
`--dir` to the same effect); `gbcli` stands for the binary's absolute path.

## 2026-11-09 — ray1 (rhoqoN): a raycast view drawn into tile memory

### What I built

- **The view:** 128x64 pixels (16x8 tiles) at tile (2, 2), one ray per 2-pixel column
  (64 rays), flat-shaded walls: east/west faces shade 3, north/south faces shade 2,
  ceiling 0, floor 1.
- **The math:** 8.8 fixed point, 1024 angle units a turn, a lodev-style DDA on a 16x16
  map. Per ray: `|1/cos|` from a 1024-entry table for both axes, two 16x8 multiplies
  for the first side distances, then 16-bit adds. A third multiply by `cos(column
  offset)` undoes the fisheye, and a 1024-entry table maps the perpendicular distance
  `>> 3` to a half height.
- **The buffer:** `wBuf` (2,048 bytes) is laid out column-major as the view's tiles:
  tile column `tx` at `tx * 128`, row `y` at `2y`. The background map numbers tile
  `(tx, ty)` as `tx * 8 + ty`, so VRAM has the same layout and the copy is one linear
  run.
- **The copy:** double-buffered in VRAM. Set A is tiles $00-$7F at $8000 (LCDC bit 4
  set) and set B is tiles $00-$7F at $9000 (bit 4 clear). One background map indexes
  both, so a flip is a single LCDC write, made in VBlank after the last chunk. The
  VBlank handler copies 10 chunks of 16 bytes per VBlank (20 on the GBC at double
  speed) with `pop de` / `ld [hl], e`, then OAM DMA, then input. The main loop casts
  into `wBuf` and then waits for the copy to finish. Tiles $80-$FF ($8800) belong to
  both sets and to sprites: they hold the HUD's shared tiles.
- **The tools** (Python stdlib, `tools/`): `raymath.py` holds the tables and the
  integer cast, `gen_tables.py` writes `src/tables.asm` from it (`--check` for
  staleness), `reference.py` compares one view, and `check_poses.py` walks to seven
  poses and compares each. `pngio.py` decodes gbcli's PNGs.
- The `--gbc` build switches to double speed (Ben's ruling allows it) and sets a gray
  BG palette. It draws the same picture.

### Criteria, and the commands that show them

**The reference matches `gbcli shot` pixel for pixel at several poses** (DMG and GBC):

```
gbcli build && gbcli build --gbc
gbcli run build/raycaster.gb --session ray --frames 120 --checkpoint start
python3 tools/check_poses.py --gbcli gbcli --session ray
  match x=$0280 y=$0280 a=0 pixels=8192 keys=''
  match x=$03FA y=$02BF a=78 pixels=8192 keys='RIGHT@0 -RIGHT@13 UP@13 -UP@40'
  match x=$0280 y=$0280 a=784 pixels=8192 keys='LEFT@0 -LEFT@40'
  match x=$01E0 y=$04D8 a=300 pixels=8192 keys='RIGHT@0 -RIGHT@50 UP@50 -UP@90'
  match x=$0140 y=$0280 a=514 pixels=8192 keys='LEFT@0 -LEFT@85 UP@85 -UP@140'
  match x=$0640 y=$0280 a=0 pixels=8192 keys='UP@0 -UP@60'
  match x=$05DE y=$059B a=6 pixels=8192 keys='RIGHT@0 -RIGHT@21 UP@21 ... -DOWN@200'
  ok 7 of 7 views
gbcli run build/gbc/raycaster.gbc --session color --model gbc --frames 120 --checkpoint start
python3 tools/check_poses.py --gbcli gbcli --session color      # ok 7 of 7 views
```

`reference.py` takes `gbcli shot --scale 1`, reads the pose of the view on screen
(`gbcli read 'u16[wViewX]' 'u16[wViewY]' 'u16[wViewA]'`), dumps the map (`gbcli view
memory wMap 256 --out map.bin`), renders it with the ROM's tables and compares the
8,192 pixels. It reads the four shades from a swatch on screen, so the same check works
against the GBC palette.

**No VRAM write lands in mode 3.** Every VRAM write after startup is in `CopyChunks`,
and `CopyChunks.copied` follows the last one in each VBlank. I broke there on any line
outside 144-151, and added the dropped-write watch from `gbcli help traps` on the last
byte of each set. Then I ran the 600-frame walk:

```
gbcli run build/raycaster.gb --session vram --frames 120 --checkpoint start
gbcli break CopyChunks.copied --if 'ly > 151 || ly < 144' --session vram
gbcli watch write '$97FF' --if '[$97FF] != new' --session vram
gbcli watch write '$87FF' --if '[$87FF] != new' --session vram
gbcli run --session vram --at start --frames 600 --keys-file keys/walk.keys
  stop budget_exhausted kind=frames ... frame=720 ...      # nothing fired
```

The positive control: `--if 'ly > 150'` stopped at frame 123 with `ly=151`. So the copy
ends on line 151, which leaves about 1,300 T of VBlank for OAM DMA. `gbcli measure
VBlank..CopyChunks.copied --iterations 20` gives the copy's real cost as the minimum,
2,896 T from the handler's entry. The maximum is the span to a later VBlank's copy,
because not every VBlank copies.

**View updates per second:**

```
gbcli measure MainLoop..MainLoop.waitCopy --session vram --at start --iterations 10
  t min=563060 max=563076 ... frame_pct min=801.806     # casting: 8.0 frames
gbcli run --session vram --at start --frames 600 --keys-file keys/walk.keys --read 'u16[wViews]'
  read expr=u16[wViews] value=34                        # 5 at start: 29 views in 600 frames
```

On the DMG: 8.0 frames to cast plus 12.8 to copy (128 chunks at 10 a VBlank) is 20.7
frames per view, **2.9 view updates a second**. On the GBC at double speed: 58 views
over the same walk (11 at start, 69 at the end), **5.8 a second**. That is a first cut;
ray2 optimizes it. The profile names the hot spots (`gbcli run --at start --frames 600
--keys-file keys/walk.keys --profile --top 12`): FillSpan 8.4M T, CastRay 2.9M T
exclusive, Mul16x8 2.8M T, and RenderView 1.6M T, out of 16.0M T inclusive.

**Both builds succeed:** `gbcli build` and `gbcli build --gbc` each print `errors=0
warnings=0`.

### Where gbcli helped

- **`gbcli new` plus `gbcli build`** gave a running, double-buildable project in two
  commands, `.gbcli/` included. (The brief warned me to make `.gbcli/` by hand; `new`
  already does.) My first ROM built and drew walls on the first run.
- **The reference loop caught a real bug in minutes.** The first walk mismatched
  2,712 pixels (`mismatch x=$03FA y=$02BF a=78 pixels=2712 first=(12,0)`).
  `--diff` showed the whole left half wrong, and the cause was a carry I had clobbered
  when adding the column's angle offset (`ld a, l / add 64` between the `add` and the
  `adc`). The pipeline needs only `shot --scale 1` (exact pixels), `read` (the pose) and
  `view memory --out` (the map), each one call, and it is what makes "the walls are
  right" checkable at all.
- **`measure` and `--profile` answered the frame-rate question in two calls.** They
  give the cast in T and the copy's place in VBlank. The `break ... --if 'ly > 151'`
  form turned "no VRAM write in mode 3" into a single run with a clear yes or no.

### Where it stumbled, costliest first

1. **`--frame-budget` printed 217 lag rows** (`frame_cost index=0 frame=120
   active_t=70228 idle_t=0 lag=1`, one per lag frame) for a renderer that is meant to
   take several frames per view. That is about 15 KB of output in my context, for a
   summary line I wanted. I expected the summary row and at most a few busiest frames,
   or a `--lag-rows N` cap. I used the summary row and ignored the rest.
2. **I can't list the armed breakpoints and watches.** Neither `gbcli show` nor `show
   --json` lists them. The VRAM proof is "nothing fired", so whether they are armed is
   the whole claim. `gbcli events --limit 20` was the only place their conditions
   appear. Also, after `gbcli delete 4` the next `break` got `id=4` again, so in a
   transcript two different breakpoints share an id.
3. **`break` and `watch` don't echo `--if`** (`breakpoint id=1 addr=$00C3
   symbol=CopyChunks.copied`), so a pasted transcript doesn't show the condition that
   makes the proof.
4. **The dropped-write watch covers one byte.** `gbcli help traps` shows `gbcli watch
   write '$8000' --if '[$8000] != new'`, but my copy writes 2,048 bytes into each of two
   sets. There is no range form (`watch write $8000..$97FF`). I watched the last byte of
   each set and proved the timing with a break on the copy's end; that works, but only
   because all my writes funnel through one routine.
5. **`measure FROM..TO` whose TO isn't reached every frame** gives a misleading max
   (565,300 T for a 2,896 T copy). That is correct by its definition, but a footgun: I
   had to know to read the min.
6. **`shot` writes a 2-bit palette PNG**, which `shot --help` doesn't say. My stdlib
   decoder first refused it, and I added 1/2/4-bit support.

## 2026-11-09 — ray2 (kUjuOP): profile-driven optimization

### Method

- **The walk:** `keys/walk.keys`, 600 frames of turning and walking from checkpoint
  `start` (120 frames after boot).
- **Two spans,** both measured by `tools/bench.py` (25 iterations of `gbcli measure`
  over the walk, `--frames 2000`):
  - *cast*, `MainLoop..MainLoop.waitCopy`: casting and drawing one view into a buffer.
  - *period*, `ShowView..ShowView`: one view on screen to the next. This is the number
    the player feels.
- **Each round:** build to `build/rN.gb`, then `gbcli run build/rN.gb --session rN
  --frames 120 --checkpoint start`, `python3 tools/bench.py --gbcli gbcli rN`,
  `python3 tools/check_poses.py --gbcli gbcli --session rN` (the reference, 7 poses,
  pixel for pixel), and `gbcli equiv build/rN.gb --session base3 --frames 720
  --keys-file keys/bench.keys`. Rounds 1 and 2 ran equiv against `base`.
- **The target:** a view period of 4 frames or less on the DMG (15 views a second),
  without double speed.

### Two things I had to build before `gbcli equiv` could check anything

1. **A frame-locked bench mode.** equiv compares the screen frame by frame. A faster
   renderer shows each view sooner, so a correct optimization fails at once:
   `equiv build/r1.gb --session base --frames 720` (unlocked) diverged at frame 27 with
   6,308 pixels. I added a mode, entered with SELECT held at power-on, in which a
   finished view is shown only on frames that are multiples of 32, and the next view's
   pose is taken on that frame. `keys/bench.keys` holds SELECT at frame 0 and then
   replays the walk from frame 120. A run of `base` in that mode shows 22 views in 720
   frames (`--read '[wLock]' --read 'u16[wViews]'`), so equiv is comparing real views,
   not a blank screen. Its limit: the locked mode doesn't exercise the pipelined path
   that round 2 added. The reference check, which runs unlocked, covers that.
2. **A timer-aligned start.** Round 3 changed how long startup takes, which moved the
   frame on which the LCD comes on. `equiv build/r3.gb --session base` then diverged at
   frame 11 (`difference kind=pixels x=0 y=0 a=3 b=0 count=14784`), with the two sides'
   frame 11 at t=729016 and t=708660. Nothing said "the LCD came on at a different
   time". Now the LCD comes on at the timer's first overflow after `Start.timed`
   (262,144 T). `gbcli measure Start.timed..Start.setUp --at boot` shows setup takes
   174,864 T, so it always finishes before then. `base3` is the round-3 code with this
   start. It became the equiv baseline for later rounds, and the reference check (7 of
   7) is what ties it to the baseline's pictures.

### The rounds

| # | Hot spot, from `--profile` over the walk | Change | cast mean T | period mean T (frames) | Check |
|---|---|---|---|---|---|
| 0 | (baseline `base`: ray1 plus the bench mode) | | 547,179 | 1,426,951 (20.32) | |
| 1 | `FillSpan calls=5654 exclusive_t=8423876` (52% of the cast) | Fill whole rows per tile column: sort its 4 rays by where their wall starts, giving 5 spans per half, mirrored below the horizon. No read-modify-write. | 328,460 (-40%) | 1,216,279 (17.32, -15%) | equiv vs base 720 frames; reference 7/7 |
| 2 | `MainLoop.waitCopy+$1 exclusive_t=28278108 idle_t=28256232` (67% of the walk halted, waiting for the copy) | A second WRAM buffer: cast the next view while the VBlank handler copies this one | 347,943 | 912,912 (13.00, -25%) | equiv vs base; reference 7/7 |
| 3 | `MainLoop.waitCopy exclusive_t=23921236 idle_t=23880124` (57% idle), `CopyChunks calls=600` | Copy only edge tiles. All-ceiling, all-floor and all-wall tiles point at 18 shared tiles through a second, double-buffered BG map ($9800 and $9C00, flipped with the tile sets). | 348,926 | 342,767 (4.88, -62%) | reference 7/7 (equiv impossible, see above; became `base3`) |
| 4 | `Mul16x8 calls=22201 exclusive_t=11109932` (500 T a call) | Quarter-square tables: `(v*m)>>8 = vh*m + (vl*m)>>8`, each 8x8 product as `Sq(a+b) - Sq(\|a-b\|)` | 325,228 | 320,329 (4.56, -6.5%) | equiv vs base3 |
| 5 | `CastRay calls=7915 exclusive_t=12078212` (1,526 T a ray) | DDA state in BC/DE/HL and HRAM, one loop per quadrant with `inc l`/`dec l` and `add`/`sub 16` | 295,731 | 295,048 (4.20, -7.9%) | equiv vs base3 |
| 6 | `RenderView exclusive_t=12871360` (DrawView inside it) and `FillRows exclusive_t=6375252` | Straight-line spans, and write only the rows of edge tiles (uniform tiles are never copied) | 232,406 (-21%) | 261,340 (3.72, -11%) | equiv vs base3; reference 7/7 |
| 7 | *A correctness fix, not a speedup:* round 6 copied 8 chunks per VBlank, and `break VBlank.vramDone --if '([rSTAT] & 3) != 1'` stopped in line 0's OAM scan | OAM DMA first, then start a chunk only while LY is 144-151 | 226,276 | 303,326 (4.32) | equiv vs base3; reference 7/7 |
| 8 | Copy-bound again: period 4.32 frames against a cast of 3.22 | Copy only the view's map rows that changed: the buffer's meta page still holds the map the back map shows | 234,543 | 292,085 (4.16, -3.7%) | equiv vs base3; reference 7/7 |

**I stopped after round 8, which gained under 5% (3.7%).** The 4-frame target was *not*
reached. Round 6 reached it (3.72) only by overrunning VBlank, and the honest final
figure is **4.16 frames a view on the DMG (14.4 views a second), 4.9x the baseline's
20.32**. On the GBC at double speed the same code takes 2.56 frames (23.4 views a
second; `tools/bench.py ... color`, which reads gbcli's `frame_pct` because a
double-speed T is half as long). Round 8's reference check is also where the stricter
`--current` check paid off. A view that needed no chunks at all (standing at a wall)
left `wCopyPending=1, wCopyLeft=0` forever, so the screen froze on a stale pose. That
pose still matched the reference, because the reference renders the pose on screen. The
check now also requires the view's pose to be the player's (`stale view ...`).

**The final assertion and budget** (on the main build):

```
gbcli measure MainLoop..MainLoop.waitCopy --session ray --at start --iterations 25 --frames 2000 --keys-file keys/walk.keys --max 280000
  measure ... n=25 speed=single max_t=280000
  t min=211492 max=267752 mean=234543.2              # exit 0: every view casts within 4 frames
gbcli run --session ray --at start --frames 600 --keys-file keys/walk.keys --frame-budget
  frame_budget frames=600 lag=390 lcd_off=0 active_min=4780 active_max=70240 active_mean=52720.1 busiest=175
```

The 390 "lag" frames are by design: the main loop never halts while it has a view to
cast, and halts only when it is waiting for the copy.

**VRAM safety, rechecked on the final DMG and GBC builds.** I broke on the OAM DMA's
end, the copy's end and the LCDC flip whenever STAT's mode isn't VBlank, and added the
two dropped-write watches. Then I ran the walk:

```
gbcli run build/raycaster.gb --session proof --frames 120 --checkpoint start
gbcli break VBlank.oamDone --if '([rSTAT] & 3) != 1' --session proof
gbcli break CopyChunks.copied --if '([rSTAT] & 3) != 1' --session proof
gbcli break ShowView.flipped --if '([rSTAT] & 3) != 1' --session proof
gbcli watch write '$97FF' --if '[$97FF] != new' --session proof
gbcli watch write '$87FF' --if '[$87FF] != new' --session proof
gbcli run --session proof --at start --frames 600 --keys-file keys/walk.keys
  stop budget_exhausted kind=frames ... frame=720           # nothing fired; 149 views
```

The same three breaks on `build/gbc/raycaster.gbc` (session `proofc`, `--model gbc`)
also ran 600 frames without a stop.

### Where gbcli helped

- **`gbcli measure` on named spans with `--keys-file`** produced every number in the
  table. Two spans times 25 iterations per build ran in a few seconds, and they were
  deterministic. I scripted it as `tools/bench.py`.
- **equiv caught a real bug in one call.** In round 8 I put new HRAM declarations in the
  middle of a WRAM variable list, so every variable after them landed in HRAM. equiv
  diverged at frame 9. A follow-up `--read 'u16[wViews]'` said `uninitialized read of
  wViews+$1 ($FF9B)`, which named the cause outright.
- **The profile's `idle_t` column** said "copy-bound" before I had thought to ask
  (rounds 2, 3 and 8).
- **Breakpoints follow their symbols across rebuilds,** with a warning (`breakpoint #1
  follows CopyChunks.copied into the ROM on disk: $00C3 -> $1796`). The proof session
  survived eight rebuilds.
- **Build errors are exact:** `` `JR` target must be between -128 and 127 bytes away, not
  146; use `JP` instead `` with file and line.

### Where it stumbled, costliest first

1. **equiv can't check a speed optimization of a renderer whose latency changes.** It
   compares the screen at every frame, and there is no way to compare only RAM, or only
   the frames where some condition holds. I built a whole frame-locked mode into the
   ROM to get a screen that doesn't depend on speed. Something like `equiv --sync-on
   'u16[wViews]'` (compare when an expression changes) or `--screen off` would have
   saved most of an hour.
2. **equiv needs the two builds' LCD to come on in the same frame,** and a startup that
   is a little longer or shorter breaks it with a pixel diff at frame 11. Nothing in the
   output says the frames are offset in time, although the two `t=` values differ. I
   added a timer-aligned start to the ROM. It would help if `equiv` warned "side B's
   frame N is 20,356 T earlier" or could start both sides at a symbol.
3. **Configure verbs load the machine.** After a rebuild, `gbcli delete 1` and `gbcli
   break ...` on a session whose head was on the old ROM failed with `rom_changed` and
   a 10 KB list of every moved RAM symbol. Deleting a breakpoint needn't load a state.
   I ran `--from boot` first.
4. **`ly` is the LY register, which reads 0 through most of line 153.** My first VBlank
   break, `--if 'ly > 151 || ly < 144'`, fired with `ly=0` at a point that `gbcli read
   '[rSTAT] & 3'` showed was still VBlank. Expressions have no PPU-mode variable (`show`
   prints `mode=`), so I wrote `([rSTAT] & 3) != 1` throughout. `gbcli help traps`
   suggests checking `ly` for VBlank. That's right for 144-152, but a footgun at 153.
5. **`--frame-budget` printed a row per lag frame again** (390). I redirected the output
   to a file and read the head.
6. **`measure` has no `--from boot`.** To measure startup I made a checkpoint at the
   handoff with `run --from boot --instructions 1 --checkpoint boot`.
7. **Stop rows name the header oddly:** `pc=$0101 ... symbol=OAMDMARoutine.end+$3C`.

## 2026-11-09 — ray3 (Vhu7N0): the maze

### What I built

- **The goal:** *Lantern*. Find the three keys (glowing white blocks: walk into one to
  take it), then walk into the exit door (light gray) before the lamp's oil runs out.
  The oil burns a pixel of a 128-pixel bar every 50 frames, so a full lamp lasts 6,400
  frames (107 s). Running out loses; the door with every key wins. START after either
  ending starts a new game.
- **The maze** is hand-made in `tools/gen_maze.py`. The script checks that the border is
  solid and that every key and the door can be reached, then writes `src/maze.asm`.
  Map cells are now 0 floor, 1 stone, 2 key and 3 door, each with its own face shades:
  stone 3/2, key 0, door 1. The renderer carries two plane masks per ray instead of one.
  The shared all-wall tiles still serve stone. A tile column that holds a key or the
  door copies its wall rows too.
- **The status line** (placeholder art, from `tools/gen_font.py` →
  `assets/tiles/font.pixels`, a 50-tile tileset): `KEYS n/3`, `OIL`, the oil bar, and
  a two-line message. The VBlank handler draws one changed part a frame into both BG
  maps, after OAM DMA and before the view copy. The copy then takes what is left of
  VBlank, as LY allows. The whole HUD is drawn once at startup, while the LCD is off.
- No sprites and no minimap. Keys and the door are wall cells, so the reference
  renderer covers them as it does every wall. `raymath.py` gained the cells' shades,
  and `check_poses.py` five poses set with `--poke` that face each key and the door
  (12 of 12 match on both builds).

### Criteria, and the commands that show them

**A bot reaches the win state, and a scripted run reaches the lose state:**

```
gbcli run build/raycaster.gb --session ray --frames 120 --checkpoint start
python3 tools/bot.py --gbcli gbcli --session ray
  frame 580: key at (3, 9), keys=1 state=0
  frame 2062: key at (14, 1), keys=2 state=0
  frame 3267: key at (1, 13), keys=3 state=0
  frame 4059: door at (15, 14), keys=3 state=1
  won at frame 4059; gbcli calls=108 wall_s=3.8 keys=keys/win.keys
gbcli run --session ray --at start --frames 4060 --keys-file keys/win.keys --expect '[wState] == 1' --read '[wKeys]' --read '[wOilPx]'
  read expr=[wKeys] value=3 / read expr=[wOilPx] value=45 / expect expr="[wState] == 1" value=1 ok=1
gbcli run --session ray --at start --until '[wState] == 2' --frames 7000 --keys-file keys/lose.keys
  stop predicate ... frame=6404 ... text="[wState] == 2"
```

The same `keys/win.keys` also wins on the GBC build (`--session color`, `--expect
'[wState] == 1'` ok). The game logic runs once a frame in VBlank, so double speed
doesn't change it.

**The bot's cost, the test of whether gbcli needs embedded scripting:** 108 gbcli calls
and 3.8 s of wall time for a 4,059-frame game (5.9 s on a second run, with other agents
loading the machine). About 35-55 ms a call. The bot plans in segments: BFS to the next
key, then one call per turn and one per straight stretch, each `run --frames N --keys
'UP@0 -UP@N' --read ...`. It then records every key change at its frame, and one
`--keys-file` run replays the game. For this game, **no embedded scripting is needed**.
A bot that steered every 4 frames would make about 1,000 calls (about 50 s), and that is
where an in-process script (or `gbcli serve`, which I didn't need) would start to pay.

**ray2's assertion still holds, and the frame budget over the winning run:**

```
gbcli measure MainLoop..MainLoop.waitCopy --session ray --at start --iterations 25 --frames 2000 --keys-file keys/walk.keys --max 280000
  t min=220372 max=264140 mean=244716.5                       # exit 0
gbcli run --session ray --at start --frames 4060 --keys-file keys/win.keys --frame-budget --read 'u16[wViews]'
  frame_budget frames=4060 lag=2955 lcd_off=0 active_min=4700 active_max=70244 active_mean=57030.3 busiest=125
  read expr=u16[wViews] value=945                             # 30 at start: 915 views in 4,060 frames, 4.4 frames a view
```

The cast's mean rose from 234,543 T to 244,716 T. The HUD and game tick in VBlank cost
the main loop about 4%.

**VRAM safety, with the HUD.** The proof session gained `gbcli break VBlank.hudDone
--if '([rSTAT] & 3) != 1'`. With all four breaks and both watches armed, the whole
winning game ran without a stop: `gbcli run --session proof --at start --frames 4060
--keys-file keys/win.keys --expect '[wState] == 1'` gave `ok=1`.

**Both builds succeed:** `errors=0 warnings=0` for each.

### Where gbcli helped

- **`--poke` places the player anywhere** (`--poke wPosX=128,13 --poke wPosY=128,1
  --poke wAngle=0,0`). It gave the reference check its key and door poses without
  walking there.
- **`--until` plus `--expect`** turned "the game can be lost" and "the recorded game
  wins" into one run each, each with a yes or no.
- **The bot was cheap to write** because runs are deterministic, held keys are
  per-run, and `--read` rows are easy to parse. It won on its first run.
- **`gbcli pixels FILE`** checked the generated font (`tiles=50`) before a build used it.

### Where it stumbled, costliest first

1. **A mode-3 VRAM write failed silently, and I found it by eye.** The first HUD drew
   every part in one VBlank. The message row was cut to `THE KE` because the later
   writes landed in mode 3 and were dropped. `gbcli run` warns when the LCD is turned
   off outside VBlank, but says nothing about dropped VRAM writes. A per-run count
   (`warning: 23 VRAM writes ignored in mode 3, first pc=$2A31`) would have named the
   bug at once. The trap's per-byte watch can't, because you have to guess the byte
   first.
2. **`--frame-budget` printed 2,955 lag rows** for the winning run. I redirected it to a
   file again.
3. *(RGBDS, not gbcli:)* HRAM labels named `hL` and `hH` read as the register `hl`
   (`syntax error, unexpected hl`). gbcli's diagnostic rows pointed at every line,
   which made the cause easy to see.

## 2026-11-09 — ray4 (ll9Zq2): art, music and sound

### What I built

- **The title screen.** `tools/gen_title.py` draws `assets/bg/title.pixels`
  (160x144, 89 unique tiles): the name at twice the HUD font's size, a lantern, a
  corridor running to a far door, `PRESS START` and `FIND 3 KEYS`. Flow: title → START
  → maze → ending → START → title. The checkpoint every tool uses is now `gbcli run
  build/raycaster.gb --session ray --frames 120 --keys 'START@10 -START@12'
  --checkpoint start`.
- **Status line and font:** `tools/gen_font.py` → `assets/tiles/font.pixels`, from
  ray3. Both images are assigned one palette in `assets/palettes.txt`, the DMG's own
  grays, so one tile set serves both builds (`shades=declared`).
- **Walls:** flat shades by cell and side, from `raymath.CELL_SHADES` (stone 3/2, key
  white, door light gray). They are unchanged since ray3, so the reference still applies
  as it is.
- **GBC colors:** BG palette 0 is lamplight (cream, tan, brown, soot), one color per
  DMG shade, so the GBC picture is the DMG's recolored. The reference reads the shades
  from the on-screen swatch, so it needed no change for the palette.
- **Music** (gbcli song text, hUGEDriver, `timer off`, ticked at the end of the VBlank
  handler):
  - `assets/music/title.song`: D minor, lead, bass and ticks.
  - `assets/music/maze.song`: A minor, slower, a pad and a bell over a wave bass, with
    drips.

  Both leave **channel 1 silent**. `MusicPlay` also mutes it in the driver
  (`hUGE_mute_channel`).
- **Sound effects** (`src/sfx.asm`, channel 1, a table of note steps): a key chime
  (C6-E6-G6-C7), a locked-door knock, a win fanfare, a falling lose sweep and a START
  blip.

### Criteria, and the commands that show them

**Every image in assets/ is pixel text from a script that regenerates it
byte-identical:**

```
python3 tools/gen_font.py --check      # ok: assets/tiles/font.pixels, ok: include/charmap.inc
python3 tools/gen_title.py --check     # ok: assets/bg/title.pixels
gbcli pixels assets/bg/title.pixels --preview build/title.png   # tiles=89
```

**The music and effects in gbcli's audio views:**

```
gbcli run build/raycaster.gb --session music --frames 900 --audio-summary --piano build/title-roll.png
  audio_writer pc=$19F4 symbol=play_ch2_note+$A writes=20 registers=NR23     # channels 2, 3 and 4 from the driver
gbcli run --session ray --at start --frames 900 --piano build/maze-roll.png --audio-summary --audio-channel 2
  audio_register reg=NR21 ... writes=17
gbcli run --session ray --at start --frames 4060 --keys-file keys/win.keys --expect '[wState] == 1' --audio-summary --audio-channel 1
  audio_writer pc=$017E symbol=SfxTick+$18 writes=13 registers=NR10          # channel 1: only SfxTick writes it
gbcli run --session ray --at start --frames 600 --keys-file keys/win.keys --audio-log --audio-channel 1
  apu ... frame=696 ... text="NR13 $83: square 1 period 1923 = C6 +3.4c (1048.58 Hz)"
  apu ... frame=700 ... E6 / frame=704 ... G6 / frame=708 ... C7 / frame=718 "NR12 $00: square 1 DAC off"
```

The last run shows the first key's chime, decoded note by note, on the frames after
the pickup.

**The reference still matches:** 12 of 12 poses on the DMG (`check_poses.py --session
ray`) and on the GBC with the new palette (`--session color`).

**ray2's assertion holds with the music ticking:** `gbcli measure
MainLoop..MainLoop.waitCopy --session ray --at start --iterations 25 --frames 2000
--keys-file keys/walk.keys --max 280000` gives max 275,092 and mean 249,718 (exit 0).
The music costs the cast about 2% on the mean, and the margin under the bound is now
under 2%.

**Both builds succeed:** `errors=0 warnings=0`. The bot reran unchanged against the
new start (108 calls, 4.3 s), and `keys/win.keys` still wins on both builds.

### Where gbcli helped

- **`--audio-summary --audio-channel 1`** proved the channel split in one row: every
  write to channel 1 during a whole game comes from `SfxTick`. No listening needed.
- **`--audio-log`** decodes each write into a note with its cents error (`C6 +3.4c`), so
  the effect's pitches could be checked against their intent by reading.
- **`gbcli song FILE`** rejected `B-2` with the valid range and the spelling rule
  before any build ran. **`gbcli pixels FILE --preview`** showed the title at 4x with
  a grid before it reached the ROM.

### Where it stumbled

1. **Small:** the title run's summary shows channel 1 written once, by hUGEDriver's
   `note_cut` (NR12 and NR14), during `hUGE_init`, before `MusicPlay` mutes the
   channel. That's harmless, but `gbcli help music`'s recipe (mute after init) can't
   prevent it, and the help doesn't say so.
2. Nothing else cost time. The music and image pipeline was the smoothest part of the
   project.

## 2026-11-09 — ray5 (TGAudV): polish and verification

- **The loop:** title → START → maze → ending → START → title, then a fresh game.
  `--expect '[wMode] == 0'` holds after START at an ending, and after START again
  `--expect '[wMode] == 1' --expect '[wKeys] == 0'` both hold. The reference matches
  in the second game too (`reference.py --current`).
- **`examples/raycaster/README.md`** covers the game, how to play, what's notable (with
  the optimization table) and every claim's command. **`tools/verify.py`** reruns all of
  them in fresh sessions. Its final run: `ok: 0 failed, 32 s`, 27 steps.
- **Both builds are clean,** `errors=0 warnings=0`, so there are no warnings to explain.
- **One stumble:** `gbcli build --dir PROJECT` is refused (`bad_request: --dir only
  applies when --restore reloads the build`), although `gbcli build --help` lists
  `--dir` among its output flags. `gbcli build PROJECT` (the positional DIR) works.
  verify.py runs `build` with the project as the working directory instead.

## Summary

### The five costliest stumbles

1. **`gbcli equiv` can't check a speed optimization of a renderer whose latency
   varies** (ray2). It compares the screen at every frame, so a faster renderer
   "diverges" on its first early view. I built a frame-locked bench mode into the ROM
   (SELECT at power-on) to get a screen that doesn't depend on speed. Ask: compare only
   on frames where an expression changes (`--sync-on 'u16[wViews]'`), or with the
   screen off.
2. **equiv needs both builds' LCD to come on in the same frame** (ray2). A startup a few
   thousand cycles longer gave a pixel diff at frame 11, and nothing said the frames
   were offset in time. I added a timer-aligned start and a new baseline. Ask: warn when
   the two sides' frame N starts at different `t`, or start both at a symbol.
3. **Dropped VRAM writes are silent** (ray3; and in ray2, an OAM DMA that ran past
   VBlank, which I caught only because I had written a break for it). `run` warns when
   the LCD is turned off outside VBlank, but not when VRAM or OAM writes are ignored in
   modes 2 and 3. The HUD bug showed as a cut-off message that I spotted by eye. Ask: a
   per-run count with the first pc.
4. **`ly` reads 0 through most of line 153, and expressions have no PPU-mode
   variable** (ray2). A VBlank break on `ly < 144` fired while still in VBlank. I
   switched every proof to `([rSTAT] & 3) != 1`. `gbcli help traps` points at `ly`.
5. **Output volume and config friction** (every leaf):
   - `--frame-budget` prints one row per lag frame (217, 390 and 2,955 rows), for a
     renderer that is meant to take several frames per view.
   - `break` and `delete` on a session whose head predates a rebuild fail with a 10 KB
     `rom_changed` dump.
   - Armed breakpoints and watches can't be listed, and `break` doesn't echo `--if`,
     although "nothing fired" is the whole proof.
   - Deleted breakpoint ids are reused.

### The three things that helped most

1. **`gbcli measure` on named spans, with `--keys-file` and `--max`.** Every number in
   the eight-round table came from two deterministic measures per build, scripted as
   `tools/bench.py`. The final bound is one assertion that exits 1.
2. **`shot --scale 1`, `read` and `view memory --out`, plus `--poke`.** Together they
   made a pixel-exact host reference renderer possible: 12 poses on two machines, in
   one script. That check caught a carry bug in ray1 and a frozen-view bug in ray2
   (and equiv caught a section-layout bug), each within minutes.
3. **Deterministic runs with keys files, `--until` and `--expect`.** The closed-loop
   bot won on its first try in 108 calls and about 4 s. It writes its keys out, and one
   run replays the win on either machine. "It can be lost" and "START returns to the
   title" are one yes-or-no run each. For this game, gbcli needs no embedded scripting.
