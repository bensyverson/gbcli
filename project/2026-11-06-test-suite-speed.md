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

## Why opt-level 1 for the whole dev profile, not just tests

`cargo build` and `cargo test` share artifacts when they share a profile. An optimized `[profile.test]` alone would compile the workspace twice. It would also leave `cargo run` of the server and benchmarks slow.

opt-level 1 keeps debug info and debug assertions, including overflow checks. Stepping in a debugger may show more optimized-out locals; set `CARGO_PROFILE_DEV_OPT_LEVEL=0` for a session that needs that.

## The policy going forward

- The pre-commit hook builds the tests, then times the run and warns when it passes 30 s. It warns rather than fails because host load alone can push a run over.
- `python3 -I scripts/bench/test-times.py` gives the per-binary and per-test breakdown, with CPU seconds beside wall.
- A test that is valuable but too slow for every commit is marked `#[ignore = "pre-release: <why>"]` and runs with `cargo test --workspace -- --ignored` before a release. Prefer making it faster first, e.g. by sampling fewer points or using a smaller ROM. Moving a test out of the loop means it runs rarely, so it catches regressions late.
