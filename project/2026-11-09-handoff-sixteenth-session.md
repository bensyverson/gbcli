# Handoff: 2026-11-09 (sixteenth session)

This handoff is current until the next commit after the one that adds it. Start with `job orient`, then read this. The previous handoff is [2026-11-09-handoff-fifteenth-session.md](2026-11-09-handoff-fifteenth-session.md).

## State of play

Ben was around and ruled as things came up. Everything is landed, reviewed or under review as listed below, and **pushed**. No agent is running, and no worktree is left.

**Ben's rulings this session**, each recorded where it applies:
- **D/E DX review:** every recommendation accepted ([2026-11-09-examples-dx-review-de.md](2026-11-09-examples-dx-review-de.md), decisions at the end): `view memory`, short song blocks, `run --expect`, `run --keys-since` with frame fields, and three small leaves. Embedded scripting stays parked, with a sharper un-park condition (backlog.md); C is its test.
- **`build --gbc` defaults to `build/gbc/NAME.gbc`.** It gets its own folder because every tool finds the `.sym` by swapping the extension (backlog.md ▸ Small run and view refinements).
- **Pixel fonts, ruled in a revised form** after LoadHigh's response ([2026-11-09-pixel-fonts-design.md](2026-11-09-pixel-fonts-design.md) ▸ Ben's decisions):
  - a strict YAFF file is each font's one source, with gbcli's own parser (no `libyaff`);
  - a `FontAsset` in the `assets` op;
  - the 254-glyph cap applies only to what `build` emits;
  - accented letters come from a mark table gbcli keeps;
  - three leaves: core, then authoring, then the renderer.
  
  The summary passed to LoadHigh is [2026-11-09-pixel-fonts-for-loadhigh.md](2026-11-09-pixel-fonts-for-loadhigh.md).
- **C (the raycaster):** the `--gbc` build may use double speed if the DMG build reaches its target alone, and C waits for the D/E fixes, which are now in ([2026-11-09-raycaster-plan.md](2026-11-09-raycaster-plan.md), questions).
- **Process:**
  - The hook fails a commit when the suite runs over 30 s on a quiet machine (load ≤ half the CPUs), and only warns past 60 s under load.
  - The help golden is split, one file per command, under `golden/help/`.
  - A new shared **`subagent` module** (agents-md `fd85070`, a default for new repos; enabled here): the rules every subagent follows are in `project/agents/subagent.md`, and the briefing template in delegation.md is 12 lines.

**Main-thread calls Ben hasn't seen** (two-way doors):
- `--keys-since boot` before the boot handoff is **refused**, not deferred.
- A banked or region `view memory` block that crosses its area's end is refused, naming the length that fits.
- `SessionRow.model` became `Option<Model>`, since `Model` has no default.
- A writer PC that the boot ROM ever wrote from stays unnamed, even for cart writes from the same PC.

**Landed:**

| Commit | What |
|---|---|
| `7bfeed1`, `aed4b52`, `8fc96be` | The raycaster plan (draft), the D/E DX review and its rulings, the idle suite timing |
| `1121835` | build-de: short song blocks, the starter's restart vectors, the palettes.txt cascade, `build/gbc/NAME.gbc`, help fixes |
| `634c8d0` | run-de: `run --expect`, `--keys-since`, frame numbers on frame-budget and audio rows, the expression-as-ROM refusal |
| `78b7761` | obs-de: `view memory`, model/cgb in `show` and `sessions`, writer symbols, `idle_t`; the LoadHigh font summary |
| `5a268fb` | Hook budget by load; help golden split |
| `4a77761` | Adopts the subagent module |
| `bd0d11b`, `c608570`, `81ae475` | Fixes for the three reviews' findings (fix-run, fix-obs, fix-build) |

**Reviews.** The three D/E fix commits were reviewed (review-run, review-obs, review-build, each given the code and a NEVER list), and their leaves are labeled `reviewed`. The reviews found four medium findings: keys-since before the handoff, a bare-symbol `--expect`, boot ROM writer symbols, and help time's $0040 claim. All four are fixed. **The three fix commits (`bd0d11b`, `c608570`, `81ae475`) are not independently reviewed.** They are small, built test-first from the reviewers' probes, and I read each staged diff. Review them first next session, or accept them consciously.

## Next, in order

1. **Review, or accept, the three fix commits** above (`project/agents/review.md`).
2. **Rule on `lDEWkI`** (a decision; my lean is in the leaf): extend the bare-RAM-symbol refusal to `&&`/`||`/`!` operands in `--until` and `--expect`, and leave `--since boot` before the handoff as it is, since the stop names its base.
3. **C, the raycaster.** Import [2026-11-09-raycaster-plan.md](2026-11-09-raycaster-plan.md)'s `tasks:` block under `uI1JDB` (blocked only by `d5FoEo`, now closed). Rebuild the pinned binary at dispatch. Brief with the short template: the plan's "What D and E taught the briefs" section lists what the brief must say.
4. **Pixel fonts, leaf 1 (`IQRDj5`)**, which LoadHigh is waiting on. It can run in parallel with C, since it touches gbcli's crates and C touches only `examples/raycaster/`.

**Leaves still open under Stage 11:**
- `uI1JDB`: C, the raycaster.
- `60ycZS`: fonts (`IQRDj5`, then `mDCZDQ` and `RDlaLo`).
- `XYOE8r`: review fixes; only `lDEWkI` is left open.

## What this session learned

- **Reviewer probes often assert the bug.** Two fix agents found that a probe written to show the bug can't turn green once the bug is fixed. Brief fix agents to turn each probe into a test of the opposite behavior, not to copy it.
- **The first brief using `subagent.md` is still to come.** All of this session's agents were briefed the old way. Ben suggested an A/B: brief one agent of the next fan-out the old way, and compare deviations and compliance.
- **Load is the norm on this machine** (load average 14-46 most of the session). Only the quiet-machine rule in the hook enforces the 30 s budget now. `scripts/bench/test-times.py` on a quiet machine is the real check.
