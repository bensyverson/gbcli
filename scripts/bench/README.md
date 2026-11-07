# Benchmarks

Re-runnable measurements behind figures in `project/2026-11-06-backend-evaluation.md` and, for `workspace-build`, the head of `AGENTS.md`.

## test-times.py

The warm test suite, timed per test binary (wall and CPU) and then per test inside any binary over a threshold, each test run alone. Builds first, so compile time is excluded. The warm suite must stay under 30 s; figures and the policy are in `project/2026-11-06-test-suite-speed.md`. Wall time on a loaded machine is inflated, so the script prints the load average; compare CPU seconds across runs.

```sh
python3 -I scripts/bench/test-times.py                      # per-test breakdown for binaries over 2 s
python3 -I scripts/bench/test-times.py --per-test-over 1.0
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

## gbcli-sameboy (the backend, through the core trait)

Throughput of the SameBoy backend on a ROM after a 100-frame boot through the built-in DMG boot ROM, then the mean and worst time to restore a checkpoint and run one frame of ticks (1,000 times). Mode `plain` (write tracking off, no filters), `tracking` (never-written tracking on, the default), `filters` (1,000 armed filters that never match) or `layers` (per-pixel layer capture on). Figures are in `project/2026-11-06-design.md` (§9), `project/2026-11-06-sameboy-vendoring.md` and the `job` notes of leaves jQ3ATQ, m65CaC and n2WX5r. On a shared machine, take the best of several interleaved runs.

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

gbcli's own debug build times: a clean `cargo build --workspace`, then incremental builds after touching the binary crate (`crates/gbcli/src/main.rs`) and the engine crate (`crates/engine/src/lib.rs`); prints the median of N runs for each incremental case. Uses `target/bench-workspace-build`, so the normal build cache is untouched. Figures are recorded in the head of `AGENTS.md`.

```sh
scripts/bench/workspace-build/run.sh      # optional arg: runs per incremental case (default 5)
```
