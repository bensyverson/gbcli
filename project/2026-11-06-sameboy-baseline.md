# SameBoy baseline: the test-ROM corpus through the core trait

2026-11-06. **Finding: SameBoy passes every mooneye and blargg test its authors claim, except the ones that depend on the boot ROM — and those fail because gbcli runs SameBoy's open boot ROMs, not Nintendo's.** Every other difference is explained below as boot-ROM dependent, a revision mismatch, a reference-image problem, a manifest problem, a runner limit, or a SameBoy gap (recorded, not fixed). Leaf `rInSEQ`. The corpus and its pass rules: [2026-11-06-test-rom-corpus.md](2026-11-06-test-rom-corpus.md).

> **Updated 2026-11-06 (leaf `5ewkYZ`):** docboy's cases now run from docboy's own post-boot state and are classified case by case in [2026-11-06-docboy-baseline.md](2026-11-06-docboy-baseline.md): 173 fail, not 469, and none of the 173 is a boot-state effect. Two causes below were wrong and are corrected in place: six GBMicrotest `poweron_oam_*` failures are a power-on OAM seed artifact, not boot-ROM dependent, and blargg's `7-timing_effect` overflows its own output buffer rather than depending on boot state. The two manifest problems are fixed.

> **Updated 2026-11-06 (leaf `Jwo9sg`):** the STOP-wake bug in gbcli-sameboy is fixed, and the baseline was rewritten by the same command (`cargo run --release -p gbcli-corpus -- --write tests/corpus/baseline-sameboy.json`): **4,491 runs: 4,126 pass, 317 fail, 48 skip**, docboy 2,772 / 166 / 11. The only status changes are 7 docboy STOP cases, fail → pass; the other 12 of the 19 now wake and fail their own check. Details in [2026-11-06-docboy-baseline.md](2026-11-06-docboy-baseline.md).

> **Updated 2026-11-06 (leaf `34Nx7n`):** emulated time now passes in STOP (design §2), and the runner lets it pass while a scheduled input is pending. `--check` changed one row, docboy's `mbc3_rtc_gb_stopped_recover` on dmg-B, fail → pass; rewritten with `--write` (72.9 s on 8 threads): **4,491 runs: 4,127 pass, 316 fail, 48 skip**, docboy 2,773 / 165 / 11.

## Re-running it

```
python3 -I scripts/corpus/fetch.py                                     # once: the corpus (~190 MB)
cargo run --release -p gbcli-corpus -- --check tests/corpus/baseline-sameboy.json
```

