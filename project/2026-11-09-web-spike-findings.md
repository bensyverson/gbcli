# Findings: web spike 1, gbcli's handler built for wasm32

2026-11-09, nineteenth session, leaf 3zNDPj (Stage 12, session 1 of 2). The plan and Ben's rulings are in [2026-11-09-web-spike-plan.md](2026-11-09-web-spike-plan.md); the web ruling is in [2026-11-09-web-target.md](2026-11-09-web-target.md). This doc rides on the `spike/web` branch. Spike 2 adds the page, the timing figures and the decisions.

## Headline answers

1. **SameBoy, the engine and the whole protocol handler build for `wasm32-wasip1`.** This is `gbcli-server` itself, with `gbcli-build`, `gbcli-bundle`, `gbcli-view` and `gbcli-audio`, built with wasi-sdk 34. It needed no change to SameBoy's C or to the shim, and the C produced no warnings. Nothing failed to compile: Rust's wasip1 `std` compiles `std::process`, `std::thread` and file locks, and they fail at run time instead. That's why the cuts below were found by running, not by compiling.
2. **Runs are bit-identical to native.** Node drives the `.wasm` handler with a `run` request: N frames, a key timeline, reads, a shot and a WAV. On 7 runs (5 ROMs on DMG, 2 on GBC), the wasm and native builds gave the same response line, the same PNG SHA-256 and the same WAV SHA-256. For `layers.gb`, the real native `gbcli run` gives the same PNG hash. In a separate check, all 32,768 RGB555 colors convert identically on a GBC. That conversion runs libm's `pow`, the one place wasi-libc's libm could round differently from Apple's.
3. **The handler needed four run-time cuts, each a `cfg(target_family = "wasm")`:** `std::process::id()` (it panics) in `gbcli-bundle` and `gbcli-build`, `File::try_lock` (unsupported) in the session lock, and the watchdog thread (spawning one is unsupported). One known abort remains: `std::env::temp_dir()` panics on WASI, and `assets` uses it. Every other op probed answers (table below).
4. **Size:** 6,125,792 bytes raw (release), 4,948,142 stripped, 1,442,252 stripped and gzip -9. That is the whole handler, including the build, font, music, pixel and view crates.
5. **The bespoke-core backlog entry's web condition doesn't hold on build or correctness:** SameBoy builds and runs exactly under wasm32. "Acceptably" also means speed, which spike 2 measures.

## Toolchain, and why

**wasi-sdk 34 (clang 23.1.0-wasi-sdk) with Rust's `wasm32-wasip1` target, run under Node 26.13.0's built-in `node:wasi`.**

