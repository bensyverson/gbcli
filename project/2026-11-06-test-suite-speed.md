# Finding: the test suite's speed, and the 30-second budget

**Ruling (user, 2026-11-06): the warm test suite must stay under 30 s.** "Warm" means the tests are already built: `cargo test --workspace` after `cargo test --workspace --no-run`. Exceptionally slow tests go to a separate pre-release suite rather than slow down the feedback loop.

**Outcome: no test needed moving.** The suite was slow because debug-build Rust drives the emulator. Building the dev profile at `opt-level = 1` cut its CPU time to a third, and left the slowest test binary at 2 s.

## Measurements

The host was heavily loaded throughout: a load average of 33–106 on 8 CPUs, from other sessions, a Swift test run and Mail. So wall times are inflated and noisy. CPU seconds (user + system) are the steadier comparison. Apple M1, 8 CPUs, rustc 1.99.1, SameBoy at `-O3` in both configurations.

| | opt-level 0 (before) | opt-level 1 (now) |
|---|---|---|
| Warm suite, `cargo test --quiet --workspace --all-targets` (`/usr/bin/time -p`) | 28.6 s wall, 38.4 s user (load ≈33) | 13.8 s wall, 12.5 s user (load ≈31) |
| Sum of test-binary wall times, `scripts/bench/test-times.py` | 33.2 s, 42.5 s CPU (load 67) | 12.7 s, 14.5 s CPU (load 104) |
| Clean debug build, `scripts/bench/workspace-build/run.sh 3` | 10.7 s | 16.4 s |
| Incremental, touch `crates/gbcli/src/main.rs` (median of 3) | 0.26 s | 0.24 s |
| Incremental, touch `crates/engine/src/lib.rs` (median of 3) | 0.94 s | 0.87 s |

A commit's hook run of `cargo test` took 2:08 earlier in the session, at a load average of about 106. That is host contention, not the suite.

There are no doctests (`cargo test --workspace --doc`: 1.1 s, zero tests).

**The slowest tests at opt-level 0**, each run alone (wall):
- `decoded_registers_match_bus_reads_on_a_cgb` 5.0 s, and `…_on_a_dmg` 4.5 s (`gbcli-sameboy::io_decode`);
- `every_register_decodes_as_the_cpu_reads_it_on_a_dmg_and_a_cgb` 3.5 s (`gbcli-engine::inspect_hwreg`);
- `inspection_never_changes_the_state_at_random_points` 2.6 s;
- `nothing_reaches_stdout_even_when_sameboy_logs` 2.5 s;
- `save_load_continue_equals_straight_through_on_cpu_instrs` 2.5 s.

All are CPU-bound, Rust-side loops over emulator state, which opt-level 1 speeds up most.

**At opt-level 1, the slowest binaries** are:
- `gbcli-sameboy::save_load_property`, 2.0 s;
- `gbcli-sameboy::stdout`, 1.5 s;
- `gbcli-sameboy::state`, 1.1 s. Nearly all of it is a deliberate 1.1 s sleep in `host_time_does_not_reach_the_state`, which proves host time doesn't leak into the state.

> **Update, 2026-11-06 (sixth session): the hook's 37.8–41.0 s warnings were host load, not the suite.** On a quiet machine (load 2.6 on 8 CPUs, after commit `7baaaf0`), `python3 -I scripts/bench/test-times.py` measured 48 binaries at a sum of 14.5 s wall and 19.9 s CPU. The slowest: `gbcli-sameboy::save_load_property` 1.5 s, `gbcli-sameboy::state` 1.2 s, `gbcli-server::gbcli_server` 1.2 s (6.5 s CPU), `gbcli-sameboy::stdout` 1.2 s, `gbcli-server::wall_guard_threads` 1.1 s. Nothing needed moving to the pre-release suite.

