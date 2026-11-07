# docboy baseline: docboy's cases from docboy's own hand-over state

2026-11-06. **Finding: run from the post-boot state docboy assumes, SameBoy fails 173 of docboy's 2,949 cases, not 469; 26 of those 173 are runner artifacts, and none of the rest is a boot-state effect.** 136 are disagreements with docboy's own tests, which state no hardware verification; 10 are SameBoy gaps already known from third-party suites. Leaf `5ewkYZ`; it follows the SameBoy baseline ([2026-11-06-sameboy-baseline.md](2026-11-06-sameboy-baseline.md)), which ran docboy's cases after SameBoy's open boot ROM and left them unclassified.

## Re-running it

```
python3 -I scripts/corpus/fetch.py                                        # once: the corpus
cargo run --release -p gbcli-corpus -- --check tests/corpus/baseline-sameboy.json
python3 -I scripts/corpus/docboy_classes.py                               # the classes below, from the baseline
```

`docboy_classes.py` maps every failing docboy row of a baseline to one class below by its id. It exits 1 on a failure no class matches (a new failure to explain) or on a class whose cases all pass now (an explanation to retire). The one-command property of the baseline is unchanged: `--check` re-checks every row, docboy's included.

## The boot ROM

docboy runs its tests with no boot ROM. Its emulator starts from a hard-coded hand-over state: the `if_bootrom_else(...)` reset values in docboy `01e04d3` (`cpu.cpp`, `timers.cpp`, `ppu.cpp`, `apu.cpp`, `interrupts.h`, `hram.cpp`) and the VRAM image in `vram0.cpp`. That state is a real DMG boot ROM's. SameBoy always runs a boot ROM, and the old runner booted SameBoy's open one, which leaves DIV, the PPU and the APU at different phases.

`crates/corpus/roms/docboy_boot.asm` (ours, built by `scripts/build-roms.sh`, committed as `docboy_boot.bin`) gets to docboy's state with four timed writes instead of the logo animation. It sets AF=$01B0, BC=$0013, DE=$00D8, HL=$014D, SP=$FFFE, LCDC=$91, BGP=$FC, NR50/51/52=$77/$F3/$F1, NR12=$F3, IF=$E1 and IE=0. It loads the cartridge's logo and ® into VRAM the way the DMG boot ROM does, and writes docboy's deterministic HRAM bytes ($FFFA–$FFFD from the boot stack, $FFFE=$4C).

| Timed write | M-cycles before the FF50 write | Target | How it was set |
|---|---|---|---|
| DIV reset | 10,994 | DIV's 16-bit counter at $ABCC | Calibrated: mooneye `boot_div-dmgABCmgb` and GBMicrotest `poweron_div_004`/`_005` all pass only at 10,994 (10,992–10,999 swept) |
| LCD on | 17,540 | PPU 395 dots into line 153 | Calibrated: `poweron_ly_119/120`, `poweron_stat_005/006` and `poweron_vram_025/026` all pass only at 17,540 (17,530–17,550 swept), and again a frame earlier (35,096) |
| Channel 1 trigger | 786,432 (0.75 s) | Channel 1 on, faded to volume 0 | Derived: envelope F, decreasing, pace 3 reaches 0 after 45 ticks at 64 Hz (0.70 s) |
| DIV reset, then APU on | 805,613 | SameBoy's frame-sequencer counter at 1 mod 8, which is docboy's `div_apu = 18` (docboy acts on even counts after incrementing, SameBoy on odd ones) | Derived (393 DIV-APU events by hand-over), then checked: docboy's six `ch1_length_timer_delay*` cases pass at every even frame-sequencer step from it and fail at every odd one (steps −4…+3 swept) |

Only docboy rows use it. `judge::run` boots every other row with the model's built-in boot ROM, as before, and no session or core default changed. `judge::run_with_boot` lets a test run a non-docboy row from another boot ROM. `crates/corpus/tests/docboy_boot.rs` pins the hand-over registers, IO, VRAM and HRAM, and runs 12 calibration rows from the suites `cargo test` already needs.

**Calibration result.** docboy's `boot` category (88 cases: its own `boot_*` tests, mooneye's three DMG boot tests, and 62 GBMicrotest `poweron_*` tests) went from 49 failures to 6. The 6 are the power-on OAM seed artifact below; they fail from any boot ROM.

## Results

`tests/corpus/baseline-sameboy.json`, docboy rows (`cargo run --release -p gbcli-corpus -- --suite docboy`, 49.5 s wall on 8 threads): **2,765 pass, 173 fail, 11 skip** (was 2,469 / 469 / 11). 297 cases went from fail to pass and 1 from pass to fail (`stop_trigger_interrupt`, one of the STOP harness cases below).

