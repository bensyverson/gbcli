# The flash-chip model for flash-save carts (leaf L00RuF)

2026-11-07. What was built, what the GB Studio validation found, and the one choice left open: how a session names its board.

## What exists

- **`crates/flash` (`gbcli-flash`)**: a backend-free AMD/JEDEC chip (`FlashChip`) driven by a core's memory callbacks, with `FlashProfile` for the board and `FlashNote` for commands that miss it. Design §6, update of this date, lists the behavior. Unit tests: `crates/flash/tests/chip.rs`, the sequences from gbvm's `src/core/gb/flasher_s.s` (chrismaltby/gbvm at `4f01d26`, MIT) and the AMD command set.
- **Named profiles** restate the facts of ten FlashGBX files (lesserkuma/FlashGBX `config/fc_DMG_*.txt` at `5b2398b`; GPL-3.0, so no file is vendored). `scripts/flash/flashgbx-summary.py <FlashGBX>/FlashGBX/config` prints every AMD DMG board's pin, unlock cycles, swap, size and sector map. Across the 93 AMD boards it lists: unlock $0AAA/$0555 on 52 (20 swapped), $0555/$02AA on 20 (7 swapped), $5555/$2AAA on 10, $4AAA/$4555 on 8 (5 swapped), and three rarer variants; /WE on WR 72, AUDIO 21.
- **SameBoy**: `CoreConfig::flash`, the chip on SameBoy's callbacks (`crates/sameboy/src/flash.rs`), the blob's flash section (format 4), `Core::flash_contents` / `load_flash_contents` / `take_flash_notes`. Tests: `crates/sameboy/tests/flash.rs`.
- **Engine**: `Machine` keeps the profile in its setup and `power_cycle` carries the chip's contents.

Assumed, not sourced: the busy times (`BusyTicks::TYPICAL`: 10 µs program, 0.7 s sector erase, 25 s chip erase) and the 2 MiB size of the generic boards, whose FlashGBX files give none.

## The GB Studio validation

Ben exported GB Studio's default project with MBC5 and "Save game data to ROM" (`local/demo/`, gitignored). Header: $1E (MBC5+RUMBLE+RAM+BATTERY), ROM size $02 (128 KiB), RAM $03. `__start_save` is 12 (`demo.map`).

- **The save lands past the image.** gbvm erases and programs banks 12-15, but the file is 8 banks. SameBoy masks the bank by its buffer size, so on the bare image bank 12 is bank 4, which holds code (785 of its 16,384 bytes are $FF): the erase would wipe it. A real cart's chip is larger than the image. So the model pads SameBoy's buffer with $FF to the profile's chip size, and the mapper reaches all of it.
- **How the project saves**: the cave's save point (actor at tile 14,11, `project/scenes/cave/actors/actor_6.gbsres`) offers "Save Game / Cancel" and runs `EVENT_SAVE_DATA`; the title screen's "Continue" runs `EVENT_LOAD_DATA` or says "No Save Data Found...". gbvm restores SRAM from flash at start-up (`load_save.c`), so the cart's battery-backed SRAM, which the power cycle keeps, is overwritten from flash and doesn't mask a failed save.
- **The tests** (`crates/engine/tests/flash_save.rs`) drive that by key input: power-on, START, New Game, walk west around the pond, north up column 2, east along row 9 into the cave, up and right to (12,11), A, "Save Game", then a power cycle and "Continue". The player's tile is read from `_actors+2` (x) and `_actors+4` (y), found by walking the player with `gbcli run --keys ... --read`, not from gbvm's source. On `generic-wr-aaa-a9` (swapped, the sequence gbvm hard-codes) the save is programmed into banks 12-15, no notes are taken, and Continue puts the player back at (12,11). On `generic-wr-aaa-aa` nothing is programmed, the first note reads "$0AAA<-$A9 is not an unlock cycle on this board: the chip reads $A9 and expects $AA. $A9 is what a board with D0 and D1 swapped needs ...", and Continue doesn't reach the cave. Disabling the power cycle's flash carry makes the first test fail (checked by hand).
- **Cost**: each test runs about 3,000 frames with the read callback on every access, about 10-12 s alone (`cargo test -p gbcli-engine --test flash_save`, dev profile, Apple M1 under load average 26-38). That is a large share of the 30 s warm budget; whether they belong in the pre-release suite is open (below).

## Open: how a session names its board

The model is reachable only by building a `CoreConfig` in Rust. Neither the checkpoint header nor the session bundle records a profile, so `Machine::restore` keeps the machine's own and no request can choose one. Options:

1. **A session property, like `--model` and `--seed` (recommended).** `--flash PROFILE` when a session is created; stored in the bundle's meta and in the checkpoint header (a new optional field beside `config`, the profile's name plus any overridden parameters), checked on restore like the model (a conflict is `bad_request`), and kept by `power-cycle`. A board is part of the cart, which a session fixes at creation; this is the existing convention for what the core is built from. Notes surface on the run that took them, as warning rows (`flash_note at= addr= value= text=`) and `--json` fields.
2. **A hint from a mask-ROM session**, in addition to 1: when a run sees a write that is an unlock cycle for some board (the same test `FlashChip` uses), warn "this program sends flash commands; create the session with --flash generic-wr-aaa-a9 to model a flash-save cart". Cheap, and it routes an agent that didn't know to the flag.
3. **Per power cycle** (`power-cycle --flash PROFILE`), to try one save on several boards. Rejected as the primary surface: it changes the cart mid-session, and a checkpoint before it would load into another board. Trying boards is two sessions.
4. **A project file** for the build loop (`gbcli.toml` naming the board). Useful later, as a default for option 1; not a substitute.

Choices inside option 1 that also need a ruling: whether custom parameters are allowed at the CLI (e.g. `--flash generic-wr-aaa-aa,size=4M`) or only named profiles at first; and whether `sav`/`power-cycle --sav` should also export/import the flash save area (a GB Studio batteryless game's save lives there, not in SRAM).

## Other loose ends

- The GB Studio tests need the hand-built ROM; a fresh clone fails them with the build steps, as the corpus tests do. Whether to keep them in the warm suite (10-12 s each) or move them to the pre-release suite is for Ben.
- The server's rebuild path (`power_cycle` across a rebuilt ROM) carries the flash from a fresh core, so nothing; moot until a session can name a profile, then it should carry the head's contents as it carries its battery.
- Not modeled: Intel command sets, CPLD multicarts (out of scope), CFI queries, erase suspend, read-while-write banks, the partial state an operation cut off by the power leaves (counted as done, with a note), and the DQ5 timeout a program that sets bits raises on hardware (here a note, and the AND of the bytes).