- **Why wasi-sdk first:** SameBoy's C includes `stdio.h`, `time.h`, `unistd.h` and `sys/select.h`, and calls `malloc`, `fopen`, `printf`, `vasprintf`, `time` and libm. wasi-libc has all of it, and Rust's wasip1 target links wasi-libc already, so the cc crate's C and Rust's `std` share one libc and one allocator. Emscripten also has the libc, but Rust's `wasm32-unknown-emscripten` target is lower-tier and brings Emscripten's JS runtime. `wasm32-unknown-unknown` has no libc at all (tried below).
- **The price:** the module imports 23 `wasi_snapshot_preview1` functions (files, clocks, `random_get`, `poll_oneoff`, `proc_exit`). Node provides them. A browser needs a WASI shim with an in-memory filesystem (for example `@bjorn3/browser_wasi_shim`); that's spike 2. wasm-bindgen targets `wasm32-unknown-unknown`, not wasip1, so the spike's JS boundary is hand-written C-ABI exports with JSON in and out, not wasm-bindgen. wasm-bindgen-cli wasn't installed.
- **`wasm32-unknown-unknown`, tried:** `cargo build -p sameboy-sys --target wasm32-unknown-unknown` with wasi-sdk's clang fails at once: `SameBoy/Core/display.c:2:10: fatal error: 'stdlib.h' file not found`. Getting there means writing a small libc for exactly the symbols SameBoy imports, listed below. That's feasible (they're few, and the file and clock ones are on paths gbcli never calls), but it's work wasi-sdk makes unnecessary.

### Installs (Ben's ruling allowed these)

```
rustup target add wasm32-wasip1 wasm32-unknown-unknown
curl -sSL -o ~/.local/wasi/wasi-sdk-34.0-arm64-macos.tar.gz \
  https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-34/wasi-sdk-34.0-arm64-macos.tar.gz
tar -xzf ~/.local/wasi/wasi-sdk-34.0-arm64-macos.tar.gz -C ~/.local/wasi
```

### Build and check

`scripts/web/build-spike.sh` does all of this. By hand, from the repo root:

```
CC_wasm32_wasip1=$HOME/.local/wasi/wasi-sdk-34.0-arm64-macos/bin/clang \
AR_wasm32_wasip1=$HOME/.local/wasi/wasi-sdk-34.0-arm64-macos/bin/llvm-ar \
  cargo build -p gbcli-web --target wasm32-wasip1 --release
cargo build -p gbcli-web --release --examples          # the native references
node crates/web/node/check.mjs target/wasm32-wasip1/release/gbcli_web.wasm \
  crates/sameboy/tests/roms/layers.gb target/release/examples/native_ref 600 [MODEL]
node crates/web/node/colors.mjs target/wasm32-wasip1/release/gbcli_web.wasm \
  crates/engine/tests/roms/gbc_palette.gb target/release/examples/native_colors
node crates/web/node/probe.mjs target/wasm32-wasip1/release/gbcli_web.wasm crates/sameboy/tests/roms/layers.gb
```

The cc crate needed nothing beyond `CC_`/`AR_` for the target: `sameboy-sys/build.rs` is unchanged, and so are the vendored patches and the shim.

## What the spike crate is

`crates/web` (`gbcli-web`) is a `cdylib` + `rlib`. It does what the CLI's `start_handler` does: `Bundle::open_or_create`, `open_session` (creating the session from the ROM), `Handler::new`, `set_host`. It answers request lines through `gbcli_server::serve`, exactly as `gbcli serve` does. So this is the protocol handler, not the engine with the handler bypassed; nothing in the adapters was too tangled to keep. Its exports:

- `gbw_alloc` and `gbw_free` manage buffers.
- `gbw_put_file(path, bytes)`: the ROM goes in as bytes, written into the WASI filesystem.
- `gbw_open({"dir","session","rom","model"?})` opens the handler.
- `gbw_request(line) -> line` answers one protocol request.
- `gbw_get_file(path) -> bytes`: the shot's PNG and the WAV come out as bytes.
- `gbw_color_table_sha256(rom)` hashes the color table, through the engine directly.

Results come back as one packed `u64` (`ptr << 32 | len`). JavaScript sees it as a *signed* BigInt, so `check.mjs` applies `BigInt.asUintN(64, …)`.

The session bundle stays the filesystem, as WASI exposes it: Node preopens a temp directory as `/work`, and a browser shim would hold the same tree in memory. The handler, the bundle store and every op run unchanged on it, which is what makes this spike cheap. A real web build should still replace that with a store trait (below), not lean on a WASI filesystem forever.

The frame: `run` with `shot` writes a PNG into the WASI filesystem, and JS reads it back with `gbw_get_file`. The state is the protocol's own `Response` (stop record, reads, shot and audio results) as its JSON line.

## The determinism check

`crates/web/node/check.mjs` runs one `run` request through the wasm handler under Node's WASI, and the same request through the same `Spike` calls built natively (`crates/web/examples/native_ref.rs`). The request: N frames, keys `START@10 -START@12 A@60 -A@70 RIGHT@100 -RIGHT@160`, reads `a pc [$FF40]`, a 1x shot and a mixed WAV. The check requires the response lines to be byte-identical and the PNG and WAV SHA-256 to match.

| ROM | frames | model | PNG SHA-256 (prefix) | result |
|---|---|---|---|---|
| `crates/sameboy/tests/roms/layers.gb` | 600 | DMG-B (default) | `7dda3185` | identical |
| `crates/sameboy/tests/roms/mode_sweep_win.gb` | 3000 | DMG-B | `14719d14` | identical |
| `crates/sameboy/tests/roms/banked.gb` | 1200 | DMG-B | `e766136b` | identical |
| `crates/audio/tests/roms/tune.gb` | 600 | DMG-B | `3c0bf5b4` (WAV `87388fe6`) | identical |
| `crates/engine/tests/roms/joypad.gb` | 600 | DMG-B | `3c0bf5b4` | identical |
| `crates/engine/tests/roms/gbc_palette.gb` | 600 | gbc (CGB-E) | `9070379f` | identical |
| `crates/sameboy/tests/roms/layers_cgb.gb` | 1200 | gbc | `7f4d0333` | identical |

Reproduce with `scripts/web/build-spike.sh`. `tune.gb` and `joypad.gb` show the same blank screen, so their PNG proves little; their WAVs differ from each other and match across builds. The native CLI agrees: `gbcli run rom.gb --session web --frames 600 --keys 'START@10 -START@12 A@60 -A@70 RIGHT@100 -RIGHT@160' --shot shot.png --scale 1` on `layers.gb`, in a project outside the repo, writes a PNG whose `shasum -a 256` is `7dda318511b88c2ee5da8f85b019bf2d1144b3797ff1b1888a7e45ef2e90439f`, the same as the wasm build's.

The color table (`node/colors.mjs`) hashes SameBoy's `GB_convert_rgb15` for all 32,768 colors on a CGB-E under default color correction, which calls `pow`. Wasm and native both give `98f5aa28…` (the full hash is in the check's output). The APU's band-limited synthesis table uses `sin` and `cos`, and the WAVs match.

