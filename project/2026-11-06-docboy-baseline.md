# docboy baseline: docboy's cases from docboy's own hand-over state

2026-11-06. **Finding: run from the post-boot state docboy assumes, SameBoy fails 173 of docboy's 2,949 cases, not 469; 26 of those 173 are runner artifacts, and none of the rest is a boot-state effect.** 136 are disagreements with docboy's own tests, which state no hardware verification; 10 are SameBoy gaps already known from third-party suites. Leaf `5ewkYZ`; it follows the SameBoy baseline ([2026-11-06-sameboy-baseline.md](2026-11-06-sameboy-baseline.md)), which ran docboy's cases after SameBoy's open boot ROM and left them unclassified.

> **Update 2026-11-06 (leaf `Jwo9sg`): 166 fail now, not 173.** The STOP wake bug below is fixed: `gbcli_pre_run` now lets `GB_run` take its STOP step while SameBoy's joypad has unapplied input (`inputs_changed`) or is still settling (`!joypad_is_stable`), so a key pressed in STOP wakes the core as SameBoy would. Of the 19 cases it held, 7 now pass (`stop_joypad0_interrupt0/1`, `stop_hello_world`, `stop_read_tima_round1/2`, `stop_tima_falling_edge`, `dma_stop_during_transfer_timing_round2`). The other 12 now wake and then write their own fail value ($02 at $FFF0); they are reclassified "Unexplained: after STOP" below. So 7 of the 166 are runner artifacts (6 power-on OAM seed, 1 STOP unexplained), not 26. The counts in the tables below are updated in place; the 2,765 / 173 line is the original result.

