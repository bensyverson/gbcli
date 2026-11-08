# Benchmarks

Re-runnable measurements behind figures in `project/2026-11-06-backend-evaluation.md` and the build-cost figures below.

## test-times.py

The warm test suite, timed per test binary (wall and CPU) and then per test inside any binary over a threshold, each test run alone. Builds first, so compile time is excluded. The warm suite must stay under 30 s; figures and the policy are in `project/2026-11-06-test-suite-speed.md`. Wall time on a loaded machine is inflated, so the script prints the load average; compare CPU seconds across runs.

```sh
python3 -I scripts/bench/test-times.py                      # per-test breakdown for binaries over 2 s
python3 -I scripts/bench/test-times.py --per-test-over 1.0
```

## cli-latency

Wall time of one `gbcli` process answering a request against a live session: process start, argv parsing, bundle open, the session lock, the head load, the request and the reply. Builds the release binary, creates a session over the committed V1 test ROM, runs it one frame so it has a head, then times N runs each of `version`, `show`, `read` and `step` and prints min, median and p95. The leaf criterion is under 50 ms (leaf LUax7P); the figure is in `project/2026-11-06-design.md` §12.

```sh
python3 -I scripts/bench/cli-latency/run.py        # N = 200 per request
python3 -I scripts/bench/cli-latency/run.py 50
```

## sameboy-throughput

Headless SameBoy (DMG_B, turbo uncapped, no boot ROM via the FF50 write) running a ROM for N frames' worth of 8 MHz ticks; also times in-memory state save/load (mean of 10,000).

```sh
# SAMEBOY = a SameBoy checkout at the pinned commit; build its static library first:
make -C "$SAMEBOY" lib CONF=release
cc -O2 -std=gnu11 -I"$SAMEBOY" -o sb_bench scripts/bench/sameboy-throughput/sb_bench.c \
   "$SAMEBOY/build/lib/libsameboy.a" -lm
./sb_bench path/to/cpu_instrs.gb 3600 1   # args: rom, frames, render(0|1)
```

Compiled with `-DSB_BENCH_CPU_TIME`, it times with the process CPU clock instead of the wall clock and prints `cpu=` in place of `wall=`.

## audio-tap-cost

What the sound-register write log and sample capture cost the run loop: the `gbcli-sameboy` bench in modes `plain`, `apu-log`, `audio` and `apu-log+audio`, in interleaved rounds; prints each mode's best and median speed and its cost against `plain`. Given a base revision from before the features, it also builds that revision's bench from `git archive` and runs its `plain` and `tracking` modes in the same rounds, which is the cost when off. Figures are in `project/2026-11-06-design.md` (§9, the leaf aDOL0w update).

```sh
scripts/bench/audio-tap-cost/run.sh tests/corpus/fetched/blargg/cpu_instrs/cpu_instrs.gb 6 3600 main   # args: rounds (5), frames (3600), base revision (none)
```

## profile-overhead

What the cycle profiler costs: the `gbcli-engine` example `profile_bench` in modes `plain` (a run with no sink), `profile` (`Machine::profile`) and `frames` (`Machine::frame_budget`), in interleaved rounds; prints each mode's best and median speed and its time against `plain`'s best. Figures are in `project/2026-11-06-design.md` (§11, the leaf 6eLWSu update).

```sh
scripts/bench/profile-overhead/run.sh tests/corpus/fetched/blargg/cpu_instrs/cpu_instrs.gb 5 3600   # args: rounds (5), frames (3600)
```

## layer-patch-cost

What gbcli's SameBoy patches (`crates/sameboy-sys/patches/`) cost with the layer buffer detached: `sb_bench.c` in CPU seconds, built with `build.rs`'s flags against the vendored tree and against the same tree with the patches reverse-applied (no clone), run in simultaneously-started pairs; prints each pair's cost and the median. Figures are in `project/2026-11-06-sameboy-vendoring.md` (Throughput).

```sh
scripts/bench/layer-patch-cost/run.sh tests/corpus/fetched/blargg/cpu_instrs/cpu_instrs.gb   # optional args: pairs (8), frames (7200)
```

## gbcli-sameboy (the backend, through the core trait)

Throughput of the SameBoy backend on a ROM after a 100-frame boot through the built-in DMG boot ROM, then the mean and worst time to restore a checkpoint and run one frame of ticks (1,000 times). Mode `plain` (write tracking off, no filters), `tracking` (never-written tracking on, the default), `filters` (1,000 armed filters that never match), `layers` (per-pixel layer capture on), `apu-log` (the sound-register write log on), `audio` (sample capture on) or `apu-log+audio`. Figures are in `project/2026-11-06-design.md` (§9), `project/2026-11-06-sameboy-vendoring.md` and the `job` notes of leaves jQ3ATQ, m65CaC and n2WX5r. On a shared machine, take the best of several interleaved runs.

```sh
python3 -I scripts/corpus/fetch.py --only blargg
cargo run --release -p gbcli-sameboy --example bench -- \
    tests/corpus/fetched/blargg/cpu_instrs/cpu_instrs.gb 3600 1 plain   # args: rom, frames, render, mode
```

## cgo-overhead

Cost of a Go→C call and a C→Go callback (10M iterations each).

```sh
cd scripts/bench/cgo-overhead && GOTELEMETRY=off go run .
```

## rust-build

Rust build times for a crate that compiles SameBoy's `Core/*.c` via the `cc` crate and depends on serde, serde_json and png. Clean, then incremental after touching `main.rs`.

```sh
cd scripts/bench/rust-build && SAMEBOY=/path/to/SameBoy cargo build   # time clean and incremental
```

## workspace-build

gbcli's own debug build times: a clean `cargo build --workspace`, then incremental builds after touching the binary crate (`crates/gbcli/src/main.rs`) and the engine crate (`crates/engine/src/lib.rs`); prints the median of N runs for each incremental case. Uses `target/bench-workspace-build`, so the normal build cache is untouched.

Figures (debug; Apple M1, 8 CPUs, rustc 1.99.1, Apple clang 21; `scripts/bench/workspace-build/run.sh 3`):

- 2026-11-06, after stage 3, SameBoy at `-O3`: clean 13.2 s; incremental 0.35 s after touching the binary crate and 1.11 s after touching the engine crate (medians of 3). The stage-1 figures were clean 7.5 s and incremental 0.32/0.31 s; the added crates and the server's dependency edge widened the rebuild.
- 2026-11-06, dev profile at `opt-level = 1`, under host load: clean 16.4 s, incremental 0.24 s (binary) and 0.87 s (engine), against 10.7, 0.26 and 0.94 s at opt-level 0 in the same sitting.

Re-run it when the crates grow and add a dated line here.

```sh
scripts/bench/workspace-build/run.sh      # optional arg: runs per incremental case (default 5)
```