## What SameBoy's C needed from libc, and how it was handled

`llvm-nm -u` on the wasip1 `libsameboy.a` lists every libc symbol SameBoy and the shim import:

| Need | Symbols | Where | Handled |
|---|---|---|---|
| Heap | `malloc` `calloc` `realloc` `free` | everywhere | wasi-libc; one allocator shared with Rust |
| File I/O | `fopen` `fread` `fwrite` `fseek` `ftell` `fclose` | `GB_load_rom`/boot ROM/battery/state *by path*, cheats, audio recording (`apu.c`) | Linked but never called: gbcli loads everything from buffers (`GB_load_boot_rom_from_buffer`, ROM and states from memory) |
| Logs | `printf` `vasprintf` | `gb.c` default log | gbcli sets a log callback (`hooks::on_log`) |
| Text | `sscanf` `strlen` `strncpy` `strerror` `errno` | cheats, error paths | wasi-libc |
| Clocks | `time` `gettimeofday` `nanosleep` `localtime` `mktime` | RTC stamps, `GB_timing_sync`, random seed, WorkBoy | Never on gbcli's path: turbo is uncapped (`GB_set_turbo_mode(true, true)`, cap 0), so timing sync never sleeps; the shim pins `last_rtc_second`; RTC runs in accurate (emulated) mode; the RNG is seeded. They still link, and Node's `clock_time_get` and `poll_oneoff` back them |
| RNG | `rand` | `apu.c` `interference()`, only while an interference volume is set | Never called: gbcli never calls `GB_set_interference_volume`, so it stays 0. Were it on, wasm and native would differ, since each libc's unseeded `rand()` gives its own sequence |
| libm | `pow` `round` `sin` `cos` `exp2` | color correction, APU, SGB, joypad | wasi-libc's libm (musl's); identical to native on everything checked |

- **Boot ROMs** are embedded with `include_bytes!` (`crates/sameboy-sys/src/lib.rs`, `bootroms/*.bin`) and loaded from a buffer, so no file is read.
- **`setjmp`/`longjmp`:** none. **Threads:** none in the C.
- `memcpy` and `memset` don't appear as imports: clang 23 lowers them to wasm bulk-memory instructions.
- The `random_get` import comes from Rust's `std` (HashMap's `RandomState` seeds), not from SameBoy.
- A minimal libc for `wasm32-unknown-unknown` would need exactly the table above. Of it, only the heap, the text functions and libm are on gbcli's path; the file and clock functions could be stubs that fail.

## Every adapter cut, and what a real web build needs instead

Each cut is marked `WEB SPIKE CUT` in the source and gated on `cfg(target_family = "wasm")`, so native builds are unchanged (`cargo test --workspace` passes).

