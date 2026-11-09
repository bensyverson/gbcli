# Handoff: 2026-11-09 (nineteenth session)

This handoff is current until the next commit after the one that adds it. Start with `job orient`, then read this. The previous handoff is [2026-11-09-handoff-eighteenth-session.md](2026-11-09-handoff-eighteenth-session.md).

## State of play

Ben was around for most of the session and ruled as things came up. Everything is landed and **pushed**, including the branch `spike/web`. No worktree is left. **Stage 11 (example games) and Stage 12 (the web spike) are both closed.** The only open leaf is `zkoPft` (replace rgbgfx), which Ben asked to file and which waits with the paused web track.

**Landed on main** (in order): `3276433` architecture.md one sentence per line (retires the conflict gotcha); `a0a4f63` RST entry naming, singleton font keys, NFC wording (rev-cli2's findings); `289a4a5` the mode caveat names the window (rev-hw2's finding, probes kept); `5cd60dd` raster text bounded to 64 lines, riskiest writers first (VrZPr5); `878d98f` `wall_guard` error code and `--wall-guard` (xdES5r, **PROTOCOL_VERSION 5**), the wave channel's octave in `help music` (qjAObe); `705f9ae` LCD-off raster writes count as `lcd_off=` (rev-wg's findings); `348b02f` rev-mt's wording nits; `6209173` song export counts a shared pattern's instruments per channel kind (xdkoZy); `4e815ed` unique build-test temp dirs (eJ6IfH); `0949f0d` a no-MBC build that spills past bank 1 or bank 0 relinks as one 32 KB bank (Rf5UAe); `edeee27` the export's stop at wave 15 named as a departure; `10b558a` a failed one-bank relink names rgblink's real error (XqVcCd); plus plan, findings, decision and job-log commits.

**Ben's rulings this session**, each recorded where it applies:
- **Next work after Stage 11 was the web spike** ([2026-11-09-web-spike-plan.md](2026-11-09-web-spike-plan.md): two sessions, Node plus a tiny page, installs allowed, spike code on a branch, RGBDS research only).
- **After the spike** ([findings, Ben's decisions](2026-11-09-web-spike-findings.md)): keep SameBoy, the bespoke core stays parked (backlog entry updated); **pause the web track**; spike code stays on `spike/web`. rgbgfx: filed as `zkoPft`, not scheduled.
- **Song export:** fix it, departing from hUGETracker's export (note on xdkoZy; module doc and help).
- **Starter overflow:** relink with `rgblink -t` on a no-MBC spill (note on Rf5UAe). Fonts are not subset by usage, only by `NAME.charlist` past 254 glyphs; Ben thought they were.

**The web spike's answer, in one paragraph:** the real protocol handler (SameBoy, engine, server, even the CLI) builds for `wasm32-wasip1` with wasi-sdk 34, unmodified apart from four run-time cuts, answers byte-identically to native, and runs `layers.gb` at about 800 fps in headless Chromium (0.68 of native, 13x real time), 1.4 MB gzipped. Two traps: link as a WASI reactor (`_initialize`) or every export reruns SameBoy's constructors; the browser shim needs file times for the bundle store. RGBDS 1.0.4's rgbasm, rgblink and rgbfix build unpatched with Emscripten (0.37 MB gzip); nobody ships rgbgfx, and gbcli only uses it as an encoder of its own plan ([2026-11-09-web-spike-rgbds.md](2026-11-09-web-spike-rgbds.md)).

**LoadHigh:** Ben was given a note to paste about protocol 5 (`wall_guard`, `wall_guard_s`), the singleton key message and the raster cap.

**Machine changes:** the RGBDS researcher's `brew install emscripten cmake bison` upgraded node and python@3.15; web1 installed the `wasm32-wasip1` and `wasm32-unknown-unknown` Rust targets and wasi-sdk 34 under `~/.local/wasi/`; web2's Playwright Chromium (219 MB) is under the spike worktree's `target/`, now removed with it.

## Reviews

Reviewed and fixed: `ae7261d`, `2ac2a03` (from the eighteenth session), `5cd60dd`, `878d98f`, and the main thread's own `a0a4f63`, `289a4a5`, `705f9ae`. rev-bld reviewed `6209173` (no findings; all 15 samples' tables match hUGETracker's export) and `0949f0d`: **one medium and two low findings, filed as `XqVcCd`** (a failed one-bank relink blames size and hides rgblink's real error). Fixed in `10b558a` (agent relinkfix; the three probes flipped into `rom_banks.rs` tests). **Not independently verified: `10b558a`** (checked only against the reviewer's probes by its author, and by the integrator's read): have a fresh verifier run the reviewer's three cases first next session. `4e815ed` is test-only. The `spike/web` branch is spike code, exempt.

## Next, in order

1. **Verify `10b558a`** (above).
2. **Ask Ben what gbcli does next.** There is no plan past Stage 12. Candidates raised this session: a third wave of example games (the best source of real bugs so far); capping `run --profile`'s routine rows (the last list design §12 knows is unbounded); the library entry point (a `Handler` constructor taking host, store and interrupt; a store trait; help as an API), if the agent-tool goal moves up; `zkoPft` (rgbgfx in Rust, about one session) if the web track resumes.
3. If a pre-release run is due, `cargo test --workspace -- --ignored` now passes the music samples in parallel (`music_equiv`, 22 tests, about 60 s).

## What this session learned

- **One sentence per line ended the architecture.md conflicts:** every agent clause this session spliced in without a conflict.
- **A spike's first finding can be that the seam isn't where the plan said:** wasm's std compiles processes, threads and locks and fails at run time, so the "cuts" were found by running, and the help needed no move at all.
- **Agents told `--as` without `--db` claim into a worktree replica** (songfix did): briefs now spell out the full `job --as X --db ... <verb>` form.
- **A hook warning about suite time under four parallel agents is noise** (as the gotcha says); every warm run alone stayed in budget.
