//! `gbcli recipes`: goal → command.

pub const RECIPES: &str = "\
Recipes — goal → command. Add --session NAME to each, or set GBCLI_SESSION.

The dev loop, from an empty directory (gbcli's tests run these lines in order)
  gbcli doctor
  gbcli new game
  cd game
  gbcli build
  gbcli run build/game.gb --session game --frames 60 --checkpoint start
  gbcli shot first.png --session game
  gbcli read 'u16[wFrame]' '[rLCDC]' --session game
  gbcli view oam --session game
  gbcli run --session game --frames 30 --audio-log
  (edit src/main.asm, then rebuild and carry on from the checkpoint)
  gbcli build --session game --restore start
  gbcli measure VBlank..VBlank.done --session game

Start a session on a ROM
  gbcli run game.gb --session dx --frames 120
Start a new game from scratch
  gbcli doctor (is the pinned RGBDS on PATH?), gbcli new game, gbcli build game
Why won't it build?
  gbcli build: one diagnostic row per message, with file= and line=
Rebuild after an edit and carry on where you were
  gbcli build --session dx --restore start (the checkpoint, in the new ROM;
  refused with what moved if its routine or variables moved; gbcli help rgbds)
Does the save survive turning the Game Boy off? (or a rebuild?)
  gbcli power-cycle (keeps battery RAM and clock; --rtc-advance 8h for time
  off), then gbcli run --frames 300 --shot loaded.png
Hand the save to an emulator or flash cart, or load one
  gbcli sav game.sav; gbcli power-cycle --sav game.sav
Does my art fit the hardware, and how will it look?
  put the PNG in assets/bg/ or assets/sprites/, then gbcli build: a problem row
  per limit broken, with a remedy, else a .preview.png (gbcli help images)
Read or write music (hUGETracker songs)
  gbcli song theme.uge --text theme.song (a grid an agent can edit; gbcli help
  music), then gbcli song theme.song --uge theme.uge --piano theme.png
Put a song in the game and hear it
  edit assets/music/theme.song (gbcli new makes it; every .song or .uge there is
  built), gbcli build, then gbcli run --frames 600 --audio-log --piano roll.png
  (and --wav music.wav --channels each for one file per channel)
Run until something is true
  gbcli run --until 'u16[wFrame] >= 60'
Run to a routine
  gbcli run --to Main.loop
See the screen
  gbcli shot now.png (the head), or gbcli run --frames 60 --shot after.png
Watch the screen change over a run
  gbcli run --frames 60 --filmstrip strip.png --every 4 (16 labeled frames in a row)
Why is the screen blank?
  gbcli shot blank.png --any-frame, then gbcli read '[rLCDC]' (bit 7: LCD on)
Press buttons during a run
  gbcli run --frames 30 --keys 'START@5 -START@7' (or --keys-file keys.txt)
What does my music do?
  gbcli run --frames 120 --audio-log (every sound write, decoded into notes), then
  --wav music.wav to hear it, --channels each for one file per channel, --piano
  roll.png or --spectrogram spec.png to see it (--scale 4 enlarges a short clip)
Look at tiles, the background map or sprites
  gbcli view tiles, gbcli view map --map window, gbcli view oam
Find a symbol's address
  gbcli sym 'Player*'
Read variables, registers or IO
  gbcli read '[wSpeed]' 'u16[wFrame]' pc '[rLCDC]'
Where is the machine, and why did it stop?
  gbcli show
Find who writes a variable
  gbcli watch write wSpeed, then gbcli run (stops at the first write, or fails
  with budget_exhausted if none comes in 600 frames)
Stop at a routine only in some cases
  gbcli break Main.loop --if 'a == 3', then gbcli run
Does every scroll or palette write land in HBlank?
  gbcli run --frames 2 --raster (mode= on each IO write: hblank, oam_scan,
  drawing or vblank; none where the emulator can't tell). To prove it over a
  whole run from boot, break on each write instruction if STAT isn't mode 0:
  gbcli break YourWrite --if '([rSTAT] & 3) != 0' --rom game.gb --session hb
  gbcli run --session hb --frames 3000 --keys-file play.txt (no stop is the
  proof; STAT is read as the instruction starts, just before it writes; a
  condition_error stop means STAT couldn't be read there, so it proves nothing)
Try a value without rebuilding (what if wSpeed were 2?)
  gbcli run --at before --poke wSpeed=2 --until 'u16[wFrame] >= 60' --read '[wSpriteX]'
Turn the LCD on as the program would
  gbcli write rLCDC=0x91
Keep a state to come back to
  gbcli checkpoint before, later gbcli restore before or gbcli run --at before
What can I restore?
  gbcli checkpoints
Undo the last run, step or write
  gbcli undo
Step through code
  gbcli step, gbcli step over, gbcli step out
How many cycles does X take?
  gbcli measure Work..Work.done --iterations 60 (T, M and share of a frame;
  --max T makes it an assertion that exits 1; --keys 'RIGHT@0' with RIGHT
  held); gbcli help optimize for the loop
How many bytes and cycles is an instruction, and which flags does it set?
  gbcli opcode 'ld a, [hl+]', gbcli opcode xor a, gbcli opcode '$CB37'
A write, a HALT or a DMA does nothing, or the code runs wild
  gbcli help traps (VRAM in mode 3, LCD off, the HALT bug, OAM DMA, old RGBDS)
Where does my frame time go?
  gbcli run --frames 60 --profile --top 10 (cycles per routine), and
  --frame-budget (each frame's busy and HALT time; lag frames never halted)
Does my optimized build still behave?
  gbcli build --out build/fast.gb (not over the session's ROM, which stays the
  baseline), then gbcli equiv build/fast.gb --frames 600 --keys-file walk.txt
  --ram wX (exit 1 at the first frame that differs, with both sides saved as
  equiv-a and equiv-b); gbcli help optimize for the whole loop
Did my rebuilt ROM break a saved state?
  gbcli run --at before --frames 1 (refused with what moved, or loaded)
Who changed the session?
  gbcli events --limit 10
What can't the emulator do here?
  gbcli gaps
Drive gbcli from a harness
  gbcli serve --session dx (see gbcli schema), or gbcli batch -F ops.jsonl
Clean up
  gbcli sessions, gbcli sessions rm dx, gbcli sessions prune --unused-for 7d
";
