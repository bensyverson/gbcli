# Benchmarks

Re-runnable measurements behind figures in `project/2026-11-06-backend-evaluation.md`.

## sameboy-throughput

Headless SameBoy (DMG_B, turbo uncapped, no boot ROM via the FF50 write) running a ROM for N frames' worth of 8 MHz ticks; also times in-memory state save/load (mean of 10,000).

```sh
# SAMEBOY = a SameBoy checkout at the pinned commit; build its static library first:
make -C "$SAMEBOY" lib CONF=release
cc -O2 -std=gnu11 -I"$SAMEBOY" -o sb_bench scripts/bench/sameboy-throughput/sb_bench.c \
   "$SAMEBOY/build/lib/libsameboy.a" -lm
./sb_bench path/to/cpu_instrs.gb 3600 1   # args: rom, frames, render(0|1)
```

## cgo-overhead

Cost of a Go→C call and a C→Go callback (10M iterations each).

```sh
cd scripts/bench/cgo-overhead && GOTELEMETRY=off go run .
```
