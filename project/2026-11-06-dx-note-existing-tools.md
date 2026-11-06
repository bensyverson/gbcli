# DX note: a homebrew task with existing tools

2026-11-06. Job leaf `wumkEJ`. Done by a fresh subagent (dx-scout) with no knowledge of our design beyond a skim of the plan, acting as the target agent would: shell only, a small RGBDS program, then debugging and measuring it with PyBoy and SameBoy. About 11 minutes of wall time. Main-thread verification is at the end. Scratch artifacts (not in the repo) live under the session scratchpad's `dx/` folder.

**Headline findings for the design doc:**
1. **Name the time base.** "Frame N" meant the boot logo in one backend and gameplay in another.
2. **Track never-written memory.** Real hardware randomizes RAM, so predicates on uninitialized memory pass instantly at exit 0.
3. **Tag pixels with the frame they show.**
4. **Make cycles first-class.**
5. **A stop returns to the caller.**
6. **A fresh native process per round trip costs only ~25 ms**, so a daemon earns its keep through retained state, not latency (see "What this changes" below).

## Tools and exact versions

- **RGBDS v1.0.4** (Homebrew).
  - Build command: `rgbasm -o main.o main.asm && rgblink -o demo.gb -m demo.map -n demo.sym main.o && rgbfix -v -p 0xFF -t DEMO demo.gb`. Takes about 0.24 s, with no warnings under `-Wall`.
- **hardware.inc v4.9.1**, fetched from `https://raw.githubusercontent.com/gbdev/hardware.inc/v4.9.1/hardware.inc`. v4 was chosen on purpose: v5 renames constants (`LCDCF_*` and others), while most tutorials still use v4 names.
- **PyBoy 2.9.0** in a venv, installed with `pip install "pyboy==2.9.*"`.
  - It pulls numpy 2.5.4 and PySDL2. Pillow 13.0.0 had to be installed separately.
  - Run as `PyBoy(rom, window="null", sound_emulated=False)` with `set_emulation_speed(0)`.
- **SameBoy v1.0.5** at `ea667ad79aa2`. Build times: `make tester` 7.5 s, `make sdl` 6.8 s, `make lib` 5.0 s.
- **A C harness over libsameboy:** 86 lines. It reads one command per line on stdin; `.`-prefixed lines are harness commands, and everything else goes to SameBoy's debugger.
- Python 3.12.12, macOS (Darwin 27), Apple clang.

## The task and its answers

- **The program:** `main.asm`, 164 lines; 275 B of ROM0, 164 B of WRAM, 9 B of HRAM. It has a header and VBlank vector, turns the LCD off only during VBlank, and copies an 8×8 hollow-square tile. A 9-byte OAM DMA routine runs from HRAM, with a shadow OAM at $C000. Variables: `wSpeed` $C0A0, `wSpriteX` $C0A1, 16-bit `wFrame` $C0A2. The VBlank handler calls DMA and sets a flag; the main loop runs `halt`, increments `wFrame` and adds `wSpeed` to `wSpriteX`. It built first try. The variant `main_v2.asm` adds `sla b` to double the speed.
- **The answers.** All three backends agree once aligned on the game's own frame counter:
  - The sprite is visible.
  - Between wFrame 60 and 61, screen x goes from 59 to 60. Pixels lag WRAM by one update.
  - Main-loop work is **164 T = 41 M cycles per frame**. This agrees in SameBoy (`ticks`), PyBoy (via a save-state hack) and a hand count. It is about 0.23% of a 70,224 T frame.
  - A write watchpoint on `$FF46` stops in `hOAMDMA` with the correct backtrace.
  - Save at wFrame 30 / at wFrame 60:

    | Branch | wSpriteX at wFrame 60 |
    |---|---|
    | Baseline | 68 |
    | `wSpeed` poked to 2 | 98 |
    | Rebuilt v2 ROM under the old state | 98 |

    Restore-and-rerun gives bit-identical results.

## Findings → rules for gbcli

