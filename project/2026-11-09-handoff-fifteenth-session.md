# Handoff: 2026-11-09 (fifteenth session)

This handoff is current until the next commit after the one that adds it. Start with `job orient`, then read this. The previous handoff is [2026-11-09-handoff-fourteenth-session.md](2026-11-09-handoff-fourteenth-session.md).

## State of play

Ben was around and ruled as things came up. The gbcli fixes from the first wave of example games (`SQhTqx`) are all landed, reviewed and **pushed**: origin is at `08752b0` or later. Ben said pushing is always fine ("We can always push with impunity"), so push after every landing.

**Ben's rulings this session**, each recorded where it applies:
- **A label breakpoint whose label is gone after a rebuild is disarmed** (kept, warned about, stopping nothing) and re-armed when the label comes back: design §7, correction of 2026-11-09 (leaf `k6LoHj`).
- **`PROTOCOL_VERSION` is 2**, one bump for `condition_error` (pushed under 1) and `ppu.mode`'s nullable shape: design §5, update of 2026-11-09.
- **Push freely.**

**Main-thread calls Ben hasn't seen** (two-way doors):
- The mixed 8x8/8x16 sheet warning reuses the `obj_pair_8x16` limit (project-level, no `image=`, count 2, max 1) rather than a new `ImageLimit` value, so the protocol stays at 2.
- pixfix15 added two marked notes to Ben's approved [pixel text design](2026-11-08-pixel-text-design.md): a hardware problem's `line=` is the line that drew the most legend characters of the tile; `--tiles` keys are the build's tile numbers only until the build merges colors (RGB555-equal, or one DMG shade).
- The VRAM-in-mode-3 trap now recommends a write watch (`watch write '$8000' --if '[$8000] != new'`, checked after the write), since `--raster` lists only IO writes.

**Landed** (each through the pre-commit hook; agent work squash-merged after reading the staged diff):

| Commit | What |
|---|---|
| `b08453e` | PPU mode `unknown`, not wrong, in SameBoy's late batch window; HBlank recipe and VRAM trap point at the landed mode; protocol 2; D and E detailed into leaves ([2026-11-09-games-de-plan.md](2026-11-09-games-de-plan.md)) |
| `448a716` | `objects: 8x8\|8x16` in `assets/palettes.txt` (and a manifest's `objects`); plain builds warn image-wide on shared shades; manifest remedies name the `dmg` field |
| `f8635de` | Disarmed label breakpoints and watches (`arming` field, persisted, backward compatible) |
| `c38cd29` | Pixel text review fixes; mixed object sizes warn |
| `08752b0` | Review last15's fixes: `gbcli pixels` reads big PNGs again (the size refusal is `to_text`'s), `write_all` partial writes, help wording |

