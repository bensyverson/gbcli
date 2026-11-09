# Findings: what the raycaster taught us about gbcli

2026-11-09, seventeenth session (review-c, leaf `rsUZcp`). **Draft, waiting for Ben's rulings** (questions at the end). Drafted by a research agent (dx-review-c) that checked each stumble at main with a debug build of `daf6d93`; saved by the main thread.

One Opus agent built Lantern (`examples/raycaster/`, C), a first-person raycaster, using gbcli as a user. It finished all five leaves (ray1 to ray5) in one session, with gbcli pinned at `b30becb`. Source: [2026-11-09-dx-raycaster.md](2026-11-09-dx-raycaster.md), plus the game's README and `tools/`. This is the last of the five example games; the last section looks across all five DX docs.

**What changed on main since the pin.** Bare RAM symbols are refused as truth values in conditions (`655a334`, `d6c136c`, `c085c52`); pixel fonts (`cff86f7`, `6feaf3e`); the starter's text renderer (`daf6d93`). None touches equiv, breakpoints and watches, the frame budget, `measure`, `shot` or `build`'s flags (`git diff --name-only b30becb..HEAD -- crates`), so every item below is **still open** unless marked otherwise.

**Ratings.** *Agent cost* comes from the DX doc: high is about half an hour or more, or a stumble the agent ranked in its top five; medium is 10 to 30 minutes; low is a few minutes or one detour. *Build cost* is a rough guess: S fits inside a shared leaf, M is a leaf of its own, L is more than one leaf. Every recommendation respects the standing rulings ([2026-11-09-web-target.md](2026-11-09-web-target.md)): nothing a web build or a Rust-library caller couldn't use, and every verb is a protocol op first.

**The agent was not blocked.** It reached a 4.9x speedup (4.16 frames a view on the DMG), a pixel-exact reference check at 12 poses on both machines, a bot that wins, and VRAM-safety proofs on both builds. Its stumbles cluster in two places: comparing builds whose timing differs (`equiv`), and proving that no write landed when the hardware ignores it (VRAM in mode 3).

## Lantern (C) alone

1. **equiv can't check a speed-up of a renderer whose latency changes** (most of an hour; the agent's #1). Agent cost: high.
   - **What happened:** equiv compares the screen at the end of every frame. A faster renderer shows each view sooner, so a correct change "diverged at frame 27 with 6,308 pixels". The agent built a frame-locked bench mode into the ROM (SELECT at power-on; a view only on multiples of 32 frames) for rounds 1-8. Round 3 couldn't use equiv at all.
   - **Status:** still open (observed). `equiv --help` offers `--ram`, `--audio` and `--patch`, but no way to stop comparing the screen; `EquivArgs` has no screen field (`crates/protocol/src/equiv.rs:18-39`).
   - **A limit on any fix (inferred):** a renderer that takes a new pose each view samples input at different frames when it gets faster, so a faster build shows *different* views, not only earlier ones. `--sync-on 'u16[wViews]'` would still diverge while keys are held. The ROM-side frame lock is the sound check, with the reference renderer covering what the lock doesn't exercise.
   - **Options:** (a) a `gbcli help optimize` paragraph on "a change that alters timing" (lock output in a test mode, compare timing-free state with `--ram`, or use a reference check), S; (b) `equiv --screen off`, so a `--ram` compare can stand alone, S; (c) `--sync-on EXPR`, M, limited as above.
   - **Recommendation:** (a) and (b) now; park (c) in backlog.md until a game whose views don't depend on input timing needs it (question 1).