`--check` exits 1 and prints one `changed:` line per row and model whose status differs from the committed baseline. `--write tests/corpus/baseline-sameboy.json` regenerates it (header: backend, SameBoy commit, date, the command). `--suite`, `--skip-suite` and `--id` narrow a run, `--jobs` sets threads, and `--dump DIR` writes the frame of every failing screenshot row as a PNG. Run it with the sandbox off (cargo's registry), and detached if your harness kills commands after 10 minutes (Claude Code does; see Runtime).

`cargo test -p gbcli-corpus` also runs four rows end to end (`crates/corpus/tests/smoke.rs`: a mooneye, a blargg serial, a GBMicrotest and a dmg-acid2 row), which needs those suites fetched.

## What the runner does

`crates/corpus` (`gbcli-corpus`, a library plus a binary) reads `tests/corpus/manifest.json` into typed structs and runs each row on every model the row lists that SameBoy has, through `dyn Core` only: the binary passes a SameBoy constructor in, nothing else names the backend. Threads: `std::thread::available_parallelism()` workers, each building its own cores.

- **Models.** A family with no revisions runs on SameBoy's default (DMG-B, CGB-E, AGB-A, MGB, SGB, SGB2). Listed revisions run once each where SameBoy has them (all CGB revisions); where it has none of them (DMG-0, DMG-C, AGB revisions other than A) the row runs on the family default and the result carries `substitute_for` (e.g. `"C"`).
- **Boot.** Every run boots SameBoy's built-in open boot ROM to the hand-over at $0100; exits and timeouts count from there (c-sp's and docboy's timings assume a ROM start, and docboy runs with no boot ROM at all).

  > **Changed 2026-11-06 (leaf `5ewkYZ`):** docboy rows now boot `crates/corpus/roms/docboy_boot.asm`, a test-only boot ROM that hands over in docboy's post-boot state ([2026-11-06-docboy-baseline.md](2026-11-06-docboy-baseline.md) § The boot ROM). Every other row still boots the built-in open boot ROM.
- **Exits.** `ld_b_b`: an execution filter over every ROM address holding $40 (bank-exact) plus every RAM address holding $40 now, rescanned once per frame; a hit whose opcode is $40 is the exit. `emulated_seconds`/`emulated_frames`: a tick budget (a frame is 70,224 dots). `result_reported`: polled every 0.1 emulated s. A null `timeout_s` means 30 emulated seconds.
- **Serial** needs no backend change: a write filter on SC ($FF02) stops the run after any write that starts an internally clocked transfer, and SB still holds the outgoing byte then.
- **Rules.** `fibonacci_registers` reads B–L at the exit; `blargg_memory` reads cart RAM; `memory_byte` reads HRAM; `framebuffer` renders the last complete frame in the row's palette (DMG shades as $FF/$AA/$55/$00, CGB RGB as SameBoy emits it with color correction off, i.e. `(x << 3) | (x >> 2)`) and compares exactly. A frame that differs only by a one-to-one recoloring is reported as such. `pass_alternatives` are judged on the same run and recorded beside the verdict. `manual` rows are skipped with their reason.
- **docboy cases** run through an adapter (`docboy.rs`) that follows docboy's `runners.h`: checks every 100,000 T-cycles (memory at once, frames at the last complete frame rather than docboy's next vblank), `check_at_tick`, `check_at_instruction` (LD B,B), `max_ticks` (default 100M T-cycles), joypad `inputs` at ticks, docboy's default and `grey` appearances in RGB565 including the LCD-off color, and exact serial-buffer equality. Not adapted, skipped with the reason: the two linked-Game-Boy cases (`rom2`; the trait has no link cable), inputs triggered by an instruction, and the one case docboy disables. `limit_speed` (real-time RTC) needs nothing: SameBoy's RTC already follows emulated time.

## Results

`tests/corpus/baseline-sameboy.json`, written 2026-11-06 by `cargo run --release -p gbcli-corpus -- --write tests/corpus/baseline-sameboy.json` against SameBoy `ea667ad` (v1.0.5): **4,491 runs of 3,855 rows: 3,822 pass, 621 fail, 48 skip.**

