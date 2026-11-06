# Backend evaluation: which emulator core, and how to drive it

2026-11-06. **Decision: wrap SameBoy (vendored, pinned) behind a C interface of our own design, driven from Go; keep a bespoke Rust core as a defended option with a scheduled decision.** The plan that follows from this is [2026-11-06-plan.md](2026-11-06-plan.md).

This is a point-in-time record. Upstream snapshots evaluated:

| Project | Commit | Date | License |
|---|---|---|---|
| SameBoy | `ea667ad` (v1.0.5) | 2026-11-02 | Expat (MIT); `iOS/` adds an app-store clause, `HexFiend/` BSD — neither is used |
| docboy | `01e04d3` | 2026-08-21 | MIT |
| PyBoy | `350e96f` (v2.9.0) | 2026-10-30 | LGPL-3 |
| MetroBoy (GateBoy/LogicBoy) | `36797ad` | 2025-02-22 | **none** (no LICENSE file) |
| Gameboy_MiSTer | `f6796f3` | 2026-09-21 | GPL-3 |

**Evidence tags.** *Verified* means someone read the code at the cited line, or ran it. *Measured* means a number from a run, with the tool named. *Assumed* means an inference not yet tested. Most reading was done by research subagents and spot-checked on the main thread; the spot-checked claims are marked ✓. Reading code is not running it (`project/agents/evidence.md`), so the first plan stage re-tests the load-bearing claims.

## The goal, as clarified with the user