| docboy category | Cases | Fail, open boot ROM | Fail, docboy's state |
|---|---|---|---|
| ppu | 1,641 | 239 | 114 |
| interrupts | 164 | 73 | 0 |
| boot | 88 | 49 | 6 |
| serial | 78 | 40 | 1 |
| apu | 321 | 24 | 11 |
| oam_bug | 275 | 22 | 22 |
| mbc/mbc3 | 29 | 6 | 6 |
| timers | 78 | 5 | 5 |
| cpu | 46 | 4 | 3 |
| stat_write_bug | 43 | 3 | 2 |
| dma | 74 | 2 | 2 |
| mbc/huc3 | 9 | 1 | 1 |
| integration | 1 | 1 | 0 |
| the other 7 | 107 | 0 | 0 |

## Classification of the 173

| Class | Cases | Evidence |
|---|---|---|
| Harness: STOP wake bug | 19 | below; job `Jwo9sg` |
| Harness: STOP, unexplained | 1 | below |
| Harness: power-on OAM seed | 6 | below |
| Reference problem, probable | 1 | below |
| SameBoy gap, third-party reference | 10 | below |
| Disputed with docboy's own tests | 136 | below |

**Harness: a STOPped core never wakes on a key (19).** These are `stop_read_ly*` (6), `stop_read_stat*` (3), `stop_read_tima*` (2), `stop_tima_falling_edge`, `stop_joypad0_interrupt0/1`, `dma_stop_during_transfer_timing_round1/2`, `stop_trigger_interrupt`, `stop_hello_world` and `mbc3_rtc_gb_stopped_recover`. Each enters STOP and expects a key press scheduled later to wake it; each ends "the CPU stopped with no wake source". The cases where the key is already down when STOP runs (`stop_joypad1_*`) pass. Cause: `gbcli_pre_run` (`crates/sameboy-sys/shim/gbcli_shim.c:276`) reports STOP while JOYP's low nibble is $F, and `run` returns before calling `GB_run`. But `set_keys` (`GB_set_key_mask`) only marks the inputs changed, and SameBoy recomputes JOYP in `GB_joypad_run`, inside `GB_run`. Observed with a scratch test on `stop_joypad0_interrupt0` from docboy's state: STOP at 8,464 ticks, `set_keys(UP)`, JOYP still reads $EF, `run` stops again. After a P1 write that changes the select bits, which calls `GB_update_joyp`, the next `run` proceeds. This is a gbcli-sameboy bug, not a SameBoy gap and not docboy's. It is filed as `Jwo9sg` and not fixed here: the leaf's scope is classification, and the fix changes the backend.

**Harness: STOP, unexplained (1).** `stop_immediate_exit_joypad_interrupt` presses A before STOP and releases it at 131,072 T-cycles, then times out. It is STOP-adjacent but does not end in "stopped", so the bug above does not obviously explain it. Not investigated further.

**Harness: power-on OAM seed (6).** These are `poweron_oam_000`, `_005`, `_070`, `_119`, `_184` and `_233`, the GBMicrotest cases that read $FE35 while OAM is accessible and fail if it reads $FF (the "locked" value). SameBoy fills DMG OAM at power-on with ORs of three random bytes (`gb.c:1573`), and with the runner's seed 0, $FE35 is $FF. With a scratch test on `poweron_oam_000` and `_070`, seeds 1–3 give $EF, $FE, $FE and both tests pass, from either boot ROM. docboy's own OAM table holds $EB there. This is a runner artifact, and it applies equally to the same six rows of the GBMicrotest suite. The baseline note had counted those as boot-ROM dependent; it is corrected in place.

**Reference problem, probable (1).** `cpp/latch-rtc-test` (framebuffer, `limit_speed`). The shootout passes SameBoy and fails docboy on this ROM ([2026-11-06-shootout-failures.md](2026-11-06-shootout-failures.md)), and docboy's case compares against docboy's own `results/dmg/cpp/latch-rtc-test.png`. SameBoy's frame differs from it in the RTC values printed. The likeliest reading is that docboy's reference records docboy's behavior. It stays "probable": the shootout's reference image was not compared here.

**SameBoy gaps against third-party references (10).** The nine Mealybug `m3_*` cases are the shootout's nine DMG-blob failures, against an emulator-made reference ([2026-11-06-sameboy-baseline.md](2026-11-06-sameboy-baseline.md) § The shootout). GBMicrotest `lyc1_int_if_edge_c` fails from both boot ROMs and is already a SameBoy gap in the GBMicrotest suite.

**Disputed with docboy's own tests (136).** The docboy-test-suite README (fetched 2026-11-06 with `curl -sSL https://api.github.com/repos/Docheinstein/docboy-test-suite/readme -H "Accept: application/vnd.github.raw"`) describes the tests as "written to support the accurate development of the DocBoy emulator". It states no hardware verification, so these are disagreements whose truth is unestablished. They are not counted as SameBoy gaps. Two facts rule out the boot state:

- **Every one of the 136 also failed after SameBoy's open boot ROM**, whose DIV, PPU and APU phases differ from docboy's. By contrast, 297 cases flipped between the two boots.
- **Every family is partial: siblings that differ only in a NOP count, an X offset or a round pass.** For example, 14 of 34 `oam_scan_blocked_by_dma` cases fail, 7 of 42 `oam_bug_ld_hl_a`, and 36 of 72 `window_turn_off`.

| Family | Cases | Notes |
|---|---|---|
| `ppu/visual` mid-mode-3 | 77 | `window_turn_off` (36 of 72), `window_turn_off_alt` (24 of 32), `change_bg_tile_data_glitch_sprites` (5 of 5), `change_win_tile_data_glitch_window` (5 of 5), `change_scy` (3 of 16), `window_bg_reprise` (2 of 18), `change_win_map` (1 of 8), `enable_win_during_pixel_transfer` (1 of 3) |
| `ppu/oam_scan_blocked_by_*` | 16 | DMA (14 of 34), CPU write (1 of 2), IDU write (1 of 2) |
| `oam_bug/*` | 22 | across `ld_a_hl`, `ld_a_hli`, `ld_hl_a`, `ld_hli_a`, `pop`, `push`; blargg's `oam_bug` passes on SameBoy |
| `apu/ch1_period_sweep*` | 11 | fail at all eight frame-sequencer phases tried; docboy itself fails SameSuite's `channel_1_sweep_restart_2`, which SameBoy passes (shootout) |
| other | 10 | `stat_write_bug_hblank` (2 of 16), `write_tima_while_reloading_*` (2), `serial_abort_to_81_sc_round1` (1 of 2), `mbc3_rtc_latch_*` (3 of 6), `mbc3_rtc_write_out_of_range_tick`, `huc3_command_5x` |

**The visual cases are not an adapter artifact.** The adapter compares the last complete frame where docboy compares its live buffer, so a capture-timing error would show as a horizontal split. A scratch test re-ran all 77 and diffed SameBoy's frame against docboy's PNG. In 67 cases the difference is 1–16 pixels on one line, at a sprite or the window edge. In the 10 tile-data-glitch cases it is 128–576 pixels in an 8-pixel column. None is 0 pixels and none is a band. The LD B,B cases hit LD B,B at LY 144, where the last complete frame is the one drawn. Their siblings pass with 0 pixels through the same path (for example `window_turn_off_nops22_sprite_x0`). These are the same register-write-during-mode-3 family as SameBoy's Mealybug gap.

## Other findings

- **blargg `oam_bug/rom_singles/7-timing_effect` overflows its own output.** The cause is not the boot state, which the baseline note suspected. The single ROM prints a timing table per case into the $A004 text buffer. With a scratch test sampling every 0.1 s, the text reaches 8,188 bytes (the end of the 8 KiB cart RAM at $BFFF) at 13.0 emulated seconds. Then `WRAM[0..4]` changes from the test's copied code (`c3 e6 c2`, `jp $C2E6`) to text (`20 2d 2d 20`, " -- "), and SP leaves WRAM: the output ran past $BFFF into the code the test runs from $C000. The status byte stays $80. It fails the same way with seeds 0–5 and from docboy's hand-over state. The multi-test ROM runs the same check without printing the table, and reports "07:ok". docboy's config also leaves this ROM out (it lists `1`–`6` and `8`). The header declares 8 KiB (`$0149` = $02), so real hardware would overflow the same way. This is a test-ROM limit, not a SameBoy result.
- **The two manifest problems are fixed** in `scripts/corpus/build_manifest.py` (regenerated; `SHA256SUMS` unchanged). `mgb_oam_dma_halt_sprites` starts an OAM DMA at LY 144 and HALTs with IE=0, so it never reaches LD B,B. It is now judged after 1 emulated second (11,523 pixels differ, the same as the earlier 5-second scratch run, so the frame is static). It still fails: the SameBoy gap, unconfirmed, stands. `oam_bug@cgb` is judged in a new `cgb_rgb555_shift` palette (x << 3, white $F8), the convention its reference was captured in, and now passes. `crates/corpus/tests/manifest_rows.rs` pins both.

## What would change these results

- Fixing `Jwo9sg` should flip some or all of the 19 STOP cases. Re-run `--check`, rewrite the baseline and the counts here.
- A per-row seed, or seeding OAM the way docboy does, would flip the six `poweron_oam` cases in both suites. Deliberately not done: the runner's fixed seed is a determinism choice, and tuning it to pass tests would hide what it does.
- Hardware evidence for any disputed family would move it to "SameBoy gap" or "reference problem".