> **Update 2026-11-06 (leaf `RU4MNT`): the 12 "after STOP" cases are classified; 1 is ours, 11 are disputed.** Measured by letting SameBoy step through STOP itself (`gbcli_pre_run` never returning `GBCLI_PRE_STOPPED`, so emulated time passes until the key, as in SameBoy's own frontend) and re-running `--suite docboy --id stop`: `mbc3_rtc_gb_stopped_recover` passes, and the other 11 fail exactly as before. So one case is a runner artifact (no emulated time, hence no RTC time, passes in STOP) and the clock skip plays no part in the other 11, which disagree with docboy under SameBoy's own STOP behavior. Details under "Harness: no time passes in STOP" and "Disputed: docboy after STOP" below. Not fixed: letting time pass in STOP needs the core to run through it, a `Core` contract change queued for a decision. Counts: runner artifacts are 8 of the 166 (6 power-on OAM seed, 1 STOP unexplained, 1 no time in STOP); disputed are 147.

> **Update 2026-11-06 (leaf `34Nx7n`): 165 fail now, not 166; runner artifacts are 7.** The core contract changed so emulated time passes in STOP (design §2, `InStop::PassTime`), and the runner passes time in STOP while a scheduled input is pending instead of skipping its clock (`Driver::idle_until` is gone). `mbc3_rtc_gb_stopped_recover` passes, as the RU4MNT measurement predicted. `cargo run --release -p gbcli-corpus -- --check tests/corpus/baseline-sameboy.json` (97.8 s on 8 threads) changed exactly that row, fail → pass, and the baseline was rewritten with `--write`. The other 11 STOP cases still fail as classified below; the clock skip played no part in them.

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
| ppu | 1,641 | 239 | 114 → 113 |
| interrupts | 164 | 73 | 0 |
| boot | 88 | 49 | 6 |
| serial | 78 | 40 | 1 |
| apu | 321 | 24 | 11 |
| oam_bug | 275 | 22 | 22 |
| mbc/mbc3 | 29 | 6 | 6 |
| timers | 78 | 5 | 5 → 2 |
| cpu | 46 | 4 | 3 → 1 |
| stat_write_bug | 43 | 3 | 2 |
| dma | 74 | 2 | 2 → 1 |
| mbc/huc3 | 9 | 1 | 1 |
| integration | 1 | 1 | 0 |
| the other 7 | 107 | 0 | 0 |

## Classification of the 173

| Class | Cases | Evidence |
|---|---|---|
| ~~Harness: STOP wake bug~~ | ~~19~~ 0 | fixed, job `Jwo9sg`: 7 now pass, 12 moved to the next row |
| ~~Unexplained: after STOP~~ | ~~12~~ 0 | classified, job `RU4MNT`: split into the next two rows |
| ~~Harness: no time passes in STOP~~ | ~~1~~ 0 | fixed, job `34Nx7n`: passes |
| Disputed: docboy after STOP | 11 | below |
| Harness: STOP, unexplained | 1 | below |
| Harness: power-on OAM seed | 6 | below |
| Reference problem, probable | 1 | below |
| SameBoy gap, third-party reference | 10 | below |
| Disputed with docboy's own tests | 136 | below |

> **Fixed 2026-11-06 (leaf `Jwo9sg`).** The cause below was right; the fix is in `gbcli_pre_run`, and 7 of the 19 now pass. See the update at the top and the next paragraph.

**Harness: a STOPped core never wakes on a key (19).** These are `stop_read_ly*` (6), `stop_read_stat*` (3), `stop_read_tima*` (2), `stop_tima_falling_edge`, `stop_joypad0_interrupt0/1`, `dma_stop_during_transfer_timing_round1/2`, `stop_trigger_interrupt`, `stop_hello_world` and `mbc3_rtc_gb_stopped_recover`. Each enters STOP and expects a key press scheduled later to wake it; each ends "the CPU stopped with no wake source". The cases where the key is already down when STOP runs (`stop_joypad1_*`) pass. Cause: `gbcli_pre_run` (`crates/sameboy-sys/shim/gbcli_shim.c:276`) reports STOP while JOYP's low nibble is $F, and `run` returns before calling `GB_run`. But `set_keys` (`GB_set_key_mask`) only marks the inputs changed, and SameBoy recomputes JOYP in `GB_joypad_run`, inside `GB_run`. Observed with a scratch test on `stop_joypad0_interrupt0` from docboy's state: STOP at 8,464 ticks, `set_keys(UP)`, JOYP still reads $EF, `run` stops again. After a P1 write that changes the select bits, which calls `GB_update_joyp`, the next `run` proceeds. This is a gbcli-sameboy bug, not a SameBoy gap and not docboy's. It is filed as `Jwo9sg` and not fixed here: the leaf's scope is classification, and the fix changes the backend.

> **Corrected 2026-11-06 (leaf `RU4MNT`):** the paragraph below left these unexplained and guessed at the RTC case; both are now measured, in the two paragraphs after it. The guess was right for `mbc3_rtc_gb_stopped_recover` only.

**Unexplained: after STOP (12).** `stop_read_ly*` (6), `stop_read_stat*` (3), `stop_trigger_interrupt`, `dma_stop_during_transfer_timing_round1` and `mbc3_rtc_gb_stopped_recover`. With the wake fixed they leave STOP on the scheduled key and then write $02 to $FFF0, the test's own fail value. Not investigated. Two facts bound it. The runner skips the judge's clock to the next scheduled input while the core sits in STOP (`Drive::idle_until`, `crates/corpus/src/drive.rs`), so no emulated time passes inside STOP. For LY and STAT that matches SameBoy, whose DMG PPU does not advance in STOP (`display.c`, "The PPU does not advance while in STOP mode on the DMG"), and timers also stop in STOP (`timers_run`), which fits the three TIMA cases now passing. SameBoy's RTC does run in STOP (`rtc_run`), so the skip may explain `mbc3_rtc_gb_stopped_recover`; that is a guess, not a measurement. docboy's sources are not in the corpus, so what each case expects is unknown here.

**Harness: no time passes in STOP (1).** `mbc3_rtc_gb_stopped_recover`. The runner skips its judge clock to the next input while the core reports `Stopped` (`Drive::idle_until`), and the core runs no cycles in STOP, so the cartridge RTC, which SameBoy advances with emulated cycles (`rtc_run`), loses the whole wait. Measured: with SameBoy stepping through STOP until the key (the experiment in the update at the top), the case passes; with the skip it writes $02. It is ours, not SameBoy's. The fix needs a core that can run through STOP with time passing (for example a run option the corpus runner sets while an input is pending), which changes the `Core` contract's "no time passes in STOP" (`crates/sameboy/tests/run.rs`); it is queued as a decision rather than made here.

**Disputed: docboy after STOP (11).** Under SameBoy's own STOP behavior, with the key pressed when docboy schedules it, each reads a value docboy's test rejects. Measured with a scratch probe (from docboy's hand-over state, stepping through STOP, the value in A after the test's read):

- `stop_read_ly*` (6): LCD on, a delay loop, STOP at line 20 (`_alt_alt`: line 30), then a NOP sled and `ldh a,[LY]`. Probed: `stop_read_ly_round1` and `_round2` read 21 where docboy expects 0 and 1, and `_alt_alt_round1` reads 31 where docboy expects 10 (the other three, which expect 0/1 and 11, were not probed). With the key at 20,000, 40,000, 65,536 or 100,000 T-cycles, `stop_read_ly_round1` reads 21 every time: SameBoy's DMG PPU does not advance in STOP (its `display.c` says so), so no key timing the runner could choose changes the value.
- `stop_read_stat_round1/2/3` (3): the same setup reading STAT. Probed in round 1 only: SameBoy reads $83 (mode 3) where docboy expects $85; rounds 2 and 3 expect $84 and $86.
- `stop_trigger_interrupt` (1): STAT's LYC interrupt is armed for line 30, STOP comes at line 20, and a 16-NOP sled after STOP falls through to the fail path unless the interrupt has fired. SameBoy falls through: from line 20, LY can't reach 30 within the sled unless the PPU moved during STOP.
- `dma_stop_during_transfer_timing_round1` (1): OAM DMA starts, STOP comes on the next instruction, and after the wake a delay loop reads OAM $FE00. docboy expects $FF (DMA still running) in round 1 and the copied byte one M-cycle later in round 2. SameBoy reads the copied byte ($24) in both (the DMA source is $24/$2C throughout, never $00), so round 2 passes. While stopped, OAM's last byte was still $00 and its first already copied, so SameBoy's DMA also pauses in STOP; it finishes at least an M-cycle sooner after the wake than docboy expects. Which part (wake latency or DMA) differs is not measured. Every key time from 65,530 to 65,541 T-cycles gave the same read, so the runner's input timing is not the cause.

As with the 136 below, docboy states no hardware verification, so these are disagreements, not SameBoy gaps. `stop_immediate_exit_joypad_interrupt` ("Harness: STOP, unexplained") also times out when SameBoy steps through STOP, so the clock skip doesn't explain it either.

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

> **Update, 2026-11-07 (leaf OU9rOL): the four MBC3 rows are SameBoy gaps, measured.** Re-checked with `cargo run --release -p gbcli-corpus -- --id mbc --id rtc --check tests/corpus/baseline-sameboy.json` (252 runs, no difference from the baseline: mooneye's MBC1, MBC1 multicart, MBC2 and MBC5 rows pass on all six models; docboy fails 6). (1) **SameBoy latches the MBC3 clock on every write to $6000–$7FFF** (`Core/memory.c`, the `GB_MBC3` case), where hardware latches only when 1 follows 0, so `mbc3_rtc_latch_0_to_2`, `_1_to_0` and `_1_to_1` latch when they must not; `0_to_1`, `0_to_3` and `4_to_3` pass. (2) **SameBoy counts an out-of-range clock register on past its bit width** (`GB_rtc_set_time` tests `++seconds != 60`, and reads mask the latched copy), where hardware wraps 63 to 0 without a carry (Pan Docs' MBC3 notes; the test's name), which is what `mbc3_rtc_write_out_of_range_tick` exercises (the ROM's source isn't in the corpus, so its exact check wasn't read); measured: seconds set to 63, then 61 seconds on, SameBoy reads 0:00:60 where that rule gives 0:01:01. Both are pinned as SameBoy behaves by `sameboy_latches_the_mbc3_clock_on_any_write_not_only_0_then_1` and `sameboy_counts_an_out_of_range_second_on_past_its_six_bits` (`crates/sameboy/tests/mappers.rs`), so a SameBoy update that fixes either shows there. A game that latches with 0 then 1 and never writes an out-of-range value, as Nintendo's guidance has it, sees neither. `huc3_command_5x` (HuC3, not a must-have) is still unexplained.

**The visual cases are not an adapter artifact.** The adapter compares the last complete frame where docboy compares its live buffer, so a capture-timing error would show as a horizontal split. A scratch test re-ran all 77 and diffed SameBoy's frame against docboy's PNG. In 67 cases the difference is 1–16 pixels on one line, at a sprite or the window edge. In the 10 tile-data-glitch cases it is 128–576 pixels in an 8-pixel column. None is 0 pixels and none is a band. The LD B,B cases hit LD B,B at LY 144, where the last complete frame is the one drawn. Their siblings pass with 0 pixels through the same path (for example `window_turn_off_nops22_sprite_x0`). These are the same register-write-during-mode-3 family as SameBoy's Mealybug gap.

## Other findings

- **blargg `oam_bug/rom_singles/7-timing_effect` overflows its own output.** The cause is not the boot state, which the baseline note suspected. The single ROM prints a timing table per case into the $A004 text buffer. With a scratch test sampling every 0.1 s, the text reaches 8,188 bytes (the end of the 8 KiB cart RAM at $BFFF) at 13.0 emulated seconds. Then `WRAM[0..4]` changes from the test's copied code (`c3 e6 c2`, `jp $C2E6`) to text (`20 2d 2d 20`, " -- "), and SP leaves WRAM: the output ran past $BFFF into the code the test runs from $C000. The status byte stays $80. It fails the same way with seeds 0–5 and from docboy's hand-over state. The multi-test ROM runs the same check without printing the table, and reports "07:ok". docboy's config also leaves this ROM out (it lists `1`–`6` and `8`). The header declares 8 KiB (`$0149` = $02), so real hardware would overflow the same way. This is a test-ROM limit, not a SameBoy result.
- **The two manifest problems are fixed** in `scripts/corpus/build_manifest.py` (regenerated; `SHA256SUMS` unchanged). `mgb_oam_dma_halt_sprites` starts an OAM DMA at LY 144 and HALTs with IE=0, so it never reaches LD B,B. It is now judged after 1 emulated second (11,523 pixels differ, the same as the earlier 5-second scratch run, so the frame is static). It still fails: the SameBoy gap, unconfirmed, stands. `oam_bug@cgb` is judged in a new `cgb_rgb555_shift` palette (x << 3, white $F8), the convention its reference was captured in, and now passes. `crates/corpus/tests/manifest_rows.rs` pins both.

## What would change these results

- ~~Fixing `Jwo9sg` should flip some or all of the 19 STOP cases.~~ Done: 7 flipped (see the update at the top).
- ~~Letting emulated time pass in STOP (a core that runs through STOP until a budget, rather than stopping with `Stopped`) would test the RTC guess above.~~ Tested (job `RU4MNT`): it flips `mbc3_rtc_gb_stopped_recover` and nothing else. Shipping it needs the `Core` change described under "Harness: no time passes in STOP".
- A per-row seed, or seeding OAM the way docboy does, would flip the six `poweron_oam` cases in both suites. Deliberately not done: the runner's fixed seed is a determinism choice, and tuning it to pass tests would hide what it does.
- Hardware evidence for any disputed family would move it to "SameBoy gap" or "reference problem".