> **Superseded 2026-11-06 (leaf `5ewkYZ`):** rewritten with the docboy boot ROM and the two manifest fixes by the same command (header date 2026-11-07, UTC): **4,491 runs: 4,119 pass, 324 fail, 48 skip.** Two rows of the table changed: blargg 93 / 2 / 0 (`oam_bug@cgb` passes in its reference's palette) and docboy 2,765 / 173 / 11. The 299 status changes against the old file are exactly those: 297 docboy cases fail → pass, `docboy/.../stop_trigger_interrupt` pass → fail, and `oam_bug@cgb` fail → pass.

| Suite | Pass | Fail | Skip |
|---|---|---|---|
| age | 95 | 24 | 0 |
| blargg | 92 | 3 | 0 |
| bully | 0 | 2 | 0 |
| cgb-acid-hell | 1 | 0 | 0 |
| cgb-acid2 | 1 | 0 | 0 |
| dmg-acid2 | 2 | 0 | 0 |
| docboy | 2,469 | 469 | 11 |
| gbmicrotest | 404 | 78 | 31 |
| mealybug | 52 | 21 | 6 |
| mooneye | 538 | 21 | 0 |
| samesuite | 166 | 3 | 0 |
| strikethrough | 2 | 0 | 0 |

The same totals came out of two full runs with different runner revisions in between (the second only fixed serial capture, which changes alternatives, not verdicts), and the results are deterministic: fixed seed, emulated time only.

## Calibration against SameBoy's published claims

The claim under test, as this leaf's brief states it: all of blargg's and mooneye's tests pass on SameBoy, except tests that need Nintendo's boot ROMs (not re-read from SameBoy's own site for this note). The shootout (stamped 2026-06-04, inferred SameBoy v1.0.3, Nintendo boot ROMs) is the other yardstick: [2026-11-06-shootout-failures.md](2026-11-06-shootout-failures.md).

### mooneye (114 rows, 559 runs): 20 boot-ROM failures, 1 madness test

- **Boot-ROM dependent (19):** `boot_div-dmgABCmgb` (DMG-B, MGB), `boot_div-S` and `boot_div2-S` (SGB, SGB2), `boot_hwio-dmgABCmgb` (DMG-B, MGB), `serial/boot_sclk_align-dmgABCmgb` (DMG-B, MGB), `misc/boot_div-A` (AGB-A), `misc/boot_div-cgbABCDE` (CGB-A..E), `misc/boot_div-cgb0` (CGB-0). Each checks DIV, IO registers or the serial clock phase at hand-over, which is set by how long the boot ROM runs; SameBoy's open boot ROMs don't reproduce Nintendo's timing. They report failure the mooneye way (all registers $42), so the runner read them correctly. `boot_regs-dmgABC`, `-mgb`, `-sgb`, `-sgb2` and `boot_hwio-S` pass: the open boot ROMs get the registers right, not the timing.
- **Revision mismatch (3):** `boot_div-dmg0`, `boot_hwio-dmg0` and `boot_regs-dmg0` want DMG-0, which SameBoy doesn't emulate; they ran on DMG-B as substitutes (and are boot-ROM dependent as well).
- **`madness/mgb_oam_dma_halt_sprites` (MGB):** never reaches LD B,B in 720 emulated seconds, so the manifest's `ld_b_b` exit looks wrong for it (**manifest problem, reported, not fixed**). Judged instead after 5 s (a scratch manifest), the frame differs from c-sp's reference in 11,523 pixels: the checkerboard's phase differs and the sprite is missing. A "madness" test is outside the acceptance set SameBoy's claim covers; recorded as a **SameBoy gap, unconfirmed**, since the reference is c-sp's and unverified.

  > **Fixed 2026-11-06 (leaf `5ewkYZ`):** the ROM starts an OAM DMA at LY 144 and HALTs with IE=0, so it can never reach LD B,B. The manifest now judges it after 1 emulated second (`build_manifest.py`). The frame differs in the same 11,523 pixels, so the SameBoy gap, unconfirmed, stands.

Every other mooneye run passes, including all 81 acceptance tests on all six families.

### blargg (65 rows, 95 runs): 3 failures, none a SameBoy gap

- **`oam_bug/oam_bug.gb@dmg` — reference-image problem.** Our frame shows the same text as the reference ("Passed", 01–08 ok, checked visually with `--dump`) and the `blargg_memory` alternative passes; the reference's last row holds 50 near-white pixels ($FE/$FD instead of $FF), an artifact of the image, not of any emulator.
- **`oam_bug/oam_bug.gb@cgb` — reference/palette problem.** Shape identical, colors not: c-sp's reference uses $F8 white, i.e. `x << 3` without the `| x >> 2` the manifest's `cgb_rgb555` palette (and c-sp's own howto) prescribes. The runner reports "differs only in colors".
  > **Fixed 2026-11-06 (leaf `5ewkYZ`):** the manifest now judges this row in a `cgb_rgb555_shift` palette (x << 3), and it passes.
- **`oam_bug/rom_singles/7-timing_effect.gb` (DMG) — unexplained, boot state suspected.** The result byte stays at $80 (running) with the signature in place, still after 126 emulated s (a scratch manifest with 6× the bound). The same sub-test passes inside the multi-test `oam_bug.gb` ("07:ok") on the same build, so SameBoy emulates the behavior; the single ROM starting straight from the open boot ROM is the likeliest difference. Not settled.
  > **Explained 2026-11-06 (leaf `5ewkYZ`): the suspected cause was wrong.** It fails identically from docboy's DMG hand-over state and with seeds 0–5. The single ROM prints a timing table into the $A004 text buffer, which fills cart RAM to $BFFF at 13.0 emulated seconds and then runs on into $C000, overwriting the code the test runs from WRAM. The multi-test ROM doesn't print the table. A test-ROM limit, not a SameBoy result: [2026-11-06-docboy-baseline.md](2026-11-06-docboy-baseline.md) § Other findings.

All `cpu_instrs`, `instr_timing`, `mem_timing`, `mem_timing-2`, `halt_bug`, `interrupt_time`, `dmg_sound` and `cgb_sound` rows pass, on every listed model.

### The shootout, row by row

- **The nine Mealybug DMG failures persist at SameBoy v1.0.5** (`ea667ad`): the same nine `@dmg_blob` rows fail here (`m3_lcdc_bg_en_change`, `_bg_map_change`, `_obj_size_change`, `_obj_size_change_scx`, `_tile_sel_change`, `_tile_sel_win_change`, `_win_en_change_multiple_wx`, `_win_map_change`, `m3_scy_change`), and the other 15 DMG-blob rows pass, as on the shootout.
- **SameBoy does not match the Mealybug DMG-B references either**: `m3_lcdc_bg_en_change@dmg_b` differs in 4 pixels (232 against the blob reference) and `m3_lcdc_win_en_change_multiple_wx@dmg_b` in 40 (five rows of one 8-pixel tile drawn black where the reference is light gray; 43 against blob). This answers the shootout doc's open question: on these two tests, SameBoy's DMG-B is wrong by DMG-B's own reference, not merely by the blob one — with the caveat that the DMG-B references are also from Matt Currie's emulator, checked against photos.
- The shootout's SGB failures aren't in this corpus (`sgb/command_mlt_req` is a SameSuite row SameBoy now passes on SGB; see SameSuite below).

## Beyond the claims

### Mealybug on CGB (10 runs fail), not on the shootout

The shootout runs Mealybug on DMG only, so there is no published SameBoy result for the `@cgb_c` and `@cgb_d` rows. Here 10 of 47 fail on the matching revision (37 pass, 4 skipped): `m3_lcdc_obj_size_change_scx` (C, D: 15 px), `m3_lcdc_tile_sel_change` (C: 8 px; D: 520 px), `m3_lcdc_tile_sel_change2` (C: 186), `m3_lcdc_tile_sel_win_change` (C: 8; D: 260), `m3_lcdc_tile_sel_win_change2` (C: 179), `m3_wx_4_change_sprites` (C, D: 10). Running all CGB rows on CGB-E instead (a scratch manifest) made it worse (18 of 47 fail), so this isn't a wrong revision. The family matches the DMG gap (mid-mode-3 LCDC tile-data and OBJ-size writes), so these are recorded as **SameBoy gaps against an emulator-made reference**, the same caveat as on DMG. The 6 skipped Mealybug runs are the 4 `manual` rows with no reference.

### Compatibility-mode colors (open question 1): settled for SameBoy

Of the 58 runs judged in `cgb_compat_csp`, 45 pass exactly (dmg-acid2, `sprite_priority`, most Mealybug CGB rows) and the 13 that fail differ in pixels, not colors: none is a recoloring. **SameBoy's open CGB boot ROM picks the same compatibility palette c-sp's references assume** for these ROMs.

### AGE (51 rows, 119 runs): 24 failures

- **Revision mismatch (4):** `m3-bg-bgp@dmgC` (2 pixels), `oam/oam-read-dmgC-cgbBC` and `oam/oam-write-dmgC` on DMG-B standing in for DMG-C; AGE's README says the tests were verified on DMG-C only. Not evidence against SameBoy's DMG-B.
- **SameBoy gaps on CGB, per AGE's hardware-verified expectations (20):** `lcd-align-ly-cgbBC`/`-cgbE`, `m3-bg-lcdc@cgbBCE` and `-nocgb@ncmBCE` (512 px), `speed-switch/spsw-tima-cgbBC`/`-cgbE`, `spsw-ch2-lc-delay-cgbBCE`, `caution/spsw-interrupts-cgbBC`, `stat-mode-dmgC-cgbBC` and `stat-mode-ncmBC` (CGB-B and C). AGE's failure registers (`00 6b 14 06 98 10`) are not mooneye's $42, which the runner treats as an ordinary failure, as AGE's howto says ("failure is indicated by any different values").

### SameSuite (78 rows, 169 runs): 3 failures

`apu/channel_3/channel_3_extra_length_clocking-cgbB` (CGB-B) and `apu/channel_4/channel_4_extra_length_clocking-cgb0B` (CGB-0, CGB-B): revision-specific APU behavior SameBoy doesn't model on those revisions. Recorded as **SameBoy gaps** (SameSuite is SameBoy's own suite; the shootout runs it on one model). `sgb/command_mlt_req` passes on SGB at v1.0.5, which the shootout doc guessed.

### Bully (2 runs): boot-ROM dependent

Both fail with "Invalid initial DIV" on screen (`--dump`): DIV at hand-over, set by the open boot ROM. Strikethrough, dmg-acid2, cgb-acid2 and cgb-acid-hell pass.

### GBMicrotest (513 rows): 78 failures, 31 skipped

- **Boot-ROM dependent (35):** every failing `poweron_*` test (DIV, LY, OAM, STAT, VRAM at fixed cycles after hand-over), which measure where the boot ROM leaves the PPU and timer.
  > **Corrected 2026-11-06 (leaf `5ewkYZ`): 29, not 35.** Six of them — `poweron_oam_000`, `_005`, `_070`, `_119`, `_184`, `_233` — read $FE35 while OAM is accessible, and SameBoy's power-on OAM randomization leaves $FF there under the runner's seed 0. With seeds 1–3 they pass from either boot ROM. They are a runner artifact, not boot-ROM dependent. The other 29 pass from docboy's hand-over state ([2026-11-06-docboy-baseline.md](2026-11-06-docboy-baseline.md)).
- **SameBoy gaps (43):** `hblank_int_scx{0..7}_if_{b,c,d}` and `_nops_{a,b}` (HBlank STAT interrupt timing against SCX), `line_153_lyc_b`, `line_65_ly`, `lyc1_int_if_edge_c`, `halt_op_dupe_delay`, `stat_write_glitch_l154_d`. Checked on a DMG-CPU-08 board (CPU B or C) per upstream, so DMG-B should match.
- 31 skipped: the `manual` rows with no $FF82 write. **Calibration (open question 3):** of the 50 uncalibrated rows (not in docboy's set), 47 pass and 3 fail with the test's own failure value ($FF); none ended with "no result", so the static $FF82 scan picked real tests.

### docboy (2,949 rows)

2,469 pass, 469 fail, 11 skipped (2 linked-Game-Boy cases, 8 instruction-triggered inputs, 1 disabled). **Not classified case by case; the adapter is not yet calibrated well enough to call these SameBoy gaps.** What is known:

> **Superseded 2026-11-06 (leaf `5ewkYZ`):** classified in [2026-11-06-docboy-baseline.md](2026-11-06-docboy-baseline.md). From docboy's own hand-over state, 173 fail: 26 runner artifacts (19 from a STOP-wake bug in gbcli-sameboy, 6 from the OAM seed, 1 unexplained STOP case), 1 probable reference problem, 10 SameBoy gaps already known from Mealybug and GBMicrotest, and 136 disagreements with docboy's own tests. The list below is the state before that.

- Where docboy's set overlaps ours, the verdicts agree: its mooneye `boot_div`/`boot_hwio`/`boot_sclk_align`, `hacktix/bully` and `whichboot` fail (boot-ROM dependent, as above), its 9 Mealybug failures are the DMG-blob nine, and its GBMicrotest failures (75) are the `poweron_*` and `hblank_int_*` families above.
- **docboy runs its tests with no boot ROM** and its own post-boot state; we run SameBoy's open boot ROM first, so every docboy test that depends on DIV phase or PPU alignment at hand-over is suspect. 12 `docboy/boot` cases fail for exactly that reason; the 282 cases that report docboy's own failure value ($02 at $FFF0) are concentrated in `ppu` (190 failures in all), `interrupts` (73) and `serial` (39), timing families where hand-over alignment matters.
- The adapter differs from docboy's runner in three places: frames are compared at the last complete frame (docboy: its live buffer at the next vblank, or at the instant for `check_at_tick`/`check_at_instruction`), so the 62 "expectation not met at LD B,B" frame cases may be partly the adapter's; a core in STOP is advanced to the next scheduled input (18 interactive cases still end "stopped with no wake source"); and `cpp/latch-rtc-test`, which the shootout says SameBoy passes, fails here and is unexplained.
- Next step to classify them: run docboy's cases from a no-boot-ROM start (a boot ROM that only sets docboy's post-boot registers, via `BootRom::Custom`), which separates boot-state failures from emulation ones.

## Open questions from the corpus doc

1. **Compatibility-mode colors:** settled above — SameBoy's open CGB boot ROM matches `cgb_compat_csp`.
2. **DMG revision:** SameBoy matches neither DMG-blob nor DMG-B on the two Mealybug tests that have both; on the seven blob-only ones, unknown. AGE's DMG-C-only tests can't judge DMG-B.
3. **Blargg output choice:** calibrated by running: every `serial_text` row produced text and every `blargg_memory` row wrote the signature. `halt_bug` and `interrupt_time` declare no cart RAM, and indeed their `blargg_memory` alternatives get nothing ("the cartridge has no RAM") while their screenshots pass: the corpus doc's suspicion holds, and those alternatives can never pass. Apart from those, every alternative agrees with its row's verdict except `oam_bug@dmg` (reference artifact, above). Mooneye's `serial_bytes` alternative agrees on every passing row — after a runner bug was fixed: the first version read SB after the SC write, by which time SameBoy had sometimes shifted a bit out (`03 05 11 1b 2b 45` for `03 05 08 0d 15 22`); the runner now sends the last byte written to SB.

## Runtime

Full corpus, 8 threads, Apple M1 with two other agents building: **311 s wall** (1,177 s CPU) for 4,491 runs, measured with `/usr/bin/time -p ./target/release/gbcli-corpus --write tests/corpus/baseline-sameboy.json`; an earlier identical run under heavier load took 557 s. (2026-11-06, leaf `5ewkYZ`: the same totals re-ran in 202.9 s at `32b8666`, and the rewritten baseline ran in 135.5 s, both as printed by the runner itself, because docboy's failing cases, which run to `max_ticks`, fell from 469 to 173.) Without docboy, about 30 s for gbmicrotest, Mealybug, AGE and Bully together and under 4 minutes for the rest (blargg's 55-second screenshot rows dominate). Most of it is docboy's failing cases, which run to `max_ticks` (23.8 emulated s each). Two findings from getting there:

- **An LD B,B filter over all of RAM made code running from RAM pay one run per instruction**, which kept one 720-emulated-second run going for over 13 minutes before it was killed (28 s now). The filter now holds only RAM bytes that are $40, rescanned each frame.
- **Claude Code kills a background command after 10 minutes** (`time: command terminated abnormally` at 595 s), so the full run is started detached (`nohup … & disown`).

## What would change these results

- Nintendo's boot ROMs (`BootRom::Custom`) would turn the boot-ROM rows into a real test of SameBoy; gbcli ships open ones by design.
- A manifest fix for `mgb_oam_dma_halt_sprites`'s exit and for `oam_bug@cgb`'s reference palette (both reported to the corpus leaf's owner; the manifest is generated by `scripts/corpus/build_manifest.py` and was not edited here).
  > **Done 2026-11-06 (leaf `5ewkYZ`).**