2. **equiv needs both builds' LCD to come on in the same frame** (part of the same hour; ranked #2). Agent cost: high.
   - **What happened:** round 3 lengthened startup; equiv reported `difference kind=pixels … count=14784` at frame 11 with the sides at t=729016 and t=708660, and nothing said the frames were offset in time. The agent added a timer-aligned start and a new baseline.
   - **Status:** still open (observed). Each side's stop row carries its own `t=`, but nothing compares them (`crates/cli/src/render/equiv.rs:13-27`). This is not the parked `equiv --after N` (RAM laid out differently); that entry's un-park condition doesn't hold.
   - **Options:** (a) when the first differing frame starts at a different `t` on the two sides, a row saying so ("side B's frame 11 starts 20,356 T earlier: startup timing differs"), S; (b) start both sides at a symbol, M, folding into question 1's (c).
   - **Recommendation:** (a).
3. **A VRAM write dropped in mode 3 is silent** (found by eye; ranked #3). Agent cost: high.
   - **What happened:** the first HUD drew everything in one VBlank and the message came out as `THE KE`. In ray2 an OAM DMA that ran past VBlank was caught only because a breakpoint already watched it. `run` warns when the LCD is turned off outside VBlank, but says nothing about ignored writes.
   - **Status:** still open (observed). The only per-run hardware warning is the LCD-off one (`crates/engine/src/debug/lcd.rs`). SameBoy's `write_vram` syncs the display and returns early when `vram_write_blocked` is set (`crates/sameboy-sys/SameBoy/Core/memory.c:1017-1024`), so a counter there would be exact and immune to the batched-mode-3 limit STAT conditions hit. OAM has the same shape.
   - **Recommendation:** a per-run warning in the LCD-off one's form (`warning: 23 VRAM writes ignored in mode 3, first at $2A31 (HudDraw+$12), line 2 of frame 140`), the same for OAM in modes 2 and 3, every field in `--json`. Engine-side, so it works the same in a web build. M (a small vendored patch or a write-callback check, plus the trap's help). Question 2.