| Cut | Where | What failed on wasm32 | Spike's cut | What a real web build needs |
|---|---|---|---|---|
| Process id | `gbcli-bundle` `io::process_tag` (temp names in `atomic_write`, `new_incarnation`), used by `gbcli-server` `handle/shot.rs`; `gbcli-build` `process_tag` (`doctor.rs` probe file, `project/staging.rs`, `assets.rs` scratch) | `std::process::id()` panics: "no pids on this platform", aborting the module | returns 0 on wasm | Nothing in a store that isn't files: temp-and-rename and per-process names are filesystem idioms. An in-memory or browser-storage store replaces them; an incarnation needs only a counter or a random value |
| Session lock | `gbcli-bundle` `lock.rs` `acquire` | `File::try_lock`: "try_lock() not supported" (`io_failed` on `open`) | skips the flock and returns the lock over the opened file | A store trait whose lock is the store's own: none for a single in-memory store; a Web Lock (`navigator.locks`) if two tabs share browser storage. The lock file's mtime, which pruning reads as last use, becomes a field |
| Watchdog thread | `gbcli-server` `watchdog.rs` `Watchdog::new` | `thread::Builder::spawn`: "Not supported", which `expect` turned into a panic | no thread; the guard never fires, though a zero allowance still interrupts | An interrupt the host controls, not a thread: emulation already polls an `AtomicBool`. A web host can set it from a JS callback between frame batches, or the handler can take a frame or tick budget per call so the page's event loop stays free (`run` already has budgets). The wall-clock allowance becomes the host's concern |
| Host | `Handler::new` calls `Host::from_process()` | Nothing failed: wasi-libc emulates a cwd of `/`, and `PATH` is unset | `set_host(Host { cwd: dir, path: None })` right after | A `Handler` built with an explicit host and store (web-target doc ▸ When the work starts); `from_process` belongs in the CLI |
| RGBDS | `gbcli-build` `toolchain.rs`, `project/tool.rs` (`Command`, a reader thread, `kill`) | `doctor` and `build` answer `toolchain_missing: rgbasm is not on PATH` because `PATH` is unset; with a `PATH`, `Command::spawn` would fail as unsupported. The message is wrong for wasm: the cause is "no subprocesses here" | none (out of scope; the RGBDS leaf) | The one seam the web ruling names: the tools behind a trait that takes the project's sources as data and returns ROM, `.sym`, `.map` and diagnostics. `doctor`'s git and `PATH` checks need a web answer too |
| Temp directory | `gbcli-build` `assets.rs` `Scratch::new` (`std::env::temp_dir()`), and the transient PNG it writes for rgbgfx | Panics on WASI: "not supported by WASI yet" (`std/src/sys/paths/wasi.rs:44`), aborting the module. `assets` is the one probed op that does this | none: it's on the rgbgfx path, a subprocess anyway | Scratch space from the store or the host, never `temp_dir()`; and the image conversion's rgbgfx call behind the same build seam |
| Panic guard | `gbcli-server` `serve.rs` (`catch_unwind` around each request) | wasm32 panics abort (`panic = "abort"`), so one panicking request kills the module and every session's warm machine with it; the guard is dead code there | none | Either no panics on reachable paths (the cuts above were all panics), or a host that re-instantiates the module and reloads the store after an abort. Unwinding on wasm needs the exception-handling proposal and a rebuilt `std` |

### The ops probed through one wasm handler

`crates/web/node/probe.mjs` sends these to one handler over `layers.gb`, in order:

- **Answered normally:** version, run, show, checkpoint, run, restore, undo, checkpoints, events, step, read, write, shot, view (tiles), view (memory: refused with `uninitialized_read`, a correct answer), measure, sessions, gaps, opcode, new (it scaffolds the starter into the WASI filesystem), power_cycle, sav (refused, correctly: `layers.gb` has no battery), prune.
- **Refused with `toolchain_missing`:** doctor, build.
- **Aborted the module:** assets.

## Not covered in this session

- No browser: the page, the WASI shim choice and its filesystem, and all timing and start-up figures are spike 2's.
- Not probed: `equiv`, flash carts, battery carts (`sav` on a battery ROM), the font, pixels, song and image ops other than `assets`. The font, pixels and song conversions are data in and data out and should run; their file I/O goes through the same WASI filesystem.
- The spike crate links the whole server, build crate included. A web build that drops `gbcli-build`'s subprocess half would be smaller. Size by crate wasn't measured; `twiggy` or `wasm-opt` weren't installed.
