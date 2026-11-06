# Shootout failures: SameBoy and docboy, test by test

2026-11-06. **Finding: every DMG failure SameBoy has on the gbdev shootout is one family — mid-mode-3 writes to LCDC and SCY, judged against Mealybug's "DMG-blob" screenshots, which come from another emulator.** Its other two failures are SGB, which gbcli does not target. docboy fails three tests SameBoy passes (cgb-acid-hell, one SameSuite APU test, one MBC3 RTC test). For the bespoke-core question this narrows "SameBoy's ~11 failures" (backend evaluation) to 9 DMG PPU tests whose reference is not hardware truth. Leaf `ZGHaOP`.

## Source

- Page: <https://gbdev.io/GBEmulatorShootout/>, fetched 2026-11-06 with `curl -sSL -o shootout.html https://gbdev.io/GBEmulatorShootout/`: 5,876,782 bytes (inline PNG per cell).
- Parsed with `python3 -I scripts/corpus/shootout_failures.py shootout.html` (stdlib `html.parser`; `--emulator NAME` repeatable, `--json` for machine output). It fails loudly if a row's cell count stops matching the header.
- **The page's stamp: "Updated On Thu, 04 Jun 2026 16:57:20 +0000"**; 264 test rows, 18 emulators.
- **The page states no emulator versions.** Its runner downloads the latest GitHub release at run time (`downloadGithubRelease("LIJI32/SameBoy", …)` in `emulators/sameboy.py`; the same for docboy in `emulators/docboy.py`, gbdev/GBEmulatorShootout at `38b926b`). Matching release dates to the stamp (GitHub releases API, 2026-11-06) **infers SameBoy v1.0.3** (2026-03-04; v1.0.4 followed 2026-10-09) **and docboy 1.2** (2026-03-22; 1.3 followed 2026-06-20). An inference, not a statement on the page.
- The shootout runs the Windows builds, and for SameBoy replaces its open boot ROMs with Nintendo's (`download("https://gbdev.gg8.se/files/roms/bootroms/dmg_boot.bin", …)`). gbcli will run SameBoy's open boot ROMs.

**Calibration:** the header scores are SameBoy 253/264 and docboy 258/261. The parser's tallies reproduce both: SameBoy 250 PASS + 3 INFO = 253 of 264 rows, 11 FAIL; docboy 255 PASS + 3 INFO = 258 of 261 run, 3 FAIL, 3 not run. The header counts INFO rows (`acid/which.gb` DMG and GBC, `daid/rom_and_ram.gb`) as passes.

## SameBoy: 11 failures

| # | Test | Model | docboy |
|---|---|---|---|
| 1 | `samesuite/sgb/command_mlt_req.gb` | SGB | not run |
| 2 | `cpp/sgb-ext-test.gb` | SGB | not run |
| 3 | `mealybug-tearoom-tests/ppu/m3_lcdc_bg_en_change.gb` | DMG | PASS |
| 4 | `mealybug-tearoom-tests/ppu/m3_lcdc_bg_map_change.gb` | DMG | PASS |
| 5 | `mealybug-tearoom-tests/ppu/m3_lcdc_obj_size_change.gb` | DMG | PASS |
| 6 | `mealybug-tearoom-tests/ppu/m3_lcdc_obj_size_change_scx.gb` | DMG | PASS |
| 7 | `mealybug-tearoom-tests/ppu/m3_lcdc_tile_sel_change.gb` | DMG | PASS |
| 8 | `mealybug-tearoom-tests/ppu/m3_lcdc_tile_sel_win_change.gb` | DMG | PASS |
| 9 | `mealybug-tearoom-tests/ppu/m3_lcdc_win_en_change_multiple_wx.gb` | DMG | PASS |
| 10 | `mealybug-tearoom-tests/ppu/m3_lcdc_win_map_change.gb` | DMG | PASS |
| 11 | `mealybug-tearoom-tests/ppu/m3_scy_change.gb` | DMG | PASS |

Not run: none.

## docboy: 3 failures, 3 not run

| Test | SameBoy |
|---|---|
| FAIL `acid/cgb-acid-hell.gbc` | PASS |
| FAIL `samesuite/apu/channel_1/channel_1_sweep_restart_2.gb` | PASS |
| FAIL `cpp/latch-rtc-test.gb` | PASS |
| not run `samesuite/sgb/command_mlt_req.gb` | FAIL |
| not run `samesuite/sgb/command_mlt_req_1_incrementing.gb` | PASS |
| not run `cpp/sgb-ext-test.gb` | FAIL |

docboy has no SGB mode, so the shootout skips its SGB rows (the runner returns no process for that model).

## Reading: which hardware edges SameBoy misses

1. **DMG mid-mode-3 register writes — the only DMG gap.** All nine are Mealybug PPU tests that change LCDC bits (BG enable, BG map, OBJ size, tile data select, window map, and window enable with WX changes) or SCY while a scanline is being drawn, so the fetcher sees the old value for some pixels and the new one for others. SameBoy passes the other 15 of the 24 DMG Mealybug rows (BGP, OBP0, SCX, WX, window timing, and the LCDC OBJ-enable and window-enable-multiple tests), so its fetcher timing is right in general; what it misses is when a write to LCDC's BG/window map, tile-data, BG-enable or OBJ-size bits, or to SCY, lands relative to the tile fetch on DMG. docboy passes all 24.
2. **But the reference is not hardware.** Mealybug's README: the expected images are "screenshots from my Game Boy emulator (which I believe to be correct)", with "blurry photos" of real devices alongside; the DMG set is labeled "DMG-blob" with no CPU revision. SameBoy emulates DMG-B only. Two of the nine tests also ship a DMG-B reference (`m3_lcdc_bg_en_change_dmg_b.png`, `m3_lcdc_win_en_change_multiple_wx_dmg_b.png`), and both differ from the blob reference (SHA-256 `c53832ef…` vs `576d75d0…` for the first). So at least some of this behavior varies by DMG revision, and SameBoy failing a blob reference is not proof it is wrong on a DMG-B. Whether SameBoy matches the DMG-B references is unmeasured; the stage-1 runner will answer it (the corpus has `@dmg_b` rows for both).
3. **SGB packet handling** (2 failures) is out of scope for gbcli. SameBoy v1.0.4 lists "More accurate emulation of SGB command processing" and v1.0.5 "More accurate emulation of invalid MLT_REQ SGB commands", so both may already pass at the version gbcli vendors (v1.0.5, `ea667ad`); unverified.
4. **No CPU, timer, interrupt, DMA, OAM-bug, APU or mapper failures** on this page. That is the page's coverage (blargg, mooneye, SameSuite, Mealybug, acid, a few RTC/SGB/daid tests), not proof of absence: docboy's own 2,355-case suite and GBMicrotest's 513 ROMs are not on the shootout, and the stage-1 baseline runs both.
5. **Versions.** The page reflects SameBoy v1.0.3 (inferred above). The v1.0.4 and v1.0.5 release notes list APU, SGB, printer and RTC fixes and no PPU change, so the nine DMG PPU failures most likely persist in v1.0.5 — an inference from release notes, to be confirmed by the runner.

**For the bespoke-core decision:** on the shootout, docboy's lead over SameBoy on DMG is exactly these nine mid-mode-3 tests. A bespoke core aiming to beat SameBoy on DMG would have to get them right, and would first need a hardware-grounded answer for which DMG revision's behavior to model. Lockstep against SameBoy would treat its behavior here as truth; docboy (or the DMG-B references) breaks the tie.
