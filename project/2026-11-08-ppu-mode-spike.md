# PPU mode at raster writes and in conditions: the spike (leaf PnsILN)

**Answer: yes to both, except in one narrow window.** SameBoy's display state says the PPU mode without running it everywhere but late in a mode 3 that SameBoy batched by an upper bound. So each `raster_write` now carries `mode=`, and `[rSTAT]` reads in a condition wherever that holds. In the window where it doesn't, the condition still fails, but its stop now says `condition_error`, never `breakpoint`. The bespoke-core un-park condition does not hold.

Why this was asked: both example games' costliest stumble ([2026-11-08-examples-dx-review.md](2026-11-08-examples-dx-review.md), item 1). The racer's `[rSTAT]` breakpoint failed with `unsupported (SideEffectFreeIo): … PPU registers are unavailable while mode-3 cycles are batched`, while `show` reported `stop breakpoint`.

## How SameBoy batches, and what the mode is inside a batch

Every claim here is read from the vendored source, `crates/sameboy-sys/SameBoy/Core/` (commit in `SameBoy/COMMIT`).

- `GB_display_run` is a state machine that sleeps through `GB_SLEEP`, which leaves `display_cycles <= 0`. Only `GB_BATCHPOINT` (`timing.h`) returns with cycles still held (`display_cycles > 0`), and only while `display_cycles < 2 × length`. That is what `gbcli_ppu_batched` tests.
- There are two batch points (`display.c`):
  - **state 5, the OAM scan** (length 80 dots, line 1834). STAT is set to mode 2 just before (line 1821). Within 80 dots the scan loop changes no STAT bit, so the mode is 2.
  - **state 3, mode 3's start** (line 1886), by `mode3_batching_length` (line 1500). STAT is already 3 (line 1857). The length is either:
    - exact, `167 + SCX % 8`, on a line with no objects and no window. Mode 0 begins right after the batch, so the mode is 3 while the batch holds.
    - `300`, "a bit more than the maximum Mode 3 length", on a line with objects where no HBlank interrupt can be seen (STAT bit 3 or IE bit 1 clear). There the mode is 3 only before the shortest mode 3 could end, `167 + SCX % 8` dots in. Past that, only running the display can tell 3 from 0.

      > **Correction, 2026-11-09 (leaf nin31Q, from review-ppu14): not only on a line with objects.** `mode3_batching_length` (`display.c:1522`) returns the exact length only when the line has no objects *and* the window hasn't triggered on it (`wy_triggered` with LCDC's window bit). So `300` is also the batch on a window line with no objects, wherever no HBlank interrupt can be seen. The rule above still holds there: the shim's `gbcli_ppu_mode` tests the length, not the objects, so it says mode 3 before the shortest mode 3 could end and unknown after. The window sweeps `window_line_stat_matches_cpu_reads_on_a_dmg` and `…_on_a_cgb` (pre-release, `crates/sameboy/tests/it/mode_sweep.rs`, on `mode_sweep_win.asm`: SCX 7, WX 7, no objects) find every decoded STAT and IF equal to a CPU read. The same leaf made `Core::ppu().mode` an `Option`: it had decoded STAT from the IO array, which reads mode 3 throughout the late window, so `show`'s `mode=` and the live frame's label could be confidently wrong there. They now say unknown (`ppu_mode_is_unknown_not_wrong_in_the_late_window`, same file).
- Inside a batch, the display writes only IF, STAT and LY of the IO registers (the only `io_registers[...]` assignments in `display.c`). No batch spans a line's end, so LY is current. IF and STAT are current wherever the mode is known. The rest (LCDC, SCX, SCY, LYC, BGP, OBP0/1, WY, WX, BCPS, OCPS, OPRI) change only by CPU writes, and every such write syncs the display first (`sync_ppu_if_needed`, `memory.c:471`).
- The write callback fires before the write's own sync (`GB_write_memory`, `memory.c:1834`, before `write_high_memory`'s `sync_ppu_if_needed`). So at a raster write the display can be batched, and the mode has to come from the same rule.

The rule is the shim's `gbcli_ppu_mode` (`crates/sameboy-sys/shim/gbcli_shim.c`). `gbcli_io_peek` now decodes STAT and IF wherever it gives a mode, LY and the CPU-written registers always, and still refuses HDMA5, palette data, VRAM and OAM while batched: HDMA's progress and the VRAM and palette blocks change inside the OAM-scan batch (`display.c:1846`, index 37 of the scan).

## Evidence

- **A ROM that writes while batched:** `crates/sameboy/tests/roms/ppu_mode.asm`. HALT wakes it on an LYC match, and counted NOPs place SCX writes with no PPU read before them:
  - line 50: in mode 2 (38 dots in), in mode 3 (130 dots) and in mode 0 (342 dots). The line has no objects.
  - line 100: late in mode 3 (278 dots). The line has ten objects and the HBlank interrupt is off.

  Dot figures are `into=` from `gbcli run ppu_mode.gb --frames 3 --raster`, halved.
