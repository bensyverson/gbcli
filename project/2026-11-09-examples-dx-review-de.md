# Findings: what the roguelike and the rhythm game taught us about gbcli

2026-11-09, sixteenth session, leaf `f57YRV` (review-de). **Ben ruled on 2026-11-09, accepting every recommendation; the decisions are at the end.**

Two Opus agents each built a vertical slice using gbcli as users: Lanternfall (`examples/rogue/`, D) and Lantern Drum (`examples/rhythm/`, E). Each finished all four of its leaves in one session, with gbcli pinned at `c38cd29`. Sources: [2026-11-09-dx-rogue.md](2026-11-09-dx-rogue.md) and [2026-11-09-dx-rhythm.md](2026-11-09-dx-rhythm.md), written blind to each other. The reviewer was given the docs and the code, not the agents' reports. The integrator rechecked two load-bearing claims: `s8`/`s16` in `crates/engine/src/cond/names.rs:43-44`, and the un-park condition's wording in backlog.md.

**What changed on main since the pin.** Two commits touch `crates/`. `08752b0` lets `gbcli pixels` read big PNGs, fixes `write_all` and rewords some help. `8c920d6` lists the `pixels` topic in the primer. Neither touches anything below. So every item is **still open** unless it is marked decided or parked. Each status was checked with a debug build of main, using scratchpad copies of both games. Each is marked *observed* (run or read) or *inferred*.

**Neither agent was blocked.** Both say gbcli never got in the way of the game. The rhythm game's risky part (no drift against the driver) worked on the first build. The first wave's fixes paid off: `lcd_off` in the frame budget, the one-row `keys_unapplied`, the audio channel filter and summary, 8x8 objects, pixel text and `view palettes` were each used without complaint. Both agents rank them among the helps.

## Hit by both agents

1. **Per-frame rows in `--json` have no frame number** (about 25 min together, plus a script in each game).
   - **What happened:**
     - Rogue: `frame_budget.frames[]` is an array of `{active_t, idle_t, lag, lcd_off}` counted from the run's start. "No lag on floor 5" needed `tools/budget.py` to slice the array.
     - Rhythm: audio-log rows carry `elapsed` (2^23 Hz) but no frame. The agent worked out 140,448 ticks a frame from two onsets. Skipping the title loop's notes then took a multiplication, not a filter on a field.
   - **Status:** still open (observed). `busiest` and the lag rows' `index` are run-relative indexes.
   - **Recommendation:** one leaf, "frame numbers on per-frame and per-write rows":
     - a `frame` field on each `frame_budget.frames[]` entry, counted from the run's `--since` base like a stop's `frame=`;
     - a `frame` field (and `ly`) on each `audio.log[]` entry and its text row;
     - the lag row printing `frame=` beside `index=`.
2. **A key file can't resume from a checkpoint** (rogue about 10 min; rhythm replayed 4,700-frame runs from boot every time).
   - **What happened:** key offsets count from the run's start. So a key file recorded from boot can't be replayed from a mid-game checkpoint (rogue: `--at` floor 5 gave `keys_unapplied`; rhythm asked for `--keys-offset -63`). Separately, the rogue lost about 25 minutes to key files that broke whenever startup time changed.
   - **Status:** **parked** in [backlog.md](backlog.md) ▸ "Small run and view refinements from the example games" (keys-file offsets from a time base). Its un-park condition, "a later example game or LoadHigh hits one of them again", **now holds**, twice over.
   - **Recommendation:** un-park as one leaf: `run --keys-since BASE`, where BASE is `boot`, `power-on` or a checkpoint name, as with `--since`.
3. **When a frame "ends" and when a key lands are undocumented** (rogue about 10 min, rhythm one calibration run).
   - **What happened:**
     - Rogue: `run --frames N` stops at `pc=$0040 symbol=VBlankVector ly=144`, before the handler runs, so a view check saw last frame's scroll registers.
     - Rhythm: couldn't predict whether `A@66` is seen by frame 66's joypad read. The answer is yes.
   - **Status:** still open (observed). No help topic says either.
   - **Recommendation:** a "frames and keys" paragraph in `gbcli help time`, pointed to from `run --help`'s `--frames` and `--keys` lines. Part of the doc leaf.
