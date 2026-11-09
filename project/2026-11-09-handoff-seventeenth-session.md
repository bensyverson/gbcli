# Handoff: 2026-11-09 (seventeenth session)

This handoff is current until the next commit after the one that adds it. Start with `job orient`, then read this. The previous handoff is [2026-11-09-handoff-sixteenth-session.md](2026-11-09-handoff-sixteenth-session.md).

## State of play

Ben was around and ruled as things came up. Everything is landed and **pushed**. No agent is running, and no worktree is left. The main thread stopped at its 33% context line.

**All five example games are done.** C, Lantern (`examples/raycaster/`, `301e65f`), closed Stage 11's game leaves. **All three pixel-font leaves are done** (`cff86f7`, `daf6d93`, `6feaf3e`), so LoadHigh can consume `FontAsset` (PROTOCOL_VERSION 3).

**Ben's rulings this session**, each recorded where it applies:
- **`lDEWkI`:** a RAM symbol taken as a truth value is refused in every condition ("a classic pointer error; help agents avoid it"). `--since boot` before the handoff stays as is. Design §7, update of 2026-11-09.
- **Fonts leaf 1's calls stand:** `assets/fonts/NAME.charlist`, PROTOCOL_VERSION 3, keys per font in the `assets` request ([2026-11-09-pixel-fonts-design.md](2026-11-09-pixel-fonts-design.md), end).
- **Design nothing that a web build, or a Rust library called as an agent's tool, couldn't use.** Both are real, not immediate, goals. One line in the AGENTS.md head; what it asks and the known debt: [2026-11-09-web-target.md](2026-11-09-web-target.md). The embedded-scripting backlog entry now argues portability, not speed.
- **The pre-commit hook's 30 s budget now warns instead of failing** on a quiet machine, after the fonts work brought the suite to 34.8 s (load 2.9 on 8 CPUs) and the hook refused a docs commit. 30 s stays the target ([2026-11-06-test-suite-speed.md](2026-11-06-test-suite-speed.md), docs/development.md). No re-timing asked for.
- Ben accepted the main thread's two-way-door calls from the last two sessions without review ("you have more context than I do").

**Landed:**

| Commit | What |
|---|---|
| `4ca5a6e` | The raycaster plan imported under `uI1JDB` |
| `655a334`, `d6c136c` | `lDEWkI`; review-fix's findings (nested truth positions, `--restore` remedy under `--out`, stored-condition remedy) |
| `cff86f7` | Fonts 1: strict YAFF, fonts in the build, `FontAsset`, `gbcli font`, `help fonts` (PROTOCOL_VERSION 3) |
| `c085c52` | review-fonts' findings (no generated files for a warning alone, YAFF value lines, truth inside a read's address) |
| `301e65f` | Lantern, wired into `examples/README.md` and `scripts/examples/build-roms.sh` (`examples/raycaster/lantern.gb`) |
| `4d3836c`, `03100fc` | The web-target and library rulings |
| `daf6d93` | Fonts 3: the starter's `src/text.asm` (`TextDraw`, `TextLoadTiles`) and an original `main.yaff` |
| `6feaf3e` | Fonts 2: `font edit/save/import/export/new/diff`, `.chars`, BDF, composed letters (six additive ops, PROTOCOL_VERSION stays 3) |

**Reviews.** review-fix reviewed `bd0d11b`, `c608570`, `81ae475`, `655a334`; review-fonts reviewed `cff86f7` and `d6c136c`. All findings are fixed, and those leaves are labeled `reviewed`. **Not independently reviewed: `c085c52` (small), `daf6d93` (the starter renderer), `6feaf3e` (font authoring, about 2,000 production lines, new protocol ops).** Review them first next session (`project/agents/review.md`): `job ls --status done --leaves --without-label reviewed --since 2026-11-07T04:55:00Z`.

## Next, in order

1. **Review `6feaf3e`, `daf6d93` and `c085c52`.** One reviewer for fonts 2 (protocol ops, BDF/PNG round trips, the newer-copy refusal, composition), one for the starter renderer (TextDraw against the layout rules, the starter's new VRAM line buffer at tiles 236-255).
2. **Ben accepted every recommendation of [2026-11-09-examples-dx-review-c.md](2026-11-09-examples-dx-review-c.md)** (leaf `rsUZcp`; decisions at the end). He also accepted a ninth item: a general rule for agent-shaped output (any per-frame or per-event list prints a summary or capped rows in text, every row in `--json`) with a guard test. The top asks: a warning for VRAM/OAM writes the hardware ignored, breakpoints listed in `show`, frame-budget lag rows as runs, a `mode` keyword, and `equiv --screen off` plus a start-offset warning. File the accepted fixes as a container under Stage 11 and fan out by file surface.
3. **Stage 11 then closes** once `rsUZcp` and its fixes are done. There is no plan beyond it yet: ask Ben what comes next (the web spike and the library entry point are both ruled "not immediate").

## What this session learned

- **The briefing A/B (Ben's suggestion) showed no compliance difference.** ray was briefed the old way, with every rule inline. It read `subagent.md`, `harness.md` and `gotchas.md` anyway, because AGENTS.md tells every subagent to, so the A/B wasn't isolated for game agents. fonts1-3, review-fix and review-fonts got the 12-line template and followed it. Each returned DEVIATIONS and pushback, and none committed or touched the main checkout. The one agent that broke the rules (compound commands, a shell variable holding a path) was dx-review-c, a read-only research agent with no worktree, which also skipped the start steps. **Brief agents without a worktree to read `harness.md` and `gotchas.md` explicitly.**
- **Parallel fan-outs that touch the starter conflict in tests, not just in code.** fonts2's tests assumed a starter without a font; merged after fonts3, two of them failed in the hook (fixed in `6feaf3e`). When one agent changes the starter, tell the others their test fixtures will see it.
- **The game agents' bot answered the scripting question for speed:** 108 calls, about 4 s. Portability to the web is now the argument (backlog entry).
- **Naming:** three games are lanterns (Lantern Drum, Lanternfall, Lantern). Ben hasn't asked for a rename.