> **Update, 2026-11-07 (twelfth session, leaf 9RaRvi): 107 test binaries became 31; under budget (re-timed below).** The changes:
>
> - **One integration-test binary per crate**, `tests/it/main.rs` (the `gbcli` crate's stays `tests/cli/`, which now also holds `serve`). Five files keep their own binary, each saying why at its top: `gbcli-sameboy::stdout` and `sameboy-sys::smoke` redirect fd 1, `gbcli-server::wall_guard_threads` counts the process's threads, and `gbcli-engine::opcode_rgbasm_probes` and `opcode_expr_regressions` hold wall-clock bounds (`probe_long_flat_chains_scale` failed its 0.5 s bound once inside the merged engine binary, at load 70).
> - **music_equiv:** every comparison now starts at the boot ROM's handover (`player::booted`), so its frames are the song's; whole-song lengths keep the same ~150 song frames past the end (`TAIL`). Three samples stay warm at 600 frames (SVL - Yyna, the most effects; FADE - Strap in and Suit Up, the most subpatterns; Coffee Bat - Wyrmhole, the fastest); the other twelve run only whole, pre-release. No sample uses the timer; the test song does, warm, over its whole length.
> - **build_safety:** the hang, wrapped-hang and orphan tests misbehaved on every source the starter assembles (about seven), so each paid its 0.2 s limit or the 0.5 s stderr grace seven times; now only `src/main.asm` misbehaves. Each test alone: 3.5 s → 1.05 s (orphan), 1.5 s → 0.74 s (hang), 1.6 s → 0.78-1.15 s (wrapped), at load 24-95.
> - Not waits after all: `errors::session_busy_says_to_wait` has been pre-release (`#[ignore]`) since it was written; the per-test pass of `test-times.py` ran ignored tests (`--include-ignored`) and listed them as if warm, which it no longer does. The toolchain unit tests' and images tests' wall-over-CPU is macOS's first exec of a freshly written script, about 150 ms (later execs 5 ms; `python3 -I scripts/bench/fresh-exec.py`), not a timeout.
>
> Counts (`--list` of every test binary): 1,550 tests (21 ignored) before at `6df3703`, 1,550 (21 ignored) after on `3b72e81`: twelve warm sample tests removed (their whole songs stay, ignored), twelve added by `3b72e81`.
>
> Figures, `python3 -I scripts/bench/test-times.py --per-test-over 1000`, with other agents building throughout (so CPU seconds inflate too, and neither figure compares cleanly): before, 107 binaries, a sum of 78.5 s wall and 93.1 s CPU at load 24.3; after, 31 binaries, 39.7 s wall and 93.0 s CPU at load 95.4. The largest after: `gbcli-build::it` 13.9 s wall (21.8 s CPU), `gbcli-sameboy::it` 5.3 s, `gbcli-build::gbcli_build` 4.0 s wall at 0.1 s CPU (fresh-script execs), `gbcli::cli` 3.7 s. Whether the warm suite is under 30 s on a quiet machine is still to measure.
>
> Re-timed by the integrator at `2efed71`, with no agents running (load 20.7 at start from other sessions; not in low-power mode), `python3 -I scripts/bench/test-times.py`: 32 binaries, a sum of **23.5 s wall** and 101.9 s CPU. The largest: `gbcli-build::it` 6.2 s, `gbcli::cli` 3.2 s, `gbcli-server::gbcli_server` 3.0 s, `gbcli-engine::it` 3.0 s, `gbcli-sameboy::it` 2.9 s. Under the 30 s budget by this measure. The hook's own timing of a whole `cargo test --workspace` at the same point said 32-33 s; it includes cargo's startup per binary and doc-test passes, which the script's sum leaves out, so the budget as the hook measures it is met only on an idle machine; re-time there before moving more tests.

> **Update, 2026-11-08 (fourteenth session): re-timed on an idle machine at `734f6c3`; under budget, no tests moved.** No agents or LoadHigh jobs running, not in low-power mode, load 4.1-4.2. `python3 -I scripts/bench/test-times.py` right after a rebuild: 33 binaries, a sum of 36.2 s wall and 113.8 s CPU. Run again at once (`--per-test-over 1000`): **23.9 s wall** and 111.9 s CPU. The largest: `gbcli-build::it` 6.3 s, `gbcli::cli` 3.9 s, `gbcli-server::gbcli_server` 3.0 s, `gbcli-engine::it` 2.8 s, `gbcli-sameboy::it` 2.6 s. The first run's extra 12 s was wall time only: CPU stayed the same, and binaries doing almost no work took 0.25-1.4 s each. That fits macOS checking each newly linked executable on its first launch (not verified beyond these two runs). The hook always runs freshly linked binaries, so expect its figure to sit above the warm sum. Compare warm runs, or CPU seconds, before moving tests.

## Why opt-level 1 for the whole dev profile, not just tests

`cargo build` and `cargo test` share artifacts when they share a profile. An optimized `[profile.test]` alone would compile the workspace twice. It would also leave `cargo run` of the server and benchmarks slow.

opt-level 1 keeps debug info and debug assertions, including overflow checks. Stepping in a debugger may show more optimized-out locals; set `CARGO_PROFILE_DEV_OPT_LEVEL=0` for a session that needs that.

## The policy going forward

- The pre-commit hook builds the tests, then times the run and warns when it passes 30 s. It warns rather than fails because host load alone can push a run over.

  > **Changed, 2026-11-09 (Ben):** a warning on every commit had become noise (31-72 s under load all session, against 27 s idle). The hook now reads the load average first. On a quiet machine (load at most half the CPUs) a run past 30 s **fails** the commit. Under load it only warns past 60 s. The budget itself is unchanged.

  > **Changed again, 2026-11-09 (Ben, seventeenth session): the quiet-machine check warns, not fails.** The fonts work brought the suite to 34.8 s on a quiet machine (load 2.9 on 8 CPUs), and the hook refused a docs-only commit. Ben relaxed it: a run past 30 s on a quiet machine is a warning. The 30 s target stands.
- `python3 -I scripts/bench/test-times.py` gives the per-binary and per-test breakdown, with CPU seconds beside wall.
- A test that is valuable but too slow for every commit is marked `#[ignore = "pre-release: <why>"]` and runs with `cargo test --workspace -- --ignored` before a release. Prefer making it faster first, e.g. by sampling fewer points or using a smaller ROM. Moving a test out of the loop means it runs rarely, so it catches regressions late.

> **Re-timed, 2026-11-09 (sixteenth session):** 26.3 s and 27.2 s of wall time (126-129 s CPU), from `python3 -I scripts/bench/test-times.py` run twice with no agents compiling (load average 5.5-8.1 from other sessions, not in low-power mode). That's under budget, so no test moves to the pre-release suite. The fifteenth session's 43-51 s hook warnings were load. The largest binary is `gbcli-build::it` at 7.8 s.
