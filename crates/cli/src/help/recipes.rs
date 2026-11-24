//! `gbcli recipes`: goal → command.

/// What `gbcli recipes` prints: goal → command.
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
Start over on the rebuilt ROM (rom_changed: the head or checkpoint can't load)
  gbcli run --session dx --from boot --frames 60 (keeps breakpoints and watches,
  moved with their labels; the old head stays in history, checkpoints stay)
Does the save survive turning the Game Boy off? (or a rebuild?)
  gbcli power-cycle (keeps battery RAM and clock; --rtc-advance 8h for time
  off), then gbcli run --frames 300 --shot loaded.png
Hand the save to an emulator or flash cart, or load one
  gbcli sav game.sav; gbcli power-cycle --sav game.sav
Does my art fit the hardware, and how will it look?
  put the PNG in assets/bg/ or assets/sprites/, then gbcli build: a problem row
  per limit broken, with a remedy, else a .preview.png (gbcli help images)
Read an image as text, or draw one as text
  gbcli pixels assets/sprites/hero.png --text hero.pixels (--tiles for a big
  background: tiles and a map; gbcli help pixels), then edit it and gbcli pixels
  hero.pixels --png hero.png, or --preview big.png to look at it 4x with a grid
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
  gbcli run --frames 30 --keys 'START@5 -START@7' (or --keys-file keys.txt);
  a key still held when a run ends stays held into the next run
Play the game from a script (a closed-loop bot), then replay what it did
  runs are deterministic and held keys carry over, so a script can drive a
  session a few frames at a time, reading the state and choosing keys:
  gbcli run --frames 4 --keys 'RIGHT@0' --read '[wPlayerX]' --json, in a loop;
  your script (not gbcli) logs each key change at its frame counted from the
  first run (RIGHT@8 -RIGHT@12), then gbcli run --at start --frames 3000
  --keys-file bot.keys replays the whole game in one run, frame for frame
  (K@N is seen by every joypad read after frame N's VBlank, and a stop shows
  the frame just drawn: gbcli help time)
What does my music do?
  gbcli run --frames 120 --audio-log (the sound writes decoded into notes, at most
  20 rows; --audio-summary counts them all, --json lists them all), then
  --wav music.wav to hear it, --channels each for one file per channel, --piano
  roll.png or --spectrogram spec.png to see it (--scale 4 enlarges a short clip)
Look at tiles, the background map, sprites or the palettes
  gbcli view tiles, gbcli view map --map window, gbcli view oam, gbcli view palettes
Is every sprite drawn in some frame? (a multiplexer, the 10-per-line limit)
  gbcli run --frames 120 --oam-log (the first row's never_drawn= names any object
  dropped in every frame it was on screen; then a row per frame, at most 20)
Which code writes one sound channel?
  gbcli run --frames 600 --audio-summary --audio-channel 4 (writes per register
  and per writer PC; --audio-log --json for every write)
Find a symbol's address
  gbcli sym 'Player*'
Read variables, registers or IO
  gbcli read '[wSpeed]' 'u16[wFrame]' pc '[rLCDC]'
Dump a block of memory (a map, a buffer)
  gbcli view memory wMap 1024 (16 bytes a row; --out map.bin saves the bytes)
Where is the machine, and why did it stop? Which model, which build?
  gbcli show (machine model= cgb=: the session's model, the ROM's header)
Find who writes a variable
  gbcli watch write wSpeed, then gbcli run (stops at the first write, or fails
  with budget_exhausted if none comes in 600 frames)
Stop at a routine only in some cases
  gbcli break Main.loop --if 'a == 3', then gbcli run
What scroll and LCDC was each line drawn with?
  gbcli run --frames 2 --raster --json: result.raster.drawn has one entry a
  frame, {frame, first_line, scx: [...], scy: [...], lcdc: [...]}, entry i for line
  first_line + i, each the register as that line's first tile fetch began, a
  few dots after STAT reads 3. A later write shows on the rest of the line,
  and the next line has it.
Does every scroll or palette write land in HBlank?
  gbcli run --frames 2 --raster (mode= on each IO write is the mode it landed
  in: hblank, oam_scan, drawing or vblank; none where the emulator can't
  tell). Over a whole run from boot, mode= is the proof:
  gbcli run game.gb --session hb --frames 3000 --keys-file play.txt --raster
  (each raster_writer row counts one instruction's writes to one register by
  mode: hblank=, vblank= or lcd_off= alone passes; any other mode names the
  instruction with pc=, and unknown= proves nothing).
  To stop at a write that starts outside HBlank and look around, break on it,
  then rerun the same input from boot (gbcli run --session hb --from boot
  --frames 3000 --keys-file play.txt):
  gbcli break YourWrite --if '([rSTAT] & 3) != 0' --session hb
  (STAT is read as the instruction starts, before it writes, so no stop
  proves nothing: a write that starts late in HBlank can land in the next
  line's OAM scan)
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
  --frame-budget (each frame's busy and HALT time; lag frames never halted,
  LCD-off frames are counted apart: gbcli help budgets)
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
