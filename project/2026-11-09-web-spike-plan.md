# Plan: the web spike (Stage 12)

2026-11-09, nineteenth session. Ben chose the web spike as the next work after Stage 11 ("we can do 1"), the first step [2026-11-09-web-target.md](2026-11-09-web-target.md) ▸ When the work starts names: the engine, SameBoy and the protocol handler built for `wasm32` behind an in-memory store, running a ROM headless from JavaScript. The spike answers questions; its code is disposable.

## Ben's rulings (2026-11-09)

- **Time box: two agent sessions** for the emulator spike (build, then measure and write up), stopping at the box with a report either way. The RGBDS research is a small leaf beside it.
- **Host: a Node test plus a tiny page.** Node runs a ROM headless and checks its frames against the native build; a minimal HTML page shows the screen as a demo.
- **Installs and dependencies:** agents may install the `wasm32` Rust targets, a C sysroot for SameBoy (wasi-sdk or Emscripten, via Homebrew or the vendor's release) and `wasm-bindgen-cli`. New crates go only into a spike crate on a branch (`spike/web`), never into main's workspace; what lands on main is decided from the findings.
- **RGBDS: research only.** Study how daid's rgbds-live builds RGBDS for the web, try it if cheap, and say what gbcli's build seam would need. No integration.
- **Models:** the build session gets Opus (a C toolchain for `wasm32`, FFI and seam choices, unknown failures); the RGBDS research can be Sonnet.

## What we know going in

- No Rust dependency is an obvious blocker: serde, serde_json, png, lexopt.
- SameBoy is C and needs a libc. Apple's clang lists a `wasm32` target but ships no sysroot; no `wasm32` Rust target is installed.
- The coupling is in the adapter: `gbcli-server` depends on `gbcli-build` (RGBDS subprocesses, `PATH`), `gbcli-bundle` (the filesystem, file locks) and its watchdog thread (`crates/server/src/watchdog.rs`). `gbcli-engine` depends on `gbcli-sameboy` directly. `Host::from_process` reads the working directory and `PATH`.
- Runs are deterministic (ROM, inputs, seed), so a frame hash from Node can be compared with the native build's exactly.

## What the findings decide

The store's design (an in-memory or browser-storage bundle store behind a trait); how the handler is built without a process (host, store, no watchdog thread); the toolchain (wasi-sdk vs Emscripten vs `wasm32-unknown-unknown` with a small libc); whether the bespoke-core backlog entry un-parks (its condition: SameBoy can't be built or run acceptably for `wasm32`); and the embedded-scripting question (backlog ▸ Embedded scripting for input bots), whose un-park condition names the web.

## Tasks

Imported 2026-11-09 under the root `9b5RBA`; the tracker is the plan from here (`job show` the container).