4. **Writer PCs have no symbols** (rogue about 3 min; the rhythm agent did the same lookup).
   - **What happened:** `--audio-summary` prints `audio_writer pc=$00C5` while stop and profile rows name symbols. That lookup is what exposed `PlaySfx` linked at `$0000` (E-4).
   - **Status:** still open (observed). `protocol::audio::WriterWrites` is `{pc, writes, registers}`.
   - **Recommendation:** `symbol=` (`Label+$n`, as stop rows spell it) on `audio_writer` rows and `WriterWrites`, and on `apu` log rows if it is cheap.

Also hit by both, but **not gbcli**: the worktree guard refused `PATH=…:$PATH cmd`, a `$G`-style variable and `;` compound lines (rogue about 10 min). This belongs under "What the brief got wrong".

## Lanternfall (D) alone

1. **No block memory read** (about 20 min, then "every map dump since"). The rogue's #1.
   - **What happened:** `gbcli read 'wMap..wMap+16'` gives `unknown_symbol`. `view` covers only VRAM, OAM and palettes. The workaround passes 3,075 expressions to one `gbcli read --json`. The D plan's leaf promised "`--read` or a memory view", which doesn't exist.
   - **Status:** still open (observed). There's no backlog entry.
   - **Recommendation:** `gbcli view memory ADDR LEN [--out FILE]`: hexdump rows, raw bytes with `--out`, and `read`'s rules (no side effects; never-written bytes are an error unless `--allow-uninit`). The expression grammar stays scalar. **Needs Ben** (question 1).
2. **Nothing says which build a session runs** (about 20 min). The rogue built `--gbc` over `build/rogue.gb`, then ran "DMG" sessions on the GBC build. It noticed only because its seeds differed.
   - **Status:** still open (observed). `show` and `sessions` give no model and no header CGB flag.
   - **Recommendation:** `show` and `sessions` print `model=` and the header's `cgb=dmg|dual|only` ($0143).
3. **`--profile` charges a tail-called routine's halts to its caller** (about 5 min). `MaybeYield` ends in `jp WaitFrame`, so its `exclusive_t` included HALT time.
   - **Recommendation:** an `idle_t` (HALT cycles) column per routine.
4. **One bad `palettes.txt` line becomes six errors** (about 5 min). The correct `palette_file` error is followed by five `no palette …: the palettes define none` errors (observed).
   - **Recommendation:** when `palettes.txt` fails to parse, report its errors only, or parse past the bad line.
5. **`images … errors=1` above `build … errors=0`.** **Decided** by the A/B review ("the help says so", and it does). No change.
6. **The bot recipe's "it records each key change" reads as gbcli recording** (about 3 min). Reword it to "your script records…", as part of the doc leaf.
7. **Errors print twice under `--json`.** **Decided** by design ([2026-11-06-design.md](2026-11-06-design.md), correction of Uv0dE6). No change.
8. **The piano roll draws a muted channel's retrigger as a stuck note.** **Not reproduced on main** (observed: square 2's lane is empty over 200 frames of the final ROM). It was likely an intermediate build. No change.

## Lantern Drum (E) alone