**Reviews.** Every commit through `08752b0` is reviewed (reviewers given the code, not the authors' reports, with NEVER lists). The last round's fixes (`08752b0`) were made by the integrator from the reviewer's probes and are not independently re-reviewed; they are small (pixels write path and help).

## In flight

**Landed since this handoff was first written:** E, the rhythm game **Lantern Drum** (`examples/rhythm/`, `lantern-drum.gb`, DX doc [2026-11-09-dx-rhythm.md](2026-11-09-dx-rhythm.md); wired into `scripts/examples/build-roms.sh`), and the **pixel fonts design** ([2026-11-09-pixel-fonts-design.md](2026-11-09-pixel-fonts-design.md), leaf `5UVSjE`): ten questions for Ben, each with a recommendation; nothing is built until he decides.

And D, the roguelike **Lanternfall** (`examples/rogue/`, `lanternfall.gb`, DX doc [2026-11-09-dx-rogue.md](2026-11-09-dx-rogue.md); in `build-roms.sh`). **Nothing is in flight.** Both second-wave games are in; C (`uI1JDB`) is the last game.

**What the D and E agents said the briefs got wrong** (for C's brief):
- The leaves said to look at `scripts/examples/build-roms.sh`, which the user contract forbids; the integrator wires each ROM in at merge.
- "Put the binary's directory first on PATH" fails: the worktree guard refuses a `PATH=…:$PATH` prefix and a `$G`-style variable for the binary. Brief the absolute path only.
- `gbcli gaps` lists only what the emulator core couldn't do; it printed `none` every session. Stumbles belong in the DX doc alone.
- The halfway rule never applied: each whole four-leaf chain fit in about 35% of a 1M window.

**Top stumbles from the two DX docs, unreviewed** (for the DX review): no block memory read (`read 'wMap..wMap+16'`; the rogue used 3,075 expressions in one read); key files count frames since boot, so the plain and `--gbc` builds diverge on the same keys, and `show` doesn't say which build a session runs; the frame budget covers only a whole run, with no frame numbers in its JSON; no run-level assertion (`--expect`); song text repeats every shared pattern in full; audio-log rows have no frame number; `--read` takes one expression despite its usage line, and no signed reads.

## Next, in order

1. The DX review of D and E, as `review-ab` did for A and B (stumbles above; read both docs whole), and bring Ben the fonts design's ten questions.
2. Bring Ben the ranked list in the shape of [2026-11-08-examples-dx-review.md](2026-11-08-examples-dx-review.md); check the embedded-scripting backlog entry's un-park condition against both docs.
3. Re-time the warm suite on an idle machine: every hook run this session warned 43-51 s against 30 s, all taken with agents compiling (load average 17-25). `python3 -I scripts/bench/test-times.py`, twice, take the second; only then move tests to pre-release.
4. Then C, the raycaster: detail `uI1JDB` after D and E.

**Ben asked (2026-11-09) about embedding a JS interpreter** so agents script inside gbcli instead of writing Python. The main thread's answer: most of the Python is art (now `.pixels`), build-time tables, or output post-processing (now features); the real case is closed-loop input bots (the shooter's made 1,150 `gbcli run` calls). Recommendation: wait for the D/E DX docs; if bots dominate again, choose between a bot client over `serve` and one embedded engine on every platform (QuickJS via `rquickjs`), not JavaScriptCore (macOS-only, so a second engine on Linux, which threatens determinism). Ben's lean: QuickJS, since a `serve` bot is harder to fit in an agent loop. Recorded in [backlog.md](backlog.md) ▸ Embedded scripting for input bots, with the un-park condition. Lantern Drum's agent also wrote Python (song composer, chart, sync and key-file tools): read its DX doc with this question in mind.

## Reusable briefs

The briefs are in this session's transcript only; their substance:
- **Game agents:** the user contract (the pinned binary by absolute path; learn only from `gbcli help`/topics, `recipes`, `--help`, `schema`, README.md, examples/README.md and the two plans; never `crates/`, `docs/`, `scripts/`, other `project/` docs, or another example); `gbcli new examples/<game>`; both builds every leaf; edit and generator scripts in files run with `python3 -I`; `.pixels` art, woodcase available; a blind DX doc per leaf with stumbles ranked by cost; Ben's halfway context rule; report with top five stumbles and three helps.
- **Fix agents:** the delegation template, plus the reviewer's probes as red tests (copied read-only from the reviewer's kept worktree), DO NOT BUILD naming the other agents' files, re-bless shared goldens.
- **Reviewers:** SURFACE (commits and files) / GOAL as NEVER, both directions / failure classes / BAR (probe or trace, SUSPECTED otherwise, "nothing serious" acceptable). This session the NEVER lists found a medium in every round where the fix agent's tests were green.

## Mistakes, so the next session doesn't repeat them

- **Piping a gating command.** `git commit … | grep` hid a `cargo fmt` failure from the hook; the commit silently didn't happen and the push said "Everything up-to-date". AGENTS.md already forbids it: redirect to a file and check the exit status.
- **A fix agent's own tests pinned its regression.** pixfix15's test asserted `read(big PNG).is_err()`, locking in the medium the next reviewer found. A test that asserts a new refusal should also assert what must still work.