- **Who:** an agent (Claude Code, Sonnet-class or better) helping a homebrew developer push the DMG to its limits, e.g. a fast arcade puzzler with clever assembly. GBC support is wanted; SGB and exotic variants are not.
- **What matters most:** emulation fidelity, then *fast feedback loops* — restore a state, try something, measure, restore again — then debugging and inspection depth.
- **Clients:** both harness code (owns a pipe, speaks JSON Lines) and an agent in a shell (needs a CLI client, because Claude Code's Bash tool cannot keep a stdin pipe open across calls).
- **Platforms:** macOS arm64 first; Linux ideal; open source.
- **Toolchain:** RGBDS first (assembly is the performance path), GBDK later.
- **Onboarding:** not a maximal course. The lens is "what does a Sonnet-class model need to get productive with GB development through our CLI", with links out to Pan Docs and gbdev for depth.

## Candidates at a glance

Fidelity reference: the gbdev emulator shootout, <https://gbdev.io/GBEmulatorShootout/>, updated 2026-06-04: docboy 258/261, Emulicious 254/261 (closed source), SameBoy 253/264, bgb 205/264, GambatteSpeedrun 169/207, mGBA 150/264, PyBoy 91/261 (likely a stale version), ares 26/261. **The denominators differ, so these totals are not directly comparable**; the per-test failure lists have not been extracted yet (a plan leaf does it).

| | SameBoy | docboy | PyBoy 2.9 | GateBoy | MiSTer |
|---|---|---|---|---|---|
| Throughput, cpu_instrs, 60 emulated s | **21.8× realtime** (measured) | 6.2× (2.3× with debugger compiled in) (measured) | — | 6–8 fps (README) | ModelSim: "1 s realtime ≈ 5 min" |
| DMG model | DMG-B only; runtime choice incl. CGB revisions | separate compile-time DMG build; no revision | one | one | one |
| In-memory save state | 50,712 B; load 10.3 µs, save 1.2 µs (measured) | 67,023 B; unversioned; differs by build config | exact only at frame boundaries | raw dump | via SDRAM bus |
| Side-effect-free inspection | only via direct access | yes (`const` bus reads) | — | — | — |
| Debugger drivable headless | no (blocks, text only) | no (frontend blocks on stdin) | no watchpoints | — | none |
| Per-channel audio | yes (public getters, mute, VGM) | no | — | — | — |
| Library API | stable public C API | C++, state private behind friend classes | Python | C++ | HDL |

**Verdicts.**

- **SameBoy — chosen.** Top-tier fidelity, 3.5× docboy's speed, 10 µs state restores, a stable C API, CGB in the same binary, per-channel audio. Its debugger is not reusable headless (below), so we write our own controller on its public hooks.
- **docboy — lost the bake-off; keep as a reference.** Probably the most accurate DMG core (2,949 DMG test cases in its suite; `const` bus reads; banked watchpoints work) but slower, compile-time model selection, private state, and a debugger that blocks on `std::cin` (frontend.cpp:1075 ✓). Adopting it means owning a fork. Useful later as a third opinion when SameBoy and a bespoke core disagree.
- **PyBoy — design reference only.** Accuracy improved a lot in 2026 (mooneye DMG acceptance 62/75, every failure a `boot_*` test from its own boot ROM; blargg timing suites pass; `oam_bug` fails). But save states are exact only at frame boundaries (core/lcd.py:1703-1739), hooks patch opcode `0xDB` into game-visible memory (pyboy.py:1460-1516), and there are no watchpoints. Worth borrowing: `memory[bank, addr]` addressing, `tick(n, render, sound)`, the memory scanner.
- **GateBoy/LogicBoy — rejected as a backend.** The CPU is not die-traced (README), mooneye runs are disabled with known failures (GateBoyTests.cpp:268-274), MBC1 only, no license, last commit 2025-02. Its gbmicrotests ROMs are the useful part, and docboy already runs them. Being gate-level does not by itself make it more accurate.
- **Gameboy_MiSTer — rejected.** T80 Z80 CPU core (gb.v:408); it only began passing mooneye OAM DMA tests in May 2026; its simulator is ModelSim plus Lua, not Verilator, and 32 VHDL files would block Verilator anyway; no debugger. No accuracy advantage found.
- Others: Gambatte (GPLv2, CGB-centric), mGBA (good debugger, weaker DMG), Emulicious and BGB (closed, no library), ares (uncompetitive on GB). None beats SameBoy for this use.

## SameBoy: requirements mapped to source

Tags: **P** public API in the exported headers; **I** needs internal access (compile `Core/` ourselves with `-DGB_INTERNAL`); **U** needs an upstream patch or our own work around it. Line references are against `ea667ad`.

### Build and linking

- `make lib CONF=release` exits 0 in 13.4 s on arm64 with no SDL or rgbds (measured). **But the macOS dylib exports zero symbols** (Makefile:388 passes `-exported_symbols_list /dev/null` ✓), and without the `cppp` tool all 22 generated headers are **empty while make still exits 0** (Makefile:932-934 reports sed's status).
- Compiling `Core/*.c` directly with `-DGB_INTERNAL` takes 3.3 s and gives struct access (measured). Flags: `-std=gnu11 -O2 -D_GNU_SOURCE -DGB_VERSION='"1.0.5"' -DGB_COPYRIGHT_YEAR='"2026"' -Wno-multichar`. **So we vendor `Core/` at a pinned commit and compile it ourselves.**
- Feature switches `DISABLE_TIMEKEEPING`, `DISABLE_REWIND`, `DISABLE_DEBUGGER`, `DISABLE_CHEATS` (Makefile:62-121). The debugger agent suspects `debugger.c` tests `DISABLE_REWIND` where the build defines `GB_DISABLE_REWIND` (assumed build break; untested).
- rgbds is needed only to build SameBoy's own boot ROMs. `BOOTROMS_DIR=` accepts prebuilt ones (Makefile:123).

### 1. Headless execution

- **Not headless by default.** `GB_init` installs callbacks that use the process's stdio: the log callback is `printf` (gb.c:24), and the input callbacks read stdin, the async one at every vblank (gb.c:51-105, 1217). We must replace the log, input and async-input callbacks (P).
- **`GB_run` sleeps to wall-clock time unless turbo is on** (timing.c:83-89 ✓). Use `GB_set_turbo_mode(gb, true, true)` plus `GB_set_turbo_cap(gb, 0)` (P), or `DISABLE_TIMEKEEPING`.
- **Models** (model.h ✓): `DMG_B` is the only DMG; `DMG_0/A/C` are commented out. MGB, SGB family, CGB_0…CGB_E, AGB_A, GBP_A. So "pick a DMG revision" means DMG-B; CGB revisions are real choices.
- **Boot ROM:** `GB_load_boot_rom[_from_buffer]`, `GB_set_boot_rom_load_callback` (P). There is **no skip**: without a boot ROM, `0000–00FF` reads as zeros until FF50 is written (memory.c:270-273). The bench uses the hack `GB_write_memory(0xFF50, 1)` plus PC/SP, which leaves PPU and DIV phase unlike a real boot (assumed to matter for some tests). SameBoy ships only `.asm` boot ROM sources (Expat); README:33 says some mooneye tests need Nintendo's original boot ROMs, which we must never bundle.
- **Advancing:** `GB_run` executes one CPU step — an instruction, an interrupt dispatch, or a HALT/STOP tick — and returns **8 MHz ticks** (2 per T-cycle at single speed) (gb.c:1192-1228, sm83_cpu.c:1606-1719). `GB_run_frame` runs to a vblank. While the LCD is off, a synthetic `LCD_OFF` vblank fires every 70,224 T. The vblank callback fires mid-instruction (Tester/main.c:556), so take snapshots after `GB_run` returns.
- **Anomaly (measured):** in the bench, `GB_run_frame`'s return values summed to 0 ns. Unexplained; summing `GB_run` ticks works. Don't rely on `GB_run_frame`'s return until it is understood.
- **Finer than an instruction:** no public T-cycle or dot stepping (U). Hooks that fire inside an instruction (P): read and write memory callbacks per access (memory.c:802, 1834), the execution callback per opcode fetch (sm83_cpu.c:1708), LCD line and LCD status callbacks.
- **No rendering / no audio:** `GB_set_rendering_disabled` (P); audio is rendered only when a sample rate is set. With rendering on, a NULL screen or encode callback crashes (display.c:733-765).

### 2. Save states

- **Saved** (gb.h:349-655): CPU registers, IME, IE, halt/stop, `halt_bug`, data bus and its decay; OAM DMA and HDMA; MBC banks and per-MBC state; HRAM and the IO register array; the DIV and display state machines, TIMA reload, serial, joypad bounce; the APU; RTC; the full PPU including FIFO and fetcher; printer/WorkBoy; then MBC RAM, WRAM, VRAM, and BESS blocks. Magic `SAME`, version 15.
- **Not saved:** the ROM; the boot ROM; **key state**; all callbacks; **all debugger state**; DMG palette and display settings; turbo, clock multiplier, RTC mode; audio output config; cheats.
- **Load mechanics** (save_state.c:1321-1381): the load overlays saved sections onto a copy of the live struct, so **ROM patches survive a load**. The **model is taken from the state**: within a family (DMG_B ↔ MGB ↔ SGB, or CGB revisions) a load **silently switches the instance's model**. **No native ROM identity check** (BESS INFO only logs a mismatch). The loader puts about 125 KB on the stack.
- Size and speed: 50,712 B for DMG; load 10.3 µs, save 1.2 µs (measured, `scripts/bench/sameboy-throughput`).
- **Consequence:** our checkpoint must be an *envelope* — the SameBoy blob plus wrapper-owned state: ROM hash, core commit, model, boot ROM hash, config, the emulated cycle counter, the ROM patch set, key state, debugger configuration — and loading must verify the envelope before touching the core.

### 3. Inspection

- Registers: `GB_get_registers` returns a live, mutable pointer to AF…PC (P). IME and halted are I.
- **Memory regions:** `GB_get_direct_access` (P) gives raw pointers, size and current bank for ROM, ROM0, WRAM (all banks), cart RAM (all banks), VRAM (both banks), HRAM, the IO array, BOOTROM, OAM, CGB palette RAM, IE. Not exposed: extra OAM (FEA0–FEFF), APU internals (some APU getters are P).
- **The IO array is not the register value** for several registers: DIV is derived from `div_counter`, TIMA reads 0 during reload, JOYP has a switching delay, NRxx go through `GB_apu_read`. Reads of hardware registers need per-register decoding.
- **`GB_read_memory` is a real bus read** — it fires watchpoints and the read callback, updates the open-bus value, and on DMG can **corrupt OAM** (the OAM bug) when reading FE00–FEFF in mode 2 (memory.c:546-580).
- **`GB_safe_read_memory` is not side-effect free either**: it syncs the PPU, writes the saved `cpu_vram_bus` field, runs the read callback, and returns 0xFF when VRAM/OAM is blocked (memory.c:823-837). **Only direct access is a true peek.** Inspection verbs must use it, and the plan proves "inspection does not advance or perturb emulation" by hashing the state before and after every read verb.
- Bank and MBC: the current bank comes from direct access; the MBC type and union are I (or parse header byte 0x147).

### 4. Framebuffer and debug views

- **Single buffer, written in place** (display.c:733-765). At a `NORMAL_FRAME` vblank it holds a complete frame. Mid-frame, lines above LY are the new frame, lines below are the old one, and mode-3 batching can lag the current line. A forced sync is I (`GB_display_sync`); a P workaround is `GB_safe_read_memory(0x8000)`.
- **So "last completed frame" is ours to keep**: copy the buffer at each `NORMAL_FRAME` vblank. The live buffer is a separate, explicitly partial view.
- DMG colors come from a 4-entry RGB palette (default grey). **Raw shades 0–3 are recoverable** with a sentinel palette and encoder (P). **Which layer drew a pixel is not recoverable** (U).
- Views (P): `GB_draw_tileset` (256×192), `GB_draw_tilemap` (256×256, the full off-screen map), `GB_get_oam_info` (40 entries incl. line-limit obscuring). Naming quirk: `GB_TILESET_8800` means unsigned $8000 addressing.

### 5. Live modification

- `GB_write_memory` is a full bus write with IO side effects: DIV reset, LCD on/off, DMA start, MBC bank switches, FF50 (memory.c:1821-1872). Direct-access writes are raw pokes.
- **ROM patches:** pokes through direct access take effect immediately. Header fields (0x147 etc.) are parsed only at ROM load. MMM01 ROMs are rotated in memory, so offsets differ from the file. There is **no IPS/BPS loader**; patch the buffer and reload, or poke.
- So the protocol must name three distinct operations: **bus write** (as the CPU would, with side effects), **poke** (backing memory, no side effects), and **ROM patch** (by bank and offset, wrapper-recorded).

### 6. Debugger

Read in full by a research subagent; key claims spot-checked ✓.

- **Control model:** a stop blocks inside `GB_run`, in the input callback (debugger.c:2675-2695 ✓). Returning NULL resumes; the host cannot get control back while staying stopped. The command language (~30 commands, debugger.c:2235-2294) answers only in human text through the log callback; stop reasons appear only in that text, and a stop after a step prints nothing.
- **No structured layer underneath:** breakpoint and watchpoint structures are file-static, so even a `GB_INTERNAL` build can't reach them. Public: `GB_debugger_evaluate`, symbol loading and lookup, `GB_debugger_break`, `GB_debugger_is_stopped`, `GB_debugger_set_disabled`.
- **The debugger can perturb DMG fidelity:** conditions, the watchpoint `old` value and the jump-to peek use `GB_read_memory` (debugger.c:220-237, 550, 577, 662, 3069 ✓) — open-bus updates and possible OAM corruption.
- **Banked cart RAM and VRAM watchpoints can never fire:** `bank_for_addr` returns 0 for 8000–CFFF and E000+ (debugger.c:65-83 ✓).
- **An illegal opcode locks the CPU** (IE=0, halted) without entering the debugger (sm83_cpu.c:346-351 ✓).
- Every `execute_command` takes two full state snapshots for undo (debugger.c:2503-2530).
- **Conclusion:** don't drive the text debugger. Build our own on public hooks: check PC and bank *before* each `GB_run` (a stop lands before the instruction executes); use the read and write callbacks for watchpoints (the write callback can veto, the read callback can replace a value); keep conditions side-effect-free by evaluating them over direct access. Caveats: the execution callback can't veto and doesn't fire for interrupt dispatch; the write callback misses DMA destination writes; the read callback also fires for the core's own "safe" reads.

### 7. Timing and determinism

- Finest public step: one instruction. The cycle counters `debugger_ticks` / `absolute_debugger_ticks` are I, unsaved, and removed by `DISABLE_DEBUGGER`. **We keep our own emulated-time counter** (sum of `GB_run` returns) and store it in the checkpoint envelope.
- **Initial RAM is randomized from one process-wide, time-seeded RNG** (random.c ✓). `GB_random_seed` / `GB_random_set_enabled` are P but global. Measured: seeding 42 before each `GB_init` makes two instances byte-identical.
- **RTC** defaults to syncing with the host clock; `GB_RTC_MODE_ACCURATE` is cycle-based (P). `time(NULL)` still leaks on reset (HuC3), battery load/save and BESS RTC import.
- Joypad bounce emulation exists; Tester turns it off. Keys are unsaved.
- **Several instances can coexist** in one process (libretro runs two). No locks. Globals: the RNG, camera noise seed, an `sgb.c` once-flag, libc `rand()` (interference audio only).

### 8. Audio

- Per-channel public getters: `GB_get_channel_volume`, `_amplitude`, `_period`, `_edge_triggered`, `GB_get_apu_wave_table`, plus `GB_set_channel_muted` (apu.h ✓). A sample callback and `GB_start_audio_recording` with RAW/AIFF/WAV/**VGM** formats ✓. VGM is a timestamped log of APU register writes — the structured audio view we want.
- Per-channel waveforms: sample the amplitude getters, or replay from a checkpoint with other channels muted (determinism makes the replay exact; to be verified).

## Cartridges and save mechanisms

Researched 2026-11-06 from GB Studio `eb6aa8a`, gbvm `4f01d26`, FlashGBX `5b2398b` and vendor pages.

- **GB Studio** supports exactly MBC5 (default; header byte **0x1E**, MBC5+RUMBLE+RAM+BATTERY, makeBuild.ts:73 and buildMakeScript.ts:125 ✓) and MBC3 (0x10, with RTC). Default save is 32 KiB battery SRAM.
- **GB Studio "batteryless" saves write to the ROM flash chip** with the data-line-swapped AMD sequence `$0AAA←$A9, $0555←$56, $0AAA←$A0` (gbvm flasher_s.s:49-51 ✓), reserving banks 12–15, run from WRAM with interrupts off, completion by read-back rather than DQ7/DQ6 polling.
- **No emulator checked models flash command writes** — SameBoy, docboy, MiSTer (grep; mGBA, Emulicious, BGB unverified). A gap gbcli can fill.
- **Flash boards vary:** across FlashGBX's 106 DMG cart definitions, at least six unlock-address variants (`$AAA`, `$555`, `$5555`, `$4AAA`…) and a D0/D1-swapped variant; /WE on /WR (79) or AUDIO (26). Inferred (assumed): GB Studio's hard-coded swapped sequence fails silently on unswapped boards (insideGadgets, BennVenn, ModRetro). A useful flash model is therefore parameterized.
- **Chip-based homebrew boards** (insideGadgets, BennVenn) save to **FRAM or SRAM**, not flash. FRAM behaves as plain non-volatile RAM.
- **SD flash carts** (EverDrive GB X7, EZ-Flash Junior) emulate MBC1/2/3/5; neither is documented to emulate flash-command writes. EZ-Flash Jr's RTC writes are reportedly imperfect (developer forum post).
- **MBC5** is the first MBC guaranteed for CGB double speed (Pan Docs). The often-quoted ~150 ns ROM access figure is forum lore; no primary source found.
- **Mapper priorities:** must — MBC5 (bank 0 selectable, 9-bit bank, 128 KiB RAM, rumble bit), ROM-only, battery SRAM, MBC3+RTC; should — parameterized AMD/JEDEC flash model, MBC1/MBC1M, FRAM; could — MBC2, MBC30, Intel command set, CPLD multicart, MBC7. SameBoy implements the standard mappers; the flash model can be built on its read/write hooks without patching SameBoy (assumed; the stage that builds it verifies).

## Language

Measured on this machine (Go 1.27.3, Apple clang 21, arm64), `scripts/bench/`:

| | |
|---|---|
| Go → C call | 25.7 ns |
| C → Go callback | 38.8 ns |
| SameBoy's own cost per instruction | ≈ 44 ns (1,301 fps over ≈ 17.5k instructions/frame — the instruction count is from an LCD-off NOP frame, so this is approximate) |
| Go build linking SameBoy, incremental | 0.98 s |
| Go build, clean cold cache | 6.2 s |
| Rust build | not measured — Homebrew `rustc` 1.96.0 aborts on an LLVM symbol mismatch |

- **A Go callback per instruction would roughly halve emulation speed.** So the per-instruction check lives in C: a small shim holds bank-aware address bitmaps for breakpoints and watchpoints and calls into Go only on a match. Matches are rare, so the overhead is near zero; condition evaluation, the protocol and everything else stay in Go.
- **Decision (user, 2026-11-06): Go for the SameBoy wrapper; Rust if a bespoke core is built.** Go gives the fastest builds, is already maintained by the user, and agents write it reliably. Rust's zero-cost C calls matter more for a bespoke core's raw speed.
- Zig was considered: best C interop, but it still breaks compatibility between releases, which makes it harder for agents to write. C for the server was the original default; the Go harness keeps C to the hot-path shim.

## The bespoke core question

The user asked to keep a from-scratch (or ported) core on the table. Findings:

- **For:** every wrapper limitation above — side-effecting reads, instruction-only stepping, no layer identity, no saved cycle counter, global RNG — disappears in a core designed for agents. DMG+CGB without SGB, camera and printer is roughly 12–15k lines (SameBoy's whole `Core/` is 22,467 lines including a 3,099-line debugger; measured with `wc -l`). docboy and SameBoy are MIT, so behavior can be ported, and a large test corpus exists.
- **Against:** the last few percent of DMG fidelity (mode-3 length, the OAM bug, APU quirks, STAT edge cases) took those projects years, and the user's use case aims straight at those edges. Lockstep against SameBoy treats SameBoy's ~11 failures as truth unless docboy breaks ties.
- **Decision (user, 2026-11-06): wrapper first, bespoke as a *defended* option:**
  1. Our C interface is designed for the ideal core; where SameBoy can't deliver, the shim returns an explicit *unsupported*.
  2. A **gap ledger** counts every *unsupported* an agent hits.
  3. A **scheduled decision leaf**, blocked by the first agent eval, must answer "spike now" or "park with an un-park condition".
- **The spike, if run:** Rust, implementing the same C interface; DMG only, no APU. Checkpoint 1 (~2 agent sessions): blargg `cpu_instrs` + `instr_timing`, instruction-level lockstep with SameBoy. Checkpoint 2 (~3 more): `mem_timing`, mooneye timer/interrupt/OAM DMA, dmg-acid2 pixel-exact, frame-level lockstep across homebrew ROMs. Gate: at least SameBoy's pass list on that set, plus a cost projection for the APU and CGB tail. Stop at ~6 sessions either way.

## Open questions carried into the plan

- Per-test shootout failures for SameBoy vs docboy (which edges does SameBoy miss?).
- Whether the open boot ROMs vs no-boot-ROM start changes any test result we care about.
- `GB_run_frame` return anomaly.
- Whether per-channel replay with muting is exactly deterministic.
- Test-ROM licensing: which suites may be committed vs fetched by a pinned script.
