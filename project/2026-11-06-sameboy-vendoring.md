# Vendoring SameBoy: commit, flags, feature switches

2026-11-06, scaffold leaf `eeTyfo`. How SameBoy enters the build, and why.

## The commit

`crates/sameboy-sys/SameBoy/` holds SameBoy's `Core/`, the boot ROM sources from `BootROMs/` (`*.asm`, `*.inc`, `pb12.c`, `SameBoyLogo.png`), `LICENSE` (Expat) and `version.mk`, at **`ea667ad79aa28409f034b1fd6da7304e1844750d`** (2026-11-02, "Merge branch 'master' of github.com:LIJI32/SameBoy"; version 1.0.5). The commit is also in `SameBoy/COMMIT`, which `sameboy_sys::SAMEBOY_COMMIT` reads, so the checkpoint envelope can name its core commit from the one place.

> **Correction, 2026-11-06 (leaf n2WX5r): `Core/` is no longer unmodified.** It carries one observation-only patch of ours, [the layer-identity patch](#gbclis-layer-identity-patch), applied by `scripts/vendor-sameboy.sh`. Everything else is still upstream's.

**Why this commit:** it was the latest `master` when the scaffold was built (a `git clone --depth 1` on 2026-11-06 resolved to it), and it is the commit the backend evaluation read and measured, so every line reference and measured figure in [2026-11-06-backend-evaluation.md](2026-11-06-backend-evaluation.md) applies to the vendored code as is.

**To move it:** `scripts/vendor-sameboy.sh [commit]` replaces the vendored tree, re-applies `crates/sameboy-sys/patches/*.patch`, and rewrites `COMMIT`; then `scripts/build-roms.sh` rebuilds the boot ROMs, and this doc (or a new dated one) records the new commit and why. If the patch's `git apply --check` fails, the new commit moved the code it touches: rebase the patch, don't drop it.

## gbcli's layer-identity patch

`crates/sameboy-sys/patches/layer-identity.patch` (leaf n2WX5r) makes SameBoy record **which layer drew each pixel**: background, window, or an OAM object and its index. SameBoy decides the winner of every pixel at one mixing point and then discards it; the patch writes that decision to an optional side buffer instead of inferring it later. Replaying with rendering disabled or re-rendering from VRAM/OAM would change object fetch (and therefore mode-3 timing) or be wrong under mid-line raster changes, so neither is used.

What it touches:

- **`Core/display.h`** — `GB_fifo_item_t` gains `window` (the BG FIFO: the row was pushed from the window map) and `object` (the OAM FIFO: the OAM index of the object that won the pixel). That grows the item from 4 to 6 bytes; the two FIFOs sit in SameBoy's saved `video` section, so the save state grows by 32 bytes (DMG cpu_instrs: 50,712 → 50,744). A checkpoint written by a build without the patch is therefore not compatible, which the envelope's core-commit field already rules out.

  > **Correction, 2026-11-06 (review, `project/2026-11-06-review-fixes.md`): it does not.** The patch changes no version: `blob::VERSION`, SameBoy's `STRUCT_VERSION` and the recorded commit are all unchanged. SameBoy's `read_section` accepts the shorter section and shifts every later field, so a pre-patch checkpoint or boot cache loads silently and diverges within frames (run by the reviewer). Leaf `46uWtG` adds a refusal and keys the boot cache on core identity.
- **`Core/gb.h`** — a `layer_buffer` pointer next to `screen`, inside the **unsaved** section. It is never written to a save state and never read from one; the caller owns the memory.
- **`Core/display.c`** — the two FIFO push functions carry the tags; `layer_store` writes `0` for background, `1` for the window and `2 + n` for OAM object `n` at `x + y * 160` when the pointer is non-NULL. It is called at both mixing points: `render_pixel_if_possible` (the per-pixel path) and `render_line` (the batched path), including that function's BG-disabled branch, and the object index is also recorded in `render_line`'s local object buffer. The window flag comes from `gb->wx_triggered` on the BG FIFO push. Nothing reads the buffer, and no pixel, register or state depends on it. Two recovery paths — the mode-3 abort (`display9`) and the PPU/LCD desync fill — write a colour without visiting the mixing point; those pixels keep the buffer's background tag, and they only happen when a ROM desynchronises the PPU.
- **No tag without the buffer.** With `layer_buffer == NULL` the only cost is one predictable branch and one pointer load per drawn pixel; there is no allocation, no second pass and no change to the pixel path.
- **Shim** (`shim/gbcli_shim.c`, ours, not part of the patch): `gbcli_set_layer_buffer(gb, buffer)` attaches or detaches the buffer; `gbcli_frame_scrubbed(gb)` reports SameBoy's blank, substituted frame after the LCD is turned on (`frame_skip_state == GB_FRAMESKIP_LCD_TURNED_ON`), so no layer map is published for pixels that were never rendered. The FFI declarations are in `crates/sameboy-sys/src/lib.rs`.
- **Rust** (`crates/sameboy/src/lib.rs`): `Core::set_pixel_layers(on)` turns capture on, off by default, and `Core::pixel_layers()` returns the last complete frame's map; the tags are copied at vblank into `Hooks::frame`, like the pixels, so the map always describes the frame `last_complete_frame` publishes. `Feature::PixelLayers` is `Yes` on DMG/MGB/CGB and `No` on SGB, whose border and mask modes are composited in `sgb.c` after the PPU's mixing point.

The patch is separable and small enough to offer upstream; the FIFO growth is the only part that changes SameBoy's state layout.

### Throughput: buffer off versus on

Measured 2026-11-06 on an Apple M1 (8 CPUs, Apple clang 21) shared with other agents' builds (load average 23–42 during the runs). The machine's same-binary run-to-run spread was about ±2%, so the off cost is reported as the median of paired runs that started simultaneously.

**Off, against an unpatched SameBoy.** Both binaries are `scripts/bench/sameboy-throughput/sb_bench.c` compiled with `build.rs`'s flags, one against a pristine clone of `ea667ad` and one against `crates/sameboy-sys/SameBoy`; a copy of the harness whose `now()` reads `CLOCK_PROCESS_CPUTIME_ID` gives CPU seconds:

```sh
# For each tree: then run sb_bench against cpu_instrs for 7,200 frames, rendering on.
cd <tree>/Core
cc -O3 -std=gnu11 -D_GNU_SOURCE -DGB_INTERNAL -DNDEBUG -DGB_VERSION='"1.0.5"' \
   -DGB_COPYRIGHT_YEAR='"2026"' -DGB_DISABLE_DEBUGGER -DGB_DISABLE_REWIND \
   -DGB_DISABLE_CHEAT_SEARCH -I<tree> -o /tmp/sb_bench <sb_bench.c> \
   $(print -l *.c | grep -vE '^(debugger|sm83_disassembler|symbol_hash|rewind|cheat_search)\.c$') -lm
```

Eight simultaneously-started, interleaved pairs at 7,200 frames each: **median +0.44%**, range −0.94% to +1.85%. That is at the measurement floor; structurally the off path is one well-predicted branch per pixel, and no pixel or state value changes.

**On, through the backend.** After a 100-frame boot, `cargo run --release -p gbcli-sameboy --example bench -- tests/corpus/fetched/blargg/cpu_instrs/cpu_instrs.gb 3600 1 <mode>`, where `plain` leaves capture off and `layers` turns it on: best of eight interleaved runs each gave `plain` 23.7× and `layers` 22.7× realtime; five paired runs put the on cost at **median 3.1%** (range 1.2–6.8%). The on path writes one byte per drawn pixel (23,040 bytes a frame) and takes the branch instead of predicting it. These figures were taken under load 25–45 and are not comparable with the stage-1 `plain` 19.2× recorded at load 3–6.


## How it is compiled

SameBoy's `make lib` is not used: on macOS its dylib exports no symbols and, without the `cppp` tool, its installed headers come out empty while make exits 0 (evaluation ▸ Build and linking). Instead `crates/sameboy-sys/build.rs` compiles `Core/*.c` with the `cc` crate:

- `-std=gnu11 -O2 -D_GNU_SOURCE -DGB_INTERNAL -DGB_VERSION="1.0.5" -DGB_COPYRIGHT_YEAR="2026"` — the evaluation's flags (since stage 1, `-O3 -DNDEBUG` instead of `-O2`; see the update below). `GB_VERSION` is read from `version.mk` and `GB_COPYRIGHT_YEAR` from `LICENSE`, as SameBoy's Makefile derives them, so they follow a re-vendor. `-O2` applies to debug builds too, since tests run the emulator for real.

  > **Update, 2026-11-06 (stage 1, leaf jQ3ATQ): now `-O3 -DNDEBUG`, plus our shim.** The evaluation's 21.8× baseline was SameBoy's own `make lib CONF=release`, which compiles with `-O3 -ffast-math -DNDEBUG -flto` (SameBoy Makefile:362–380 at `ea667ad`), not the `-O2` with asserts on that this build used. Linked against our `-O2` library, the unchanged `scripts/bench/sameboy-throughput/sb_bench.c` loop ran 17.7–18.2× (three runs; `cc -O2 -std=gnu11 -DGB_DISABLE_DEBUGGER -DGB_DISABLE_REWIND -DGB_DISABLE_CHEAT_SEARCH -Icrates/sameboy-sys/SameBoy sb_bench.c <target>/release/build/sameboy-sys-*/out/libsameboy.a -lm`, then `sb_bench cpu_instrs.gb 3600 1`); with `-O3 -DNDEBUG` it ran 20.7–21.7×. So `build.rs` now passes `-O3 -DNDEBUG`. Left out: `-ffast-math` (floating-point results would depend on the optimizer, a needless risk to determinism) and LTO (not available through a cc-built static library without cross-language LTO). `build.rs` also compiles `crates/sameboy-sys/shim/gbcli_shim.c` with the same flags: tiny C accessors over SameBoy internals (IME, halt, the bank mapped at an address, side-effect-free peeks, the pre-step illegal-opcode check, the layer buffer and the LCD-turn-on scrubbed-frame flag), so the struct layout is never mirrored in Rust.
- `-DGB_DISABLE_DEBUGGER -DGB_DISABLE_REWIND -DGB_DISABLE_CHEAT_SEARCH`, leaving out `debugger.c`, `sm83_disassembler.c`, `symbol_hash.c`, `rewind.c` and `cheat_search.c` (the files SameBoy's Makefile filters for the same switches). 16 of the 21 `Core/*.c` files are compiled. Why: see below.
- SameBoy's own warning set (Makefile `WARNINGS`: `-Wall` plus its `-Wno-…` exceptions, and `-Wno-extra` to cancel the `-Wextra` that cc adds) without `-Werror`, so a newer compiler's warning in vendored code can't break our build. At this commit the build is warning-free (Apple clang 21, observed with `cargo build` after `cargo clean -p sameboy-sys`).
- No SDL, no `cppp`, no RGBDS, no make. `cargo build` on a fresh copy of the tree with only `cargo`, `rustc`, `rustdoc` and `/usr/bin:/bin` on `PATH` succeeded, and `cargo test` passed there (2026-11-06).

FFI declarations are hand-written in `crates/sameboy-sys/src/lib.rs` (no bindgen), added as callers need them.

## Debugger and rewind: compiled out

**The suspected mismatch is real.** `Core/debugger.c` guards its rewind code with `#ifndef DISABLE_REWIND` (lines 2178, 2245, 2700), while the Makefile defines `GB_DISABLE_REWIND` and every other file tests that name. Compiling `debugger.c` with the build's define alone fails:

```
cc -std=gnu11 -O2 -D_GNU_SOURCE -DGB_INTERNAL -DGB_VERSION='"1.0.5"' -DGB_COPYRIGHT_YEAR='"2026"' \
   -Wno-multichar -DGB_DISABLE_REWIND -c Core/debugger.c
Core/debugger.c:2191:16: error: no member named 'rewind_sequences' in 'struct GB_gameboy_s'
Core/debugger.c:2209:9: error: call to undeclared function 'GB_rewind_pop'; …
(17 errors; exit 1)
```

It compiles with `-DGB_DISABLE_REWIND -DDISABLE_REWIND`, and with neither. So SameBoy's `make lib DISABLE_REWIND=1` (debugger on) does not build at this commit — an upstream bug worth reporting.

**Decision: compile out both the debugger and rewind**, which also sidesteps the bug, since `debugger.c` is not compiled.

- *Debugger.* We never drive SameBoy's text debugger (plan, fidelity rule 2); ours is built on the public hooks. Compiling it out enforces that rule in the build rather than by discipline, and removes the default input callbacks that read stdin (`gb.c:50-105`, only present with the debugger) — exactly what a headless server must never do. What we lose: `debugger_ticks` (we keep our own emulated-time counter anyway), `GB_cpu_disassemble` (text through the log callback, not the structured disassembly we want), SameBoy's symbol loading (we parse RGBDS `.sym` files ourselves), the `ld b, b` software breakpoint (ours comes from the execution hook), and cheat search (implied by `GB_DISABLE_DEBUGGER`). The memory read/write, execution and LCD callbacks the plan relies on are not behind the switch.
- *Rewind.* Our checkpoints are our own (envelope around `GB_save_state`); SameBoy's rewind buffer is unused, and it is inert unless `GB_set_rewind_length` is called. Compiling it out drops `rewind.c` and the per-frame push hook.
- *Kept:* cheats (`GB_DISABLE_CHEATS` not set; GameShark/Game Genie codes may be useful) and timekeeping (`GB_DISABLE_TIMEKEEPING` not set; headless runs use turbo with no cap, per the evaluation).

Both are one-line switches in `build.rs`: a two-way door. Turning the debugger back on with rewind still off needs `-DDISABLE_REWIND` as well.

## Boot ROMs

`scripts/build-roms.sh` builds SameBoy's open-source boot ROMs (Expat) from the vendored `.asm` with RGBDS (v1.0.4 here): it compiles `pb12.c`, converts the logo with `rgbgfx -Z -u -c embedded`, compresses it, then `rgbasm` + `rgblink -x` each of `dmg_boot mgb_boot cgb_boot cgb0_boot cgb_boot_fast agb_boot sgb_boot sgb2_boot`. The binaries and a copy of the license are committed in `crates/sameboy-sys/bootroms/`, and `sameboy_sys::boot_roms` embeds them, so a normal build needs no RGBDS. Rebuilding gives byte-identical output (checked with `shasum -c` across two runs). **Never commit Nintendo boot ROM dumps.**

SameBoy's DMG boot ROM hands over to the cartridge after 80–85 frames (the smoke ROM's first write appears between those frame counts; measured by bisecting the smoke test's frame budget, 2026-11-06).

The same script assembles gbcli's own test ROMs from `crates/sameboy-sys/tests/roms/*.asm` (today just `smoke.asm`) with `rgbasm`, `rgblink` and `rgbfix -v -p 0`. Since stage 1 it also assembles `crates/sameboy/tests/roms/*.asm` (the backend's test ROMs); a name ending in `_boot` (`quick_boot.asm`, a four-instruction boot ROM for fast tests) is linked unpadded with `rgblink -x` into a `.bin` instead.

## Headless smoke test

`crates/sameboy-sys/tests/smoke.rs` boots `smoke.gb` through the DMG boot ROM, runs the cartridge for 60 frames (counted in `GB_run` ticks, 140,448 per frame), and asserts that nothing reached file descriptor 1 (it redirects fd 1 to a file, so C `printf` output is caught, not just Rust's). The ROM writes once to echo RAM at `$F000`, which SameBoy logs; the test routes that line through its log callback and asserts it arrived. Without the log callback, the test fails with `SameBoy wrote to stdout: "Wrote 42 to f000 (RAM Mirror)\n"` (checked by deleting the `GB_set_log_callback` call). Note that SameBoy logs echo-RAM writes only in `$F000-$FDFF`; `$E000-$EFFF` goes through its plain RAM path silently.