1. **"Frame 60" was the boot logo.** PyBoy reaches `Start` at about frame 65, SameBoy's boot ROM at frame 84. **NEW rule: name the time base.** Every frame or cycle count says what it counts from: power-on, boot handoff (PC=$0100), or a named checkpoint. Default to frames since boot handoff; offer a skip-boot checkpoint. (cli-design: never a plausible wrong answer.)
2. **Predicates on uninitialized RAM passed instantly, twice.** SameBoy randomizes WRAM like hardware (`wFrame` read 65535). "Run until wFrame ≥ 60" returned "ran 0 frames" at exit 0, while PyBoy zeroes RAM, hiding the bug. **NEW rule: track never-written memory.** A predicate or watch that reads memory the program has never written warns or exits 1 and names a remedy. Keep randomization on, with an explicit seed.
3. **Screenshot and memory disagree by a frame**, and each emulator stops at a different point in VBlank. **Rule:** a read sees settled state; every observation names its stop point (frame, LY, cycle). **NEW sub-rule:** pixels are tagged with the frame they show (the last *complete* frame) and its vblank type.
4. **Blank and partial frames came back at exit 0** (LCD off was all white, boot was all black). **Rule:** never a plausible wrong answer. Report the frame type, and refuse or flag a non-normal frame unless the caller asks for it.
5. **A hook on HALT hung PyBoy silently**, and output piped through `grep` was block-buffered. **Rule:** everything terminates. Runs have default budgets and end with exit 3 plus the last state, and a breakpoint on any opcode works. **NEW sub-rule:** long runs stream flushed JSON lines.
6. **PyBoy has no public cycle counter.** Getting cycles took a save-state byte-offset hack and about 4 round trips; SameBoy's `ticks` took one. **NEW rule: cycles are first-class.** Every stop record carries an absolute T-cycle count, and one verb measures A→B over N iterations (min, max, mean, in T and M).
7. **PyBoy can't hook HRAM**, where DMA code runs, and its error didn't say so. It has no watchpoints either. **Rule:** errors teach. Breakpoints and watchpoints cover the whole address space (HRAM, IO, OAM), bank-aware. Watch stops report both the writer's PC and the next PC.
8. **PyBoy hooks patch an opcode (0xDB) into game-visible memory.** **NEW rule: observation must not change what the program sees.** Breakpoints live in the execution path, never as patched bytes.
9. **SameBoy's debugger is unreachable without a window or terminal.**
   - The SDL frontend needs a window server, and it always requests an OpenGL window.
   - With piped stdin it spins at EOF and blocks in `SDL_WaitEvent`.
   - The tester gives only one BMP after N seconds.

   **Rule:** help and every verb work without a window or TTY. **NEW sub-rule: a stop returns to the caller** as a stop record; it never blocks waiting on a prompt.
10. **`make lib` exits 0 with 25 empty headers** (missing `cppp`, hidden by a pipe). **Rule:** no pipe hides a failing step, and a build verifies its outputs exist and are non-empty. Don't depend on SameBoy's generated headers.
11. **The debugger grammar is learned by failing.** `x wShadowOAM 4` fails; `x/4 wShadowOAM` works. `print [rLY]` needs `registers.sym`, which only the SDL frontend ships. `print [wFrame]` silently reads one byte of a 16-bit variable. **Rule:** one shape per verb class, and parse errors show the corrected command. Hardware register names are always available. Reads are typed by width, or use `.sym` sizes to warn.
12. **Output is unstructured and stop status ambiguous.** While stopped, the debugger took over stdin, so the parser changed with the state. **Rule:** stdout is the API. Every run or step returns one stop record (reason, pc, symbol+offset, cycle, frame, ly, bank), as a row by default and JSON with `--json`. Input parses the same whether stopped or running.
13. **Both cores run at real time by default** (400 frames took 6.7 s until turbo). **Rule:** headless runs are uncapped by default; pacing is an explicit flag.
14. **Rebuild → restore worked, but neither tool would notice if it shouldn't have.** **Rule:** reads carry a revision. A checkpoint records the ROM and `.sym` hashes. Restoring under a different ROM reports moved symbols and refuses if the PC or a live symbol moved, unless the caller overrides.
15. **Each hand-rolled harness answer took 3–5 tries, and the harness itself caused plausible wrong answers.** **NEW rule: one round trip per question.** Each of the brief's questions should be one verb.
16. **Smaller frictions:**
    - PyBoy's `screen.image` returned an error sentinel when Pillow was missing, rather than raising.
    - SDL warnings appear in "null" mode.
    - Harmless sandbox noise reads like failure.
    - 160×144 is too small to inspect.

    **Rule:** scaled PNGs on request, and a `doctor` verb for missing optional dependencies.