- **Exactness, against the truth:** the sweep tests `ppu_registers_decode_while_the_display_is_batched_on_a_dmg` and `…_on_a_cgb` (`crates/sameboy/tests/it/io_decode.rs`). At every instruction boundary on lines 50 and 100 over two frames, each of STAT, LY, IF, LCDC, SCX, SCY, LYC, BGP, WY and WX that decodes equals a CPU bus read on a restored copy. A bus read syncs the display, so the copy is the truth.
  - **DMG, temporary counts** (`cargo test -p gbcli-sameboy --test it ppu_registers_decode -- --nocapture` with an `eprintln!`, since removed):
    - line 50: 13 boundaries were batched and nothing was refused there. Before this change, all 13 refused every PPU register.
    - line 100: 62 boundaries were batched. STAT and IF were refused at 5 of them (the late window) and decoded exactly at the other 57.
  - `busy_rom_takes_the_per_pixel_path` (`layers.rs`) and `debug_views_draw_while_the_ppu_has_batched_cycles` (`crates/view`) had used "STAT or BGP is refused" to detect a batch. They now use HDMA5, which is still refused exactly while batched.
- **The racer's case, reproduced:** before the fix, a breakpoint at `Line50Oam` with `([rSTAT] & 3) != 0` failed with the racer's exact message. Its stop said `Breakpoint { id: 1 }`, ly 50, which is the OAM-scan batch. The racer's `show` had said `mode=oam_scan` at its failing stop, which fits this batch. The racer's failing build was replaced before it was committed, so its own ROM can't be rerun. Now the breakpoint stops as a breakpoint (`crates/engine/tests/it/ppu_mode.rs`).
- **The example games, from boot** (worktree build; copies of `examples/` built with `gbcli build`; `gbcli run … --raster --json` counted by mode):
  - **Racer:** 900 frames on `tools/win.keys`, 174,576 IO writes. 81,330 SCX and 81,330 SCY writes are all `hblank`; the 863 of each made in VBlank are `vblank`. No write has an unknown mode with the LCD on.
  - **Shooter:** 3,000 frames on `keys/win.keys`, 39,770 IO writes. All 8,973 scroll-split SCX writes are `hblank`. 95 have an unknown mode: sound-register writes (NR33-NR44, NR51) from the main loop, late in a batched mode 3.
  - The READMEs' breakpoint proofs still pass on the new binary: the racer's stops at `[wState] == 2` at frame 2715, the shooter's at frame 4552.

## Question 1: the mode (and the dot) at each raster write

**The mode: yes.** `Event::IoWrite` gains `mode: Option<PpuMode>`, the engine's `RasterWrite` and the wire's `RasterWriteRow` gain `mode`, and the CLI prints `mode=` (`hblank`, `oam_scan`, `drawing`, `vblank`). It is absent while the LCD is off and in the late window. Test: `a_write_carries_the_mode_it_landed_in_or_none`. The `raster` golden was re-blessed.

**The dot: no, not cheaply.** The same state (`display_state`, `display_cycles`, `cycles_for_line`) would give a dot only by modeling each state's position. The stage-1 correction in [2026-11-06-design.md](2026-11-06-design.md) §2.2 found that fragile, and nothing here changes that. `ppu_dot` stays unsupported. `ppu_mode_events` also stays unsupported: SameBoy still has no callback to time a change. Its reason now says that each write carries its mode instead.

**Not done:** syncing the display early in the write callback, to fill the late window for PPU registers. The write syncs a few lines later anyway, so this is probably sound for those registers. But a write gbcli vetoes never reaches SameBoy's own sync, and the sync would run the display inside gbcli's write callback, which holds the hooks. It would need a proof that the state stays byte-identical with and without the sink, and the window didn't show up on either game's PPU writes.

## Question 2: `[rSTAT]` in a condition without the refusal

**Yes, by the display's own state, not by unbatching.** The same `gbcli_ppu_mode` lets `[rSTAT]` decode at every batched boundary except the late window. Tests: `a_stat_condition_evaluates_while_mode_2_is_batched` and `…_mode_3_is_batched`; both failed with the racer's error before. Unbatching around conditional breakpoints was rejected. Forcing SameBoy's slow path on a 300-batched line changes when its mode 3 ends, the HBlank STAT bit and VRAM access. So a breakpoint's presence would change the emulation.

**Never both an error and a stop.** A breakpoint's or watch's condition that fails to evaluate is still a run error (exit 5, `unsupported`), and its stop record now says `condition_error id=N` (`StopReason::ConditionError`, a new wire variant) instead of the breakpoint's or watch's own reason. `show` then says `stop condition_error id=1 …`, not `stop breakpoint`. This matches `--until`'s `predicate_error`. Test: `a_condition_that_cannot_be_evaluated_is_an_error_not_a_breakpoint_stop`, at `Line100Late` (red before: `Breakpoint { id: 1 }`). It is an error stop rather than a refusal before the run: whether STAT can be read is only known at the boundary.

## The bespoke-core un-park condition

[backlog.md](backlog.md) ▸ *A bespoke Rust emulator core* un-parks on an `unsupported` that blocks an agent's task "and no adapter work can serve it". **It does not hold.**

- The two games' gaps were `ppu_mode_events`, `ppu_dot` and `side_effect_free_io`. Adapter work now serves the need behind them, whether a write landed in HBlank: every scroll write in both games carries its mode.
- What is left doesn't block those tasks:
  - no dot;
  - no mode-change events;
  - STAT unreadable late in a 300-batched mode 3. A condition hitting it now says so (`condition_error`), and `--raster` leaves the mode off.

The other two un-park conditions (hardware disagreement, a mid-mode-3 eval class) aren't touched. The window for an exact STAT everywhere is SameBoy's 300-dot batch bound. If a real task hits it, the next step is an adapter one: run the display on a scratch copy of the state, not the live instance. A bespoke core isn't needed for that; see [backlog.md](backlog.md) ▸ *STAT late in a batched mode 3, from a scratch copy*.
