//! `gbcli recipes`: goal → command.

pub const RECIPES: &str = "\
Recipes — goal → command. Add --session NAME to each, or set GBCLI_SESSION.

Start a session on a ROM
  gbcli run game.gb --session dx --frames 120
Run until something is true
  gbcli run --until 'u16[wFrame] >= 60'
Run to a routine
  gbcli run --to Main.loop
See the screen
  gbcli shot now.png (the head), or gbcli run --frames 60 --shot after.png
Why is the screen blank?
  gbcli shot blank.png --any-frame, then gbcli read '[rLCDC]' (bit 7: LCD on)
Press buttons during a run
  gbcli run --frames 30 --keys 'START@5 -START@7' (or --keys-file keys.txt)
Look at tiles, the background map or sprites
  gbcli view tiles, gbcli view map --map window, gbcli view oam
Find a symbol's address
  gbcli sym 'Player*'
Read variables, registers or IO
  gbcli read '[wSpeed]' 'u16[wFrame]' pc '[rLCDC]'
Where is the machine, and why did it stop?
  gbcli show
Find who writes a variable
  gbcli watch write wSpeed, then gbcli run
Stop at a routine only in some cases
  gbcli break Main.loop --if 'a == 3', then gbcli run
Try a value without rebuilding (what if wSpeed were 2?)
  gbcli run --at before --poke wSpeed=2 --until 'u16[wFrame] >= 60' --read wSpriteX
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
How many cycles does code take?
  gbcli help optimize
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
