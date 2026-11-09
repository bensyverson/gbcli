# Decision brief: spike a bespoke Rust core now, or park it

> **Decided (Ben, 2026-11-08): park it**, with the un-park condition below. The entry is in [backlog.md](backlog.md) ▸ A bespoke Rust emulator core.

2026-11-07, thirteenth session. **This is the evidence for leaf qfXJij, with a recommendation; the decision is Ben's.** The question and both options are in [2026-11-06-backend-evaluation.md](2026-11-06-backend-evaluation.md) ▸ The bespoke core question. The leaf names four inputs, each below with its source.

## The evidence

**1. The gap ledger: zero.** Five fresh Sonnet agents built, ran, measured, profiled, equivalence-checked, listened to and drew DMG programs with gbcli: about 145 calls over 13 sessions. `gbcli gaps` printed `none` in every run's project. Run 1's first two sessions were deleted before the ledger was read. Source: [2026-11-07-agent-eval.md](2026-11-07-agent-eval.md) ▸ Summary.

**2. Eval stumbles caused by core limits: none.** Every stumble in five runs was in gbcli's docs, errors or verbs, and each was fixed at the source. None was in what SameBoy can or can't do. Same source.

**3. SameBoy's shootout failures: 9 DMG tests, one family.** Every DMG failure is a mid-mode-3 write to LCDC or SCY, judged against Mealybug's "DMG-blob" screenshots, which come from another emulator, not hardware. The other two failures are SGB, which gbcli doesn't target. Source: [2026-11-06-shootout-failures.md](2026-11-06-shootout-failures.md). The corpus baseline at SameBoy `ea667ad` is 4,119 pass, 324 fail, 48 skip over 4,491 runs. Most of the failures are docboy's own suite (173) and gbmicrotest (78): [2026-11-06-sameboy-baseline.md](2026-11-06-sameboy-baseline.md).

**4. Fidelity issues found in use: none against hardware.** Twelve sessions of building on the wrapper have recorded SameBoy quirks, and each was handled in the adapter:
- a blank frame published after the LCD turns on (project/gotchas.md);
- random power-on WRAM;
- a CGB repeated frame that publishes nothing (205f273's note).

None is a disagreement with hardware that a user's game would hit.

**What the wrapper still can't do, by design.** The core trait answers *unsupported* for capabilities SameBoy lacks:
- side-effect-free reads;
- sub-instruction stepping;
- saved cycle counters;
- and the rest the evaluation lists ("For", in the bespoke section).

These would count in the ledger the moment an agent asked for them, and none did. The eval task didn't ask for them either: dot-level raster work, OAM-bug-sensitive code, cycle-exact mid-scanline effects.

## Recommendation: park it

Park the spike in `project/backlog.md`, with this un-park condition. Un-park when any one holds:
- **The ledger:** an agent's real work (an eval run or a LoadHigh session) records an `unsupported` that blocks its task, and no adapter work can serve it.
- **Fidelity:** a disagreement between SameBoy and hardware truth (a test ROM with a hardware-verified reference, or a capture from hardware) affects a program gbcli users write, and upstream won't take a fix.
- **The task class:** an eval task class that needs mid-mode-3 effects, which is SameBoy's one failing family, is added and fails on SameBoy where docboy passes.

**Why park:**
- The evidence the decision was scheduled to read is empty on the bespoke side: no gaps, no core-caused stumbles, and failures only where the reference itself isn't hardware.
- The spike's cost is about 6 agent sessions to reach SameBoy's pass list on a DMG-only, no-APU subset.
- The work in front of LoadHigh (Stage 10's GBC images, the assets op) is all above the core.

**What would make "spike now" right instead:** a judgment that the spike is worth doing for its own sake, as a long-run hedge, or for the agent-designed core's ergonomics (no side-effecting reads, layer identity, a saved cycle counter), before any user hits those limits. That's a values call, not something this evidence settles. It's the reason to ask rather than decide.

**Done 2026-11-08:** Ben parked it; the backlog entry carries the un-park condition above, and qfXJij closed with a pointer to this brief.