1. **Song text repeats every shared pattern in full** (about 15 min, plus the composer's upkeep in every later session). The rhythm's #1.
   - **What happened:** a block header names all four patterns, and every copy of a shared pattern must read the same. The agent wrote `tools/compose.py` (245 lines), which turned the song text into a build product.
   - **Status:** this is the design as built ([2026-11-06-design.md](2026-11-06-design.md), update U2rLOt).
   - **Recommendation:** leniency on input only. A block may name fewer than four patterns (`pattern P5`, or `pattern P5 = = =` for "as in the order line"), so each pattern is written once. `--text` stays canonical. **Needs Ben** (question 2).
2. **No run-level assertion** (about 10 min). A new fail state broke the sync proof silently. `--until` exits 3 when it never holds, but it stops at the first true frame.
   - **Recommendation:** `run --expect EXPR` (repeatable), evaluated at the stop. It prints `expect expr= value= ok=` and exits 1 if any is false.
3. **The `--read` flag line reads `--read EXPR…`, but the flag takes one expression.** The USAGE line is right. Worse on main: a stray second expression is taken as the ROM path, with an error far from the cause (observed).
   - **Recommendation:** fix the flag line, and when a positional ROM argument looks like an expression, refuse it with "repeat `--read` for each".
4. **The starter leaves $0000-$003F unclaimed** (about 10 min). rgblink placed `PlaySfx` at `$0000`, in the restart vectors. A fresh `gbcli new` claims only the VBlank and timer vectors and `$0100` (observed, and rechecked by the integrator).
   - **Recommendation:** the starter claims $0000-$003F, filled with `rst $38`.
5. **The `help images` example `bg/town.png = forest  # pixel text: bg/town.pixels` reads as "assign by the .png name"** (one build). Show both forms on separate lines and make the `palette_file` remedy match.
6. **"No signed reads" is wrong: signed reads exist but aren't documented.** `s8[…]` and `s16[…]` work (`crates/engine/src/cond/names.rs:43-44`, present at the pin). `help expressions` lists only `[ADDR]` and `u16[ADDR]`.
   - **Recommendation:** document `u8/s8/u16/s16`, and give `i8`/`i16` a remedy that names `s8`/`s16`.
7. **`help images` is long for the common question** (235 lines; "which kind of art" is answered at line 20). Add a three-line lead.
8. **Exported constants aren't constant to rgbasm** (about 5 min; plain RGBDS). Add one line to `help rgbds`: constants that other files assemble against belong in `include/`.

## Helps (what worked)

Both agents, independently:
- **Deterministic runs with `--keys`, `--read` and `--frame-budget` in one call**, each agent's #1. The rogue's closed-loop bot plays a 632-turn game in 6.5 s, and its key files replay frame for frame on the DMG and the GBC. The rhythm's open-loop key files scored 106/106, then 165/165, on the first try.
- **`--frame-budget` naming the lag frame** found one real bug in each game in about five minutes: the rogue's yield threshold, and the rhythm's `LcdOff` spinning on LY.
- **`--audio-summary` writer PCs** proved the channel-2 split (music vs effects) in one command.
- **The asset pipeline as text** (`.pixels`, `palettes.txt`, song text), with located errors and remedies. Both games' assets regenerate byte-identical.
- **`build --gbc --out`** kept both ROMs side by side.

One agent each:
- **Rhythm:**
  - `gbcli help music` settled the timing design before any code was written;
  - the audio log's exact timestamps made "no drift" a number;
  - `--until` treating an unwritten byte as "not yet".
- **Rogue:**
  - `--profile` sized the generator;
  - `rom_changed` lists moved symbols.

## Python in both games, and the embedded-scripting backlog entry

The entry is [backlog.md](backlog.md) ▸ "Embedded scripting for input bots". Its un-park condition: "the D (roguelike) or E (rhythm) DX doc shows an input bot, or another per-frame script, costing real time or calls again."

| File | Lines | Kind |
|---|---|---|
| rogue/tools/font.py | 89 | art |
| rogue/tools/title.py | 109 | art |
| rogue/tools/budget.py | 42 | output post-processing (goes away with item 1) |
| rogue/tools/maptool.py | 377 | mixed: **closed-loop input bot**, connectivity checks, and a block-read workaround (goes away with D-1) |
| rhythm/tools/art.py | 163 | art |
| rhythm/tools/font.py | 103 | art |
| rhythm/tools/compose.py | 245 | song generator, a workaround for E-1 |
| rhythm/tools/chart.py | 121 | build-time tables |
| rhythm/tools/songtext.py | 104 | build-time (song-text reader for chart.py) |
| rhythm/tools/keys.py | 54 | open-loop input generator |
| rhythm/tools/sync.py | 135 | verification (simpler with a `frame` field) |
| rhythm/tools/windows.py | 46 | open-loop test sweep |

**Exactly one closed-loop bot**, the rogue's: about 1,100 `gbcli run` calls (632 for the DMG win), at 6.5 s of wall time per win. The rogue's agent ranks it the best thing in gbcli for a turn-based game, not a stumble. The rhythm game has no bot.

**Does the condition hold?** By its wording ("or calls again"), yes. By its intent (a bot costing real time), no. **Recommendation:** keep it parked, and sharpen the condition: "an agent ranks driving a bot among its costliest stumbles, or a real-time game's bot needs a decision every frame or two, so per-call overhead dominates". C, a real-time game, is the next test (question 5).

## What the brief got wrong

For C's brief:
- **The D plan promised a memory view that doesn't exist.** Either D-1 lands first, or the brief says "`gbcli read` with one expression a byte, in one call".
- **The leaves pointed at `scripts/examples/build-roms.sh`,** which the user contract forbids.
- **"Put the binary's directory first on PATH" fails** under the worktree guard. Brief the absolute path only, one plain call per command.
- **`gbcli gaps` printed `none` every session**; stumbles belong in the DX doc.
- **The halfway context rule never applied**: each chain fit in about 35% of a 1M window.
- **New for C:**
  - record key files late, after startup stops changing (key files count from boot, item 2);
  - report the bot's call count and wall time, so the scripting question gets a real answer;
  - state the frame-boundary fact (item 3) until the help says it.

## Questions for Ben

1. **Block memory read: what shape?** *Recommend* `gbcli view memory ADDR LEN [--out FILE]` (hexdump rows, raw bytes to a file, `read`'s uninit rules), not a range form in the expression grammar.
2. **Song text: accept a block naming fewer than four patterns?** Lenient on input only; `--text` stays canonical. *Recommend yes.*
3. **`run --expect EXPR`** (exit 1 when false at the stop)? *Recommend yes.*
4. **Un-park keys offsets from a time base (`run --keys-since BASE`)**, together with item 1's frame fields? *Recommend yes*: its un-park condition holds.
5. **Embedded scripting:** keep it parked, sharpen the un-park condition as above, and use C as the test? *Recommend yes.*
6. **The small items, as three leaves?** *Recommend yes:*
   - **docs:** frames and keys in `help time` (item 3), the recipe wording (D-6), `s8`/`s16` and the `i8` remedy (E-6), the `--read` flag line and its positional refusal (E-3), the `help images` examples and lead (E-5, E-7), the `include/` line in `help rgbds` (E-8);
   - **observations:** `symbol=` on writer rows (item 4), `idle_t` per routine (D-3), `model=`/`cgb=` in `show` and `sessions` (D-2);
   - **build:** the starter claims $0000-$003F (E-4), and `palettes.txt` errors stop the cascade (D-4).

## Ben's decisions (2026-11-09)

Ben accepted every recommendation:

1. **Block memory read:** `gbcli view memory ADDR LEN [--out FILE]`.
2. **Song text:** a block may name fewer than four patterns on input; `--text` stays canonical.
3. **`run --expect EXPR`:** yes.
4. **`run --keys-since BASE` with `frame` fields on per-frame and per-write rows:** yes, one leaf. Un-parked from backlog.md.
5. **Embedded scripting:** stays parked, with the sharper un-park condition in backlog.md. C is the test.
6. **The small items:** three leaves (docs, observations, build).

These are filed in `job` under "gbcli fixes from the D/E DX review". C (the raycaster) waits for them ([2026-11-09-raycaster-plan.md](2026-11-09-raycaster-plan.md), question 2).
