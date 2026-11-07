# Music in the build: the emitter, hUGEDriver in `new`, and the reference export

2026-11-07, leaf pukTN6. A song an agent writes now goes into a ROM in one loop: you edit `assets/music/theme.song`, run `gbcli build`, then `gbcli run --frames 600 --audio-log --piano roll.png`. This note records what was built, the driver pin (the brief's pin was wrong), how the emitter is checked, and the open decision about getting hUGETracker's own export.

## What was built

- **The emitter**, `gbcli_music::rgbds_asm(song, descriptor)` (`crates/music/src/asm.rs`). It is a port of hUGETracker's `RenderSongToRGBDSAsm` (codegen.pas at 3fedced3, lines 371–639) and the code it calls: `OptimizeSong`, `FindUsedStuff`, `InstrumentToBytes` and the cell renderers. It needs no Free Pascal. It keeps hUGETracker's layout and quirks, because a driver that reads past a table reads whatever comes next in the layout:
  - each order names the first stored pattern whose cells are identical;
  - instruments are written up to the highest one each bank plays, and a pattern counts only for the first channel that plays it;
  - waves are written up to the highest one anything names;
  - a subpattern is its first 32 rows, and the last row jumps to row 1.
  
  There are two departures, and neither changes a byte the driver reads. First, every label except the descriptor is scoped to it (`song_theme.P0`). hUGETracker's `order1`, `waves` and the like are also hUGEDriver's own WRAM labels, so the `.sym` held each of those names twice. The rebuilt-ROM check (`build --restore`) treated an ambiguous name as gone, and refused every restore. Second, the emitter exports `<descriptor>_TIMER` and `<descriptor>_TMA` as constants (no ROM space), so a player can set up the timer from the song it links. Tests: `crates/music/tests/asm.rs` (one per rule, plus a golden of the help example, `tests/golden/example.asm`).
- **`build` by convention** (`crates/build/src/music.rs`). Every `.song` or `.uge` directly under `assets/music` becomes `build/music/<name>.asm` with the label `song_<name>`. That file is assembled with the sources, and the result lists it (`BuildMade.songs`: path, asm, descriptor, and the TMA when the timer drives the song; rows `song path= asm= descriptor= [tma=]`). The build refuses three cases before any tool runs, each with a remedy:
  - a song that doesn't parse: `song_unreadable`, with the line and column (or the byte of a `.uge`);
  - two files that would get the same label;
  - songs with no `hUGE.inc` on the include path.
  
  Tests: `crates/build/tests/music.rs`.
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

The criteria call for a register-write comparison against hUGETracker's own export. The harness (`crates/build/tests/music_equiv.rs`) builds two starter ROMs, each with one song's data in `src/song.asm`. It runs them side by side with `equiv::compare`, comparing each frame's sound-register writes (`AudioCompare::Writes`). Both ROMs share one driver and one player, so any difference comes from the data. The harness is checked against itself on the committed test song `crates/build/tests/data/every_effect.song` (CC0, written for this). The song uses four channels, a wave instrument, noise drums, arpeggio, vibrato, portamento, pitch slides, volume slides, a note cut, an order change and a timer tempo (TMA 192, 64 Hz). The song with its patterns stored in reverse plays identically over its whole length. One changed note diverges in the frame it plays, about 6.0 s in. The test song's piano roll and spectrogram from a real run are committed as goldens for a person to look at (`crates/gbcli/tests/cli/golden/every_effect_{piano,spectrogram}.png`).

**Evidence against a real hUGETracker export (not committed).** hUGEDriver's `rgbds_example/sample_song.asm` is a hUGETracker export of the fetched sample "Coffee Bat - Wyrmhole". It was made by an older hUGETracker that wrote all 15 instruments of each bank and all 16 waves. Its orders, patterns, and every instrument, subpattern and wave our emitter writes are byte-identical to ours, apart from label names. Linked with the same driver and player, the two ROMs gave **identical sound-register writes over the whole song: 4,712 frames, `Verdict::Same`, about 8.7 writes per frame**. That check ran once, from a throwaway test. It isn't in the suite, because the file comes from an unlicensed song (see below).

## Open: hUGETracker's own export, for every sample (the user's call)

Criteria Nt1 (every sample over 600 frames) and r4P (the test song over its full length) need the `.asm` hUGETracker itself writes. There are three ways to get it, and each needs a decision:

1. **The release's `uge2source`, fetched like the corpus.** The hUGETracker 1.0.11 macOS zip holds `uge2source`, a headless converter (Mach-O x86_64, so it runs under Rosetta on Apple silicon). Its codegen is the code we port. A corpus script would fetch the zip (pinned by sha256), run `uge2source` on each sample into `tests/corpus/fetched/hugetracker-exports/` (gitignored, since the outputs derive from unlicensed songs), and a test would compare each one to ours. For the CC0 test song, its export can be committed, so r4P then runs without the tool. Costs: it adds a fetched artifact, it runs a downloaded binary on this machine, and it needs Rosetta. Not done here, and the binary was not run.
2. **Build `uge2source` from source.** This needs Free Pascal and Lazarus (`lazbuild`), a toolchain the machine doesn't have.
3. **Commit hUGEDriver's `sample_song.asm`** (or fetch it as a corpus file) as the one reference. It comes from the public-domain driver repository, but it is derived from Coffee Bat's song, whose license is unstated. The user's ruling keeps those songs out of git.

Recommendation: option 1. It covers every sample and the test song with the exact code we port, it keeps unlicensed material out of git, and a corpus fetch is a pattern the project already has. Option 3 is a cheap stopgap for one song if the user prefers.
