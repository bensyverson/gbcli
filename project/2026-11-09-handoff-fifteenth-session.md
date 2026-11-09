# Handoff: 2026-11-09 (fifteenth session)

This handoff is current until the next commit after the one that adds it, unless that commit is one of the game merges named under "In flight" below. Start with `job orient`, then read this. The previous handoff is [2026-11-09-handoff-fourteenth-session.md](2026-11-09-handoff-fourteenth-session.md).

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

Two game agents, each one chain of four session leaves in its own harness worktree, using the pinned binary `/Users/ben/.cache/gbcli-examples/target/release/gbcli` built from `c38cd29` (it has the big-PNG `gbcli pixels` regression fixed in `08752b0`; unlikely to matter at Game Boy sizes):
- **rogue15**: D, the roguelike (`QdPmrv`, `JQiBBs`, `H1Fbvm`, `RRqjrZ`), worktree `.claude/worktrees/agent-a5923bad21ab961aa`, DX doc `project/2026-11-09-dx-rogue.md`.
- **rhythm15**: E, the rhythm game (`Uw6Adb`, `LM71mY`, `e5Arro`, `ahhhvb`), worktree `.claude/worktrees/agent-a9a8e774f1e46ea56`, DX doc `project/2026-11-09-dx-rhythm.md`.

If this session ends before they report: each worktree holds the agent's uncommitted work and job notes on its leaves say where it stopped. Integrate as usual (snapshot with `write-tree`/`commit-tree`, `git merge --squash`, read the diff, commit through the hook, close the finished leaves, push). A leaf left unfinished goes to a fresh agent with the same brief (below) and a summary of the state. When both games are in, add them to `examples/README.md`'s table and to `scripts/examples/build-roms.sh` if the agents didn't, then do the DX review for D and E as `review-ab` did for A and B, before C (`uI1JDB`).

## Next, in order

1. Integrate the two games (above).
2. Read both DX docs and `gbcli gaps`; bring Ben a ranked list (the shape of [2026-11-08-examples-dx-review.md](2026-11-08-examples-dx-review.md)).
3. Re-time the warm suite on an idle machine: every hook run this session warned 43-51 s against 30 s, all taken with agents compiling (load average 17-25). `python3 -I scripts/bench/test-times.py`, twice, take the second; only then move tests to pre-release.
4. Then C, the raycaster: detail `uI1JDB` after D and E.

## Reusable briefs

The briefs are in this session's transcript only; their substance:
- **Game agents:** the user contract (the pinned binary by absolute path; learn only from `gbcli help`/topics, `recipes`, `--help`, `schema`, README.md, examples/README.md and the two plans; never `crates/`, `docs/`, `scripts/`, other `project/` docs, or another example); `gbcli new examples/<game>`; both builds every leaf; edit and generator scripts in files run with `python3 -I`; `.pixels` art, woodcase available; a blind DX doc per leaf with stumbles ranked by cost; Ben's halfway context rule; report with top five stumbles and three helps.
- **Fix agents:** the delegation template, plus the reviewer's probes as red tests (copied read-only from the reviewer's kept worktree), DO NOT BUILD naming the other agents' files, re-bless shared goldens.
- **Reviewers:** SURFACE (commits and files) / GOAL as NEVER, both directions / failure classes / BAR (probe or trace, SUSPECTED otherwise, "nothing serious" acceptable). This session the NEVER lists found a medium in every round where the fix agent's tests were green.

## Mistakes, so the next session doesn't repeat them

- **Piping a gating command.** `git commit … | grep` hid a `cargo fmt` failure from the hook; the commit silently didn't happen and the push said "Everything up-to-date". AGENTS.md already forbids it: redirect to a file and check the exit status.
- **A fix agent's own tests pinned its regression.** pixfix15's test asserted `read(big PNG).is_err()`, locking in the medium the next reviewer found. A test that asserts a new refusal should also assert what must still work.