## Round-trip timings

Each round trip: restore a frame-30 state, run about 30 frames until wFrame=60, take a screenshot.

| Path | Time | How measured |
|---|---|---|
| PyBoy, warm, in-process | 5.6–6.7 ms | `perf_counter`, 4 runs |
| SameBoy harness, warm, turbo | 22–25 ms | harness timer (CLOCK_MONOTONIC), 5 runs |
| SameBoy harness, fresh process per round trip | 20–30 ms | `/usr/bin/time -p`, 5 runs |
| PyBoy, fresh process per round trip | 300–530 ms | `/usr/bin/time -p`; mostly Python and numpy imports |

- **Throughput on this demo:** SameBoy ≈1,400 fps rendering, ≈1,570 fps without. PyBoy reached ≈17,000 fps with `render=False`.
- **Save state size:** SameBoy 50,692 B, PyBoy 153,532 B.
- **Build:** 0.24 s.

## Main-thread verification (2026-11-06)

- **Fresh-process cost, reproduced** with `scripts/bench/sameboy-throughput` on `demo.gb`, 30 frames, rendering on. Emulation takes 19–21 ms per 30 frames (1,399–1,560 fps). Whole-process time was 0.15–0.19 s, but about 0.12 s of that is the bench's own 10,000-iteration save/load timing loop. So a fresh round trip costs roughly 25–30 ms, matching the scout. Emulation dominates, not process start.
- **The PyBoy figure is specific to this workload.** The demo spends ≈99.8% of each frame in HALT, which PyBoy fast-forwards. SameBoy's speed hardly changes between the HALT-heavy demo (1,200 fps over 3,600 frames) and busy `cpu_instrs` (1,324 fps). Don't read ≈17,000 fps as PyBoy's general speed.

## What this changes

- **The daemon trigger in the plan was overstated.** It argued that recreating state per command would cost a process start plus ROM and state loads; measured, that is about 25 ms, and emulation dominates. A persistent process is still justified by retained state: checkpoints, breakpoints, symbols, session events. That state could equally live in a session file loaded per command, which is what cli-design prefers ("stateless commands over a stateful file beat a daemon"). **The design doc must weigh a stateless CLI over a session bundle against a daemon**; the plan's architecture section carries a marked correction.
- **Emulation speed is the latency that matters.** Each extra feature hook costs speed directly.
- **Rules the design doc should adopt:** the NEW rules above (time base; never-written memory; frame-tagged pixels; cycles first-class; stops return to the caller; flushed streaming; observation without patching; one round trip per question), plus the scout's sketch below.

## The scout's "one round trip" sketch

```
gbcli run demo.gb --from boot-handoff --until 'u16 wFrame >= 30' --checkpoint f30
  → stop: reason=predicate frame=31(since $0100) cycle=… pc=MainLoop  checkpoint f30 (rom 3fa2…, sym 91c0…)
gbcli measure demo.gb --at f30 --from MainLoop+1 --to MainLoop --frames 60
  → n=60  T min/max/mean 164/164/164  M 41  (0.23% of frame)
gbcli run --at f30 --poke wSpeed=2 --until 'u16 wFrame >= 60' --shot a.png --scale 4 --read wSpriteX,OAM[0]
  → stop frame=61 vblank=normal  shot a.png shows frame 60 (last complete)  wSpriteX=98  OAM[0]={y:88,x:97,tile:1}
gbcli watch --at f30 write rDMA --once --json
  → {"reason":"watch","addr":"$FF46","value":"$C0","pc":"hOAMDMA+2","writer_pc":"hOAMDMA","bt":[…],"cycle":…,"ly":144}
gbcli build && gbcli run --at f30 …   → warns: ROM changed since f30; moved: none live — continuing
gbcli diff a.png b.png                → 1 sprite moved: OAM[0] x 97→98; 8 px changed
```

## What the scout said the brief got wrong

- SDL2 *was* installed. The blocker is the window server and the forced OpenGL window.
- "60 vs 61 frames" lacked a time base.
- "Break when OAM DMA starts" needs a write watchpoint on `$FF46`, because the routine runs from HRAM.
- PyBoy has no public cycle counter.
- Patching a ROM init constant after init does nothing; the realistic loop is rebuild-and-restore.
- `make lib` also needs `cppp`.
