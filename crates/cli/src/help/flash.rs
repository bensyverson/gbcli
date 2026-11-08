//! `gbcli help flash`: flash-save carts, the boards gbcli models, and what
//! a flash session does with its save.

pub const FLASH: &str = "\
Flash saves — a cart whose ROM is a flash chip the game writes its save to
(GB Studio's \"Save game data to ROM\", and other batteryless carts).

  gbcli run game.gb --session s --flash generic-wr-aaa-a9 --frames 600

--flash BOARD fixes the cart's board when the session is created; checkpoints
record it, a restore refuses a state from another board (or mask ROM), and
power-cycle keeps the chip's contents, across a rebuild too: only the sectors
the program changed come along, and none the new image reaches (a warning names
those: the save no longer fits beside the code). The chip is the board's size,
the image at its start and erased past it; the mapper is the header's and
reaches all of it.

Boards (FlashGBX's cart database; AAA/555: the unlock addresses, A9/AA: the
first unlock byte the CPU writes, A9 when D0 and D1 are swapped):
  generic-wr-aaa-a9      /WE on WR, $0AAA/$0555, D0/D1 swapped: GB Studio's
  generic-wr-aaa-aa      /WE on WR, $0AAA/$0555, straight
  generic-wr-555-a9      /WE on WR, $0555/$02AA, swapped
  generic-wr-555-aa      /WE on WR, $0555/$02AA, straight
  generic-audio-aaa-a9   /WE on AUDIO: no game can write it (flashers only)
  generic-audio-aaa-aa   /WE on AUDIO, straight
  29dl163bd-90           2 MiB, eight 8 KiB boot sectors first
  am29lv160db            2 MiB, commands at $4AAA/$4555, swapped; the chip
                         decodes 12 address bits, so they are $0AAA/$0555
                         and generic-wr-aaa-a9 takes the same commands
  mx29lv320et            4 MiB, eight 8 KiB boot sectors last
  sst39sf040-audio       512 KiB, $5555/$2AAA, /WE on AUDIO

A command the board doesn't take is a warning on the run or step that sent it
(a run that fails too), and a flash_notes entry in --json (ticks addr value
kind text):
  warning: $0AAA<-$A9 is not an unlock cycle on this board: ... D0 and D1 ...
A mask-ROM session whose program sends a board's unlock cycle warns once a
run or step and names the flag: create the session with --flash <that board>.

  gbcli sav game.sav             also writes game.flash.gb: the whole chip,
                                 a ROM any emulator runs with its save in it
  gbcli power-cycle --sav game.sav   imports both (either may be missing; the
                                 head keeps the other); sectors that differ
                                 from the ROM on disk become the chip's
";
