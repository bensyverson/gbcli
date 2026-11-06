# Vendoring SameBoy: commit, flags, feature switches

2026-11-06, scaffold leaf `eeTyfo`. How SameBoy enters the build, and why.

## The commit

`crates/sameboy-sys/SameBoy/` holds SameBoy's `Core/` (all of it, unmodified), the boot ROM sources from `BootROMs/` (`*.asm`, `*.inc`, `pb12.c`, `SameBoyLogo.png`), `LICENSE` (Expat) and `version.mk`, at **`ea667ad79aa28409f034b1fd6da7304e1844750d`** (2026-11-02, "Merge branch 'master' of github.com:LIJI32/SameBoy"; version 1.0.5). The commit is also in `SameBoy/COMMIT`, which `sameboy_sys::SAMEBOY_COMMIT` reads, so the checkpoint envelope can name its core commit from the one place.

**Why this commit:** it was the latest `master` when the scaffold was built (a `git clone --depth 1` on 2026-11-06 resolved to it), and it is the commit the backend evaluation read and measured, so every line reference and measured figure in [2026-11-06-backend-evaluation.md](2026-11-06-backend-evaluation.md) applies to the vendored code as is.

**To move it:** `scripts/vendor-sameboy.sh [commit]` replaces the vendored tree and `COMMIT`; then `scripts/build-roms.sh` rebuilds the boot ROMs, and this doc (or a new dated one) records the new commit and why.

## How it is compiled

SameBoy's `make lib` is not used: on macOS its dylib exports no symbols and, without the `cppp` tool, its installed headers come out empty while make exits 0 (evaluation ▸ Build and linking). Instead `crates/sameboy-sys/build.rs` compiles `Core/*.c` with the `cc` crate:

- `-std=gnu11 -O2 -D_GNU_SOURCE -DGB_INTERNAL -DGB_VERSION="1.0.5" -DGB_COPYRIGHT_YEAR="2026"` — the evaluation's flags. `GB_VERSION` is read from `version.mk` and `GB_COPYRIGHT_YEAR` from `LICENSE`, as SameBoy's Makefile derives them, so they follow a re-vendor. `-O2` applies to debug builds too, since tests run the emulator for real.
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

The same script assembles gbcli's own test ROMs from `crates/sameboy-sys/tests/roms/*.asm` (today just `smoke.asm`) with `rgbasm`, `rgblink` and `rgbfix -v -p 0`.

## Headless smoke test

`crates/sameboy-sys/tests/smoke.rs` boots `smoke.gb` through the DMG boot ROM, runs the cartridge for 60 frames (counted in `GB_run` ticks, 140,448 per frame), and asserts that nothing reached file descriptor 1 (it redirects fd 1 to a file, so C `printf` output is caught, not just Rust's). The ROM writes once to echo RAM at `$F000`, which SameBoy logs; the test routes that line through its log callback and asserts it arrived. Without the log callback, the test fails with `SameBoy wrote to stdout: "Wrote 42 to f000 (RAM Mirror)\n"` (checked by deleting the `GB_set_log_callback` call). Note that SameBoy logs echo-RAM writes only in `$F000-$FDFF`; `$E000-$EFFF` goes through its plain RAM path silently.
