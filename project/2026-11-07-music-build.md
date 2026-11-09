# Music in the build: the emitter, hUGEDriver in `new`, and the reference export

2026-11-07, leaf pukTN6. A song an agent writes now goes into a ROM in one loop: you edit `assets/music/theme.song`, run `gbcli build`, then `gbcli run --frames 600 --audio-log --piano roll.png`. This note records what was built, the driver pin (the brief's pin was wrong), how the emitter is checked against hUGETracker's own export (every sample gives identical sound-register writes), and where the emitter departs from that export. Updated the same day by leaf pruzN6 (the review's fixes) and pukTN6's second part (the reference comparison).

## What was built

- **The emitter**, `gbcli_music::rgbds_asm(song, descriptor)` (`crates/music/src/asm.rs`). It is a port of hUGETracker's `RenderSongToRGBDSAsm` (codegen.pas at 3fedced3, lines 371–639) and the code it calls: `OptimizeSong`, `FindUsedStuff`, `InstrumentToBytes` and the cell renderers. It needs no Free Pascal. It keeps hUGETracker's layout and quirks, because a driver that reads past a table reads whatever comes next in the layout:
  - each order names the first stored pattern whose cells are identical;
  - instruments are written up to the highest one each bank plays, and a pattern counts only for the first channel that plays it;
  - waves are written up to the highest one anything names;
  - a subpattern is its first 32 rows, and the last row jumps to row 1.
  
  There are three departures (see "Departures" below for the full list and why). First, every data label is scoped to the descriptor (`song_theme.P0`). hUGETracker's `order1`, `waves` and the like are also hUGEDriver's own WRAM labels, so the `.sym` held each of those names twice. The rebuilt-ROM check (`build --restore`) treated an ambiguous name as gone, and refused every restore. Second, the emitter exports `<descriptor>_TIMER` and `<descriptor>_TMA` as constants (no ROM space), so a player can set up the timer from the song it links. Third (added by leaf pruzN6), `routines:` is a table of routine addresses, so 6xx works. Tests: `crates/music/tests/asm.rs` (one per rule, plus a golden of the help example, `tests/golden/example.asm`).

  > **Correction, 2026-11-07 (leaf pruzN6): this note first said the scoping "doesn't change a byte the driver reads". That was wrong in effect.** Scoping the routine labels under the descriptor (`song_theme.__hUGE_Routine_0`) opened no new scope, so every routine's local labels landed in one scope, and two routines that both used `.loop` didn't assemble. Each routine now opens a global label of its own, `song_theme__hUGE_Routine_N`, as hUGETracker's `__hUGE_Routine_N` does. The bytes of a song that assembled before are unchanged by that fix; which songs assemble is not.
- **`build` by convention** (`crates/build/src/music.rs`). Every `.song` or `.uge` directly under `assets/music` becomes `build/music/<stem>.asm` (a dotted stem keeps its dots: `theme.v2.song` → `theme.v2.asm`) with the label `song_<name>`. That file is assembled with the sources, and the result lists it (`BuildMade.songs`: path, asm, descriptor, and the TMA when the timer drives the song; rows `song path= asm= descriptor= [tma=]`). The build refuses three cases before any tool runs, each with a remedy:
  - a song that doesn't parse: `song_unreadable`, with the line and column (or the byte of a `.uge`);
  - two files that would define one global symbol: the descriptor, `_TIMER`, `_TMA` or a routine label (`gbcli_music::global_symbols`), so `theme_TIMER.song` beside `theme.song` is refused by name;
  - songs with no `hUGE.inc` on the include path.
  
  Tests: `crates/build/tests/music.rs`, and the review's regressions in `crates/build/tests/music_regressions.rs`.
- **A ROM-only link past bank 1 fails the build** (leaf pruzN6; `crates/build/src/project/banks.rs`). Each song is its own `ROMX` section and the starter is ROM-only, so songs past 16 KB put one in bank 2, which a cartridge with no MBC can't map: the driver read bank 1's bytes as the song, silently. The check is not about songs: after rgbfix, when the header's cartridge type is $00, $08 or $09 and the `.map` places any section in a ROMX bank past 1, the build fails with `build_failed`, the sections named (bank, start, size), and the remedy (shrink to 32 KB, or name an MBC at $0147 and switch banks around the code that reads them). The result carries them as `BuildMade.overflow` (rows `past_bank_1 section= bank= start= bytes=`). The starter stays ROM-only. Tests: `crates/build/tests/rom_banks.rs`.

  > **Update, 2026-11-09 (leaf Rf5UAe): a ROM-only build that spills past bank 1 is linked again as one 32 KB bank, and fails only past 32 KB.** Ben's ruling. A cartridge with no MBC maps $0000-$7FFF flat, which is how `rgblink --tiny` links; rgblink's default fills 16 KB banks, so a long song plus the starter's font (Chavez - Monkeys on Mars) put `font Main` in bank 2 while bank 0 had 13.5 KB free. Now, when the header names no MBC and the `.map` places a section past bank 1, or rgblink can't fit bank 0 (`Unable to place "NAME" (ROM0 section) anywhere`) and the one-bank ROM's header names no MBC, the build links again with `--tiny` and adds one warning row (`linked as one 32 KB bank (rgblink -t), since ...`). A build that fits links once, as before. Past 32 KB the build fails as above, `overflow` naming the sections the first link put past bank 1, the message saying they don't fit even as one bank; a full bank 0 that doesn't fit either is rgblink's own error, with a hint naming ROMX and an MBC (`diagnostics/hints.rs`). Tests: `crates/build/tests/it/rom_banks.rs`.
- **`new`** adds three things to every starter:
  - hUGEDriver (`src/hUGEDriver.asm`, `include/hUGE.inc`, `include/hUGE_note_table.inc`, pinned in `crates/build/data/hUGEDriver/SOURCE`);
  - a player, `src/music.asm`. `MusicInit` turns the APU on and calls `hUGE_init`. When the song has the timer on, it also sets TMA, starts TAC at 4096 Hz and adds the timer interrupt. `MusicVBlank`, called from the VBlank handler, ticks the driver unless the timer drives it. The timer vector calls `hUGE_dosound` itself;
  - `assets/music/theme.song`, which is the help topic's example.
  
  `main.asm` gained three edits. It calls `MusicInit`, and its VBlank handler calls `MusicVBlank`. Its main loop now waits for the frame counter to change, not for any interrupt, because a timer song's interrupts also wake the `halt`, and without that check the game logic would run several times a frame. The player also defines `_AUD3WAVERAM`, the hardware.inc 4 name the driver uses, which hardware.inc 6 calls `AUD3WAVERAM`. The driver file is committed unmodified.
- **Cost** (from the asset-pipeline research, §3, measured on the same driver code): about 1.9 KB of ROM0, 100 B of WRAM, and 2,552–5,852 T per tick (mean 3,260 T, 4.6% of a frame) on hUGEDriver's sample song. The cost depends on the song. These figures are in the starter's README and in the player's header comment.
- **The recipe** "Put a song in the game and hear it" (`gbcli help recipes`). The `music` topic also gained a paragraph on the build convention.

## The driver pin: a3cbd0ce, not 727ab8eb

The brief said to vendor hUGEDriver at 727ab8eb, the commit hUGETracker's default branch (3fedced3, "pair to correct version of hugedriver", 2026-08-15) points its submodule at. **That pair doesn't work together.** Between a3cbd0ce and 727ab8eb, the driver gained grooves ("Groove working", 3ca99c70), and with them a new song descriptor. `hUGE_init` now reads four tick bytes, then the order count as a byte. hUGETracker's RGBDS export at 3fedced3 still writes the old descriptor: one tick byte, then a *pointer* to the order count (`db ticks` / `dw order_cnt`). So 3fedced3's own export plays wrong on 727ab8eb. Even the driver repository's own `rgbds_example/sample_song.asm` at 727ab8eb is in the old form. The codegen that matches 727ab8eb exists only on hUGETracker's unreleased `grooves-2025` branch, with `.uge` files that hold four tick values.

hUGETracker 1.0.11 (0623f3f7, the latest release, whose codegen.pas is byte-identical to 3fedced3's) ships **a3cbd0ce**: the release zip's `hUGEDriver.asm` has the same sha256 as a3cbd0ce's. That is the newest driver the export we port is written for, so a3cbd0ce is what `new` vendors. For a `.uge` v6 song the two drivers write the same registers, because 727ab8eb only adds the groove lookup. Moving to grooves later means vendoring a newer driver, emitting the four-tick header, and reading `.uge` v7. That is one decision, to take once hUGETracker releases it.

## Checking the emitter

The criteria call for a register-write comparison against hUGETracker's own export. The harness (`crates/build/tests/music_equiv.rs`) builds two starter ROMs, each with one song's data in `src/song.asm`. It runs them side by side with `equiv::compare`, comparing each frame's sound-register writes (`AudioCompare::Writes`). Both ROMs share one driver (a3cbd0ce) and one player, so any difference comes from the data. The harness is checked against itself on the committed test song: the song with its patterns stored in reverse plays identically over its whole length, and one changed note diverges in the frame it plays, about 6.0 s in. The test song's piano roll and spectrogram from a real run are committed as goldens for a person to look at (`crates/gbcli/tests/cli/golden/every_effect_but_6xx_{piano,spectrogram}.png`).

> **Update, 2026-11-07 (leaf pukTN6, second part): the test song was renamed and extended.** It was `every_effect.song` but used only effects 0–4, A, B and E. It is now `crates/build/tests/data/every_effect_but_6xx.song` (CC0): four channels, a wave instrument with a wave change (9xx on the wave channel, two waves), noise drums, a square instrument with a subpattern, a timer tempo (TMA 192, 64 Hz), three orders, and every effect group the driver has but 6: 0, 1, 2, 3, 4, 5, 7, 8, 9 (duty and wave), A, B, C, D, E and F. A test checks that coverage (`the_test_song_uses_every_effect_but_6xx`). 6xx is left out because hUGETracker's RGBDS export can't play it (see "Departures"); it has its own test against our emitter (`effect_6xx_runs_its_routine`).

**Evidence against a real hUGETracker export (not committed).** hUGEDriver's `rgbds_example/sample_song.asm` is a hUGETracker export of the fetched sample "Coffee Bat - Wyrmhole". It was made by an older hUGETracker that wrote all 15 instruments of each bank and all 16 waves. Its orders, patterns, and every instrument, subpattern and wave our emitter writes are byte-identical to ours, apart from label names. Linked with the same driver and player, the two ROMs gave **identical sound-register writes over the whole song: 4,712 frames, `Verdict::Same`, about 8.7 writes per frame**. That check ran once, from a throwaway test. It was superseded by the comparison below.

## hUGETracker's own export, for every sample (done)

> This section replaces "Open: hUGETracker's own export", which listed three ways to get the reference. Ben ruled that installing hUGETracker was fine; the main thread then built `uge2source` from hUGETracker 1.0.11's source natively (option 2, with Free Pascal 3.2.2 and Lazarus 3.0's LazUtils, no Rosetta): `scripts/music/export-reference.sh` (commit 21780eb).

**How.** `scripts/music/export-reference.sh` builds `uge2source` from hUGETracker 1.0.11 (0623f3f7, whose codegen is the one we port) and exports each fetched sample, with descriptor `song_theme`, into `tests/corpus/fetched/hugetracker-exports/` (gitignored: the songs' license is unstated). `crates/build/tests/music_equiv.rs` links each export and our emitter's output for the same `.uge` into the starter, with the same driver and player, and compares sound-register writes frame by frame. The harness adds one thing to the export: the `song_theme_TIMER` and `song_theme_TMA` constants the player reads, appended from the song's timer setting, since hUGETracker leaves the timer to the player. The export's unscoped labels (`order1`, `waves`) are hUGEDriver's WRAM names too, but neither object exports them, so the link keeps them apart and no rename is needed. When the exports are missing the tests fail and name the two commands (the corpus fetch, then the script).

The test song's export is committed (`crates/build/tests/data/every_effect_but_6xx.asm`, with a provenance header: the script, hUGETracker 0623f3f7, 2026-11-07), so r4P runs without Free Pascal. It was made from the `.uge` our writer produced (`gbcli song every_effect_but_6xx.song --uge`). Remake it the same way after changing the song.

**Results.** Every comparison gave `Verdict::Same`. "Whole" is every order's 64 rows once at the song's starting speed, plus 300 frames of boot; a pattern break or a faster Fxx makes a song shorter, so the run goes into its repeat (`cargo test -p gbcli-build --test music_equiv`, and `-- --ignored` for the whole songs; 2026-11-07).

| Song | Normal suite | Pre-release suite (whole) |
|---|---|---|
| every_effect_but_6xx (committed export) | whole, 1,017 frames: Same | — |
| Arachno - A Sad Touch | 600: Same | 5,420: Same |
| Chavez - Monkeys on Mars | 600: Same | 18,668: Same |
| Coffee Bat - Blue Ocean | 600: Same | 4,524: Same |
| Coffee Bat - Pilgrim's Peril Menu | 600: Same | 4,396: Same |
| Coffee Bat - Wyrmhole | 600: Same | 4,652: Same |
| FADE - Microplastics in the Air | 600: Same | 11,308: Same |
| FADE - Strap in and Suit Up | 600: Same | 10,540: Same |
| Jester - FishNChips | 600: Same | 7,468: Same |
| Maktone - Softworld | 600: Same | 3,180: Same |
| MelonadeM - Boatship for Rent | 600: Same | 3,500: Same |
| raphaelgoulart - The Murderous Funk Machine (intro) | 600: Same | 3,884: Same |
| Reed - Funkabeer's Revenge | 600: Same | 4,524: Same |
| SVL - UwU | 600: Same | 8,364: Same |
| SVL - Yyna | 600: Same | 8,364: Same |
| Tempest - Kekri | 600: Same | 9,900: Same |

> **Correction, 2026-11-09 (leaf Rf5UAe): from daf6d93 to this correction, Chavez - Monkeys on Mars's whole-song row was not Same: its test failed at build.** daf6d93 gave the starter a text renderer and a font, and the font's section (`font Main`, 855 bytes, ROMX) no longer fit in bank 1 beside this song's data (`song_theme Song Data`, 16,222 bytes of bank 1's 16,384). rgblink put it in bank 2, and the build refused the ROM because the starter's header names no MBC (cartridge type $00), which maps banks 0 and 1 only. The ROM held 19.9 KB in all, with 13.5 KB of bank 0 free (rgblink's `.map`, built from the starter's objects with this song as `assets/music/theme.uge`). The harness now takes the starter's text out (`without_text` in `rom_with_asm`), since it writes no sound register, and the row is Same again (`cargo test -p gbcli-build --test it music_equiv::chavez -- --include-ignored`, 2026-11-09). A user's starter with a song this long still fails the same way, naming an MBC as the remedy; what the starter should do about it is open (leaf Rf5UAe's report).
>
> **Update, 2026-11-09 (leaf Rf5UAe, later): the starter with this song now builds, as one 32 KB bank (see the update under "A ROM-only link past bank 1 fails the build"), so the harness has the starter's text back (`rom_with_asm` no longer calls `without_text`) and the row is Same with the starter's real contents (`cargo test -p gbcli-build --test it chavez -- --include-ignored`, 37 s on an Apple M1, 2026-11-09).

The comparison does catch a difference: with one note of the Wyrmhole export changed (the first `dn C_5,1,$2FF` to `D_5`, through `GBCLI_TEST_ROMS` pointed at a copy), its 600-frame test failed. No sample uses 6xx, so none meets the routine departure.

Cost: the 15 sample tests run in parallel, one test each; with the test song's whole-length comparison they took the `music_equiv` binary from about 6 s to about 11 s wall (Apple M1, `cargo test -p gbcli-build --test music_equiv`). The pre-release whole songs take about 91 s.

## Departures from hUGETracker's RGBDS export

1. **Labels.** Data labels are scoped to the descriptor (`song_theme.P0`); each routine opens its own global label, `song_theme__hUGE_Routine_N`. Names never reach the bytes.

   > **Update, 2026-11-07 (leaf FPQsnp, from the second review):** one consequence of the renaming is visible to a song's author. Routine code that names hUGETracker's own labels, `__hUGE_Routine_N` or `__end_hUGE_Routine_N` (to jump between routines, or to measure one), assembled in hUGETracker's export but fails at link here, because those labels are now `song_theme__hUGE_Routine_N` and `song_theme__end_hUGE_Routine_N`. The failure is loud (rgblink names the missing symbol), never wrong data. `gbcli help music` says so and names the labels to use instead.
2. **`routines:` is a table of addresses** (leaf pruzN6). hUGEDriver's `fx_call_routine` (hUGEDriver.asm:719–743) reads a pointer from `routines + 2 × n` and jumps to it. hUGETracker's RGBDS export (codegen.pas:618–627) writes the routines' code at `routines:`, so 6xx in an exported song jumps to an address made of the first routine's code bytes, and the game freezes. hUGETracker's own preview and ROM export don't use that path: they assemble hUGEDriver's `song.asm` template (codegen.pas:758 writes `routineN.htt`), which writes the 16 routines through a `loadRoutine` macro, then `routines:` as `dw __hUGE_Routine_0, …` (eight to a line), then the waves. The emitter follows the template. The table moves the waves 32 bytes later than the export puts them, which only a 6xx, or a read running past the routines' code, would see. Songs that use 6xx therefore can't be compared with the export; the committed test song leaves 6xx out, and `crates/build/tests/music_regressions.rs` (`effect_6xx_runs_its_routine`) checks that a routine runs.
3. **Timer constants.** `<descriptor>_TIMER` and `<descriptor>_TMA` are exported constants with no ROM bytes.