4. **`ly` reads 0 through most of line 153, and expressions have no PPU-mode name** (one false stop, then every proof rewritten; ranked #4). Agent cost: medium.
   - **What happened:** `break CopyChunks.copied --if 'ly > 151 || ly < 144'` fired with `ly=0` while STAT said VBlank; the agent switched to `([rSTAT] & 3) != 1`.
   - **Status:** still open (observed). `ly` is the LY register (`crates/engine/src/cond/eval.rs:257`); the keyword table has no `mode` (`crates/engine/src/cond/names.rs:8-36`). At main, `run --until '([rSTAT] & 3) == 1 && ly == 0'` on Lantern stopped in VBlank with `ly` = 0, and the mode changed 452 T later. `help expressions` lists `ly` with no caveat (`crates/cli/src/help/topics.rs:131`); `help traps` and `help hardware` say "144 to 153 is VBlank", true of lines but not of what LY reads.
   - **Options:** (a) a `mode` keyword with the value `show`'s `mode=` prints (`condition_error` where unknown, as `[rSTAT]`), S; (b) help lines on `ly` at line 153, S.
   - **Recommendation:** both; (a) adds to the expression grammar (question 6).
5. **Armed breakpoints and watches can't be listed; `break` doesn't echo `--if`; a deleted id is reused** (ray1-ray3; within #5). Agent cost: medium. "Nothing fired" was the whole VRAM proof, so whether the traps were armed *was* the claim.
   - **Status:** still open (observed). `show` and `show --json` print no breakpoints. `break … --if …` prints `breakpoint id=1 addr=$29F3 symbol=CopyChunks.copied`; `BreakpointMade` and `WatchMade` carry no condition (`crates/protocol/src/response.rs:364-374`), though `BreakpointSpec` stores it. Ids are `max + 1` (`crates/engine/src/debug/mod.rs:271-277`), so deleting the highest id reuses it.
   - **Recommendation:** `show` prints a row per breakpoint and watch (id, place, condition, armed), `--json` carries the session's debugger config; a `condition` field on the `break` and `watch` results; ids never reused (a counter in the session file, with a serde default so old sessions load). S overall (question 3).
6. **Configure verbs load the machine** (each rebuild; part of #5). Agent cost: medium.
   - **What happened:** after a rebuild, `delete 1` and `break …` failed with `rom_changed` and a 10 KB list of moved RAM symbols; the agent ran `--from boot` first.
   - **Status:** still open (observed). `breakpoint`, `watch` and `delete` call `machine_for(…, Need::Head, …)` (`crates/server/src/handle/config.rs:17, 48, 89`); the handler already has `Need::Replace` for `restore` and `undo` (`crates/server/src/handle/mod.rs:91-103`).
   - **Recommendation:** configure verbs resolve against the ROM on disk without loading the head; `rom_changed` text names the first few moved symbols and a count, with the full list in `--json`. S-M (question 4).
7. **`--frame-budget` prints a row per lag frame** (217, 390 and 2,955 rows; every leaf; "about 15 KB in my context"). Agent cost: medium.
   - **Status:** still open (observed). `crates/cli/src/render/profile.rs:86-97` writes a `frame_cost` row per lag frame; at main, the 600-frame walk printed 545 rows (33 KB), which fall into 27 runs of consecutive frames.
   - **Recommendation:** one row per run of consecutive lag frames (`frame_lag from=120 to=146 count=27 active_max=…`), capped; `--json` keeps every frame. The third time this class has come up (A/B's `key_unapplied`, D/E's per-frame rows). Question 5.
8. **`measure FROM..TO` whose TO isn't reached on every pass gives a misleading max** (565,300 T for a 2,896 T copy). Agent cost: low. Works as defined; **recommendation:** a help line (read min, or pick a TO every pass reaches).
9. **`shot` writes a 2-bit palette PNG, and the help doesn't say so** (one decoder fix; `tools/pngio.py`). Agent cost: low. **Recommendation:** say it in `shot --help` now; backlog the screen's shade indices as data in `shot --json` (useful to library and web callers) until a second agent decodes `shot` PNGs.
10. **`measure` has no `--from boot`** (one workaround checkpoint). Agent cost: low. At main, `measure --from boot` gives `bad_request … did you mean --rom?`, a wrong suggestion. **Recommendation:** `measure --from boot|power-on`, as `run` and `equiv` have it, and a correct remedy. S.
11. **Stop rows name the cartridge header after the nearest label** (`pc=$0101 … symbol=OAMDMARoutine.end+$9`; the starter gives `+$59`). Agent cost: low. The profiler already names vectors and skips end markers (`crates/engine/src/profile/names.rs:9-46`). **Recommendation:** stop and profile rows share one namer that knows the vectors and `Header` (`$0100-$014F`). S.
12. **`gbcli build --dir PROJECT` is refused although `build --help` lists `--dir`** (`--dir only applies when --restore reloads the build`, a deliberate refusal at `crates/cli/src/rgbds.rs:36-44`). Agent cost: low. **Recommendation:** `build --dir PATH` means what it means elsewhere; one meaning per flag across verbs. S.
13. **`hUGE_init` writes the effect channel once before `MusicPlay` can mute it** (harmless). Agent cost: low. **Recommendation:** one sentence in `help music` (`crates/cli/src/help/music.rs:26-29`).
14. *(RGBDS, not gbcli:)* HRAM labels `hL` and `hH` read as the register `hl`. gbcli's diagnostic rows made it obvious. No change.

## Repeats of earlier games' stumbles

- **Write timing has no direct check** (A/B item 1, about 1 h 20 min across both games). C again built STAT-conditioned breakpoints, now to prove a 2 KB VRAM copy safe. The A/B fixes held (`--raster` gives `mode=`; the HBlank recipe). What remains is C-3, C-4 and the one-byte watch in `help traps`. C has no sprites, so it never hit the parked "STAT late in a batched mode 3" window; that entry's condition doesn't hold.
- **A rebuild costs the session** (A/B item 5). Mostly fixed (`run --from boot` keeps breakpoints, which follow their symbols); C ranks that a help. Leftover: C-6.
- **Per-row output floods** (A/B item 4; D/E item 1). C-7 is the third time.
- **Python art, and fonts in particular.** Three games wrote a font generator (rogue 89 lines, rhythm 103, C 141). Pixel fonts landed after C's pin: a `.yaff` font, `MAIN_FONT_CHARMAP hud, $A0` and `TextLoadTiles` would replace C's glyph table and charmap script; the oil bar and the title's double-size lettering would stay pixel text. **A trap for the next game:** the starter now reserves tiles 236-255 as its line buffer (`help fonts`); a game that uses the `$80-$FF` block for its own tiles must move or drop it.
- **Tile constants.** C's script emitted `FONT_BASE` and `FONT_BAR0`, not hard-coded numbers, so the parked "Tile and frame names as program constants" condition doesn't hold by its wording; fonts' charmaps now cover the glyph half.
- **`--dir` across verbs** (A/B item 10). C-12 is a leftover.

## Helps (what worked)

1. **`gbcli measure` on named spans with `--keys-file` and `--max`.** Every number in the eight-round table came from two deterministic measures a build (`tools/bench.py`); the final bound is one assertion.
2. **`shot --scale 1`, `read`, `view memory --out` and `--poke`.** Together, a pixel-exact host reference renderer at 12 poses on two machines; it caught a clobbered carry and a frozen view within minutes. `view memory` (D-1's fix) was used on its first day.
3. **Deterministic runs with key files, `--until` and `--expect`.** The bot won on its first run; "it can be lost" and "START returns to the title" are one yes-or-no run each. `--expect` (E-2's fix) carried every gameplay claim.

Also: the profile's `idle_t` column said "copy-bound" unasked; equiv caught a section-layout bug in one call; writer `symbol=` on audio rows proved the channel split in one row; `build/gbc/` kept both ROMs apart; the "frames and keys" paragraph in `help time` left no frame-boundary stumble; `gbcli song` and `gbcli pixels --preview` caught errors before a build ("the smoothest part of the project"); build errors are exact.

## The bot, and the embedded-scripting entry

The entry is [backlog.md](backlog.md) ▸ "Embedded scripting for input bots (QuickJS)". C's bot (`tools/bot.py`, 179 lines) won in **108 gbcli calls and 3.8 s** (4.3 s on a rerun, 5.9 s under load), about 35-55 ms a call: a BFS over the map, then one call per turn and one per straight stretch, writing its key changes so one `--keys-file` run replays the win. The agent ranked it a help and judged that "for this game, no embedded scripting is needed"; it estimated a bot steering every 4 frames would make about 1,000 calls (about 50 s), where in-process scripting would start to pay.

**Does the un-park condition hold?** No, on every clause: the bot is a help, not a stumble; it decides per segment, not every frame or two; no web work has started.

**What the web ruling changes** is the argument, not the verdict: portability, not speed. C adds one fact that matters: its bot is mostly *planning* (BFS) and only lightly *driving*. A small built-in input driver could run the segments but not the BFS; each host driving the protocol in its own language runs it all, but a bot doesn't move between hosts; QuickJS runs it all, in every host. **Recommendation:** keep it parked, condition unchanged, and record C's figures and the "planning, not just driving" note (question 7).

## What the brief got wrong

- **"Make `.gbcli/` by hand."** `gbcli new` already does (`created path=fresh/.gbcli/`). The stale line is [2026-11-08-example-games-plan.md](2026-11-08-example-games-plan.md), line 33.
- **"`gbcli help traps` names a command" for "no VRAM write in mode 3".** The trap watches one byte (`crates/cli/src/help/traps.rs:13-21`), and `watch` has no range form. Proving a 2 KB copy safe needed STAT-mode breakpoints on the copy's end. **Recommendation:** a "whole copy" line in the trap; C-3's warning makes the proof one run.
- **`ly` as the VBlank test.** The brief didn't say it, but the help steers there (C-4).
- **"`gbcli equiv` against the baseline" in every ray2 round** can't work for a renderer whose speed changes timing without a frame-locked ROM mode (C-1). A speed-showcase brief should say so, or make the reference check the primary proof.
- **`--dir` "to the same effect"** holds for every verb but `build` (C-12).
- The D/E review's "new for C" brief items were all followed and paid off. **Backlog housekeeping:** "Conditions that compare a bare RAM symbol" (2026-11-07) landed in `655a334`; by the backlog's rule it should be deleted.

## Questions for Ben

1. **equiv for timing-changing builds:** a `help optimize` paragraph and `equiv --screen off` now, a warning row when the two sides' first differing frame starts at a different `t` (C-2), and `--sync-on EXPR` parked? *Recommend yes.*
2. **Report ignored writes:** a per-run warning when VRAM writes land in mode 3 or OAM writes in modes 2-3, with count, first PC, line and frame, in the LCD-off warning's form? *Recommend yes;* C's costliest silent bug, and the core can count it exactly.
3. **Breakpoints in `show`:** a row per breakpoint and watch with its condition, `condition` in the `break`/`watch` results, ids never reused? *Recommend yes,* in `show` rather than a new verb.
4. **Configure verbs without the head:** `break`, `watch` and `delete` resolve against the ROM on disk, and `rom_changed` text is capped with the full list in `--json`? *Recommend yes.*
5. **Frame-budget lag rows as runs** (`frame_lag from= to= count=`), with a cap? *Recommend yes* (545 rows become 27 on the walk).
6. **A `mode` keyword in expressions** (the PPU mode, `condition_error` where unknown), plus the `ly` caveat in the help? *Recommend yes.*
7. **Embedded scripting:** keep it parked, condition unchanged, recording C's 108 calls and 3.8 s and that its bot is mostly planning? *Recommend yes;* the web spike decides.
8. **The small items, as two leaves?** *Recommend yes:*
   - **docs:** `measure`'s max (C-8), `shot`'s PNG format (C-9), `hUGE_init`'s one write (C-13), the trap's whole-copy line, the example-games plan's `.gbcli/` line; backlog `shot` shades as data;
   - **CLI:** `measure --from boot|power-on` and a correct unknown-flag remedy (C-10), one symbol namer for stops and profiles (C-11), `build --dir` with its usual meaning (C-12), and deleting the landed bare-symbol backlog entry.

## Across all five games: the three changes that would have saved the most agent time

Ranked by the time the five DX docs report; each counts what was lost before a fix, so some have since landed.

1. **Reporting writes the hardware ignored or landed in the wrong mode** (about 1 h 20 min for A/B, plus C's three timing stumbles, one found only by eye; the rhythm game's `LcdOff` spinning on LY found only through the frame budget). `mode=` on raster writes landed after A/B; the ignored-write warning (question 2) is the part still missing.
2. **Sessions and comparisons that survive a change in startup or layout** (about 15 min in every A/B session; D 45 min across the wrong build and key files that broke with startup time; E replaying 4,700-frame runs from boot; C most of an hour on equiv's frame offset). Landed: `run --from boot` keeping breakpoints, labels that follow rebuilds, `build/gbc/`, `--keys-since`, `model=`/`cgb=`. Open: C-2 and C-6.
3. **Output shaped for an agent: summaries in text, every row in `--json`, each per-frame row with its frame** (A/B 30 min plus a budget script in each game and 20 min on floods; D/E about 25 min plus a script each; C a flood in every leaf). Landed: `lcd_off`, one-row `keys_unapplied`, frame numbers. Open: lag runs (question 5).

Runner-up: **speed-independent equivalence** (C, about an hour), one game's cost, and question 1 explains why gbcli can only partly fix it.