```yaml
tasks:
  - title: 'Stage 12: Web spike (wasm32)'
    ref: stage12
    labels: [spike]
    desc: |
      A time-boxed spike: do SameBoy, the engine and the protocol handler build for wasm32 and run a ROM from JavaScript, and what would a web build of gbcli need? Ben chose it on 2026-11-09 as the next work after Stage 11. The plan, Ben's rulings (two agent sessions, Node plus a tiny page, installs allowed, new crates only on the spike/web branch, RGBDS research only) and what the findings decide: project/2026-11-09-web-spike-plan.md. Background and the web ruling: project/2026-11-09-web-target.md.
    children:
      - title: 'Web spike 1: SameBoy, the engine and the handler built for wasm32, a ROM run from Node'
        ref: build
        desc: |
          Why: the web ruling (AGENTS.md; project/2026-11-09-web-target.md) says nothing new may be designed that a web build couldn't use, but nobody has tried a web build. This session finds out whether the emulator half of gbcli builds and runs for wasm32, and what it costs. Session 1 of a two-session time box (Ben, 2026-11-09): stop at the end of the session with what you have and what blocked you.

          What: on a branch spike/web, a spike crate (for example crates/web, a cdylib) that builds for wasm32 with SameBoy's C compiled for the same target, and exposes enough of the protocol handler to load a ROM (bytes, not a path), run N frames with keys, and return the frame and state as the protocol's own types (JSON across the JS boundary is fine). A Node script runs a committed test ROM (crates/sameboy/tests/roms/ or the starter's build) for a fixed number of frames and checks a frame hash against the native build's for the same ROM and inputs: exact equality, since runs are deterministic.

          Already decided (Ben, 2026-11-09): agents may install the wasm32 Rust targets, a C sysroot (wasi-sdk or Emscripten) and wasm-bindgen-cli; new crates go only into the spike crate on spike/web, never main's workspace; spike code is disposable. Data, not paths, into core code; no new thread or clock in a core path (the web ruling).

          Seams: crates/sameboy-sys (build.rs with the cc crate, the shim, the vendored patches), crates/engine (depends on gbcli-sameboy directly), crates/server (depends on gbcli-build for RGBDS subprocesses, gbcli-bundle for the filesystem and locks, a watchdog thread, Host::from_process). Expect to bypass or stub these in the spike crate, or put them behind cargo features on the branch; record each cut, since the list is the finding.

          Open, the agent's call: which toolchain (wasi-sdk with wasm32-wasip1 and a WASI shim in Node, Emscripten with wasm32-unknown-emscripten, or wasm32-unknown-unknown with a minimal libc for SameBoy); try the one you judge likeliest first and say why. Out of scope: the browser page and timing (spike 2), RGBDS (its own leaf), any change landing on main.

          Risks: SameBoy's C may use libc pieces with no wasm32 equivalent (file I/O for boot ROMs, setjmp, threads); the boot ROM is embedded or loaded, check which. The cc crate's wasm32 support depends on the sysroot. If the C won't build, that is a finding (it un-parks the bespoke-core backlog entry's web condition): record exactly what failed.
        criteria:
          - 'the spike crate builds for wasm32 on spike/web, with the exact toolchain and commands recorded in the findings doc'
          - 'a Node script runs a test ROM N frames and its frame hash equals the native build''s, or the findings doc records exactly what blocked it'
          - 'every adapter cut (build, bundle, watchdog, host) is listed in the findings doc with what a real web build would need instead'
      - title: 'Web spike 2: a tiny page, timing and size, and the findings doc'
        ref: measure
        blockedBy: [build]
        desc: |
          Why: session 2 of the two-session time box (Ben, 2026-11-09). Spike 1 left a wasm32 build of the emulator and handler that a Node script drives (read its notes and the findings doc it started first). This session measures it, shows it in a browser, and writes the findings that decide what comes next.

          What: (1) a minimal HTML page on spike/web that loads the wasm, runs a ROM and draws the screen to a canvas, with keys if cheap; (2) figures: frames per second headless in Node and in a browser against the native build on the same ROM, the .wasm size (raw and gzip), and start-up time, each with the command that reproduces it; (3) the findings doc, project/2026-11-09-web-spike-findings.md (or dated the day it is written), answering: does SameBoy build and run acceptably for wasm32 (the bespoke-core backlog entry's web un-park condition); which toolchain and why; what the bundle store needs to be (in memory, browser storage) and what trait would put it behind the handler; how the handler gets a host without a process and runs without the watchdog thread; what the help, which lives only in crates/cli, needs to become reachable; what the embedded-scripting question looks like from a browser; and the RGBDS leaf's answer, folded in if it has landed.

          Already decided: the spike rulings in project/2026-11-09-web-spike-plan.md; nothing lands on main from the branch without a decision.

          Out of scope: a real web IDE, design work on the page, a production store.
        criteria:
          - 'a page on spike/web shows a ROM running, captured as a screenshot in the findings doc'
          - 'the findings doc gives fps (Node, browser, native), .wasm size and start-up time, each with its reproducing command'
          - 'the findings doc answers each question in the plan''s "What the findings decide", including whether the bespoke-core web condition holds'
      - title: 'Web spike: how RGBDS runs on the web (research)'
        ref: rgbds
        desc: |
          Why: a web IDE must build ROMs, and gbcli's build runs RGBDS 1.0.4 as subprocesses (crates/build/src/toolchain.rs, crates/build/src/project/tool.rs). The web ruling (project/2026-11-09-web-target.md) keeps RGBDS behind one seam a WASM build could swap for an in-process build of the same tool. Ben (2026-11-09): research only, no integration.

          What: an answer, written as a section the main thread folds into the web spike's findings doc (return it as text; subagents can't write report files). How does daid's rgbds-live (a web IDE for RGBDS) build rgbasm, rgblink, rgbfix and rgbgfx for the web: toolchain, patches, how it passes files (a virtual filesystem?), which RGBDS version, its license? Does upstream RGBDS build for wasm32 today? Try a build of 1.0.4 if it is cheap (an hour, not a day). Then: what would gbcli's build seam need (the tools take a virtual filesystem of the project's sources and return the ROM, .sym, .map and diagnostics as data), and how big are the binaries.

          Out of scope: changing gbcli's build crate; anything on main.

          Model: Sonnet is fine (Ben, 2026-11-09).
        criteria:
          - 'the answer names rgbds-live''s toolchain, file passing, RGBDS version and license, with links'
          - 'it says whether RGBDS 1.0.4 builds for wasm32 (tried, or why not tried) and what gbcli''s build seam would need'
      - title: 'Decide: what the web spike changes'
        labels: [decision]
        blockedBy: [measure, rgbds]
        desc: |
          Why: the spike's code is disposable; its findings decide the next plan. For Ben, with the main thread's recommendation.

          The questions, from project/2026-11-09-web-spike-plan.md ▸ What the findings decide: the store's design; how the handler is built without a process; the toolchain; whether the bespoke-core backlog entry un-parks; the embedded-scripting question; what, if anything, of spike/web lands on main; and whether the next step is the library entry point, a web build stage, or neither yet.
        criteria:
          - 'Ben''s decisions are recorded at the end of the findings doc, and the backlog entries they touch are updated'
```
