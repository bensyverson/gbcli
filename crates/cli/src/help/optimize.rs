//! `gbcli help optimize`: the measure-change-measure loop against a baseline
//! that stays put. A CLI test runs every command line in it, in order,
//! inside a fresh `gbcli new game` (crates/gbcli/tests/cli/recipes.rs).

pub const OPTIMIZE: &str = "\
Optimize — measure, change, measure again, against a baseline that stays put.

A session runs the file at its ROM path, so building over build/game.gb
changes what its session runs. Keep the baseline's build where it is and
build each try to another file, measured in a session of its own. The lines
run as written inside a `gbcli new game` project. There, the span
VBlank..MainLoop.wait runs from the VBlank handler's entry to the next time
the main loop waits at its HALT: the handler (OAM DMA, the frame count, the
music tick), then the frame's game logic, empty in the starter.

  1. A baseline session and a start state to measure from; its symbols are
     build/game.sym, the ROM's name with .sym (--sym FILE names another):
       gbcli run build/game.gb --session base --frames 60 --checkpoint start
  2. Find where the time goes:
       gbcli run --session base --at start --frames 60 --profile --top 10 --frame-budget
  3. Measure a span: cycles from each arrival at one address to the next
     arrival at another, on a copy (the head doesn't move). --keys holds
     buttons as run --keys does, for the held-input path: --keys 'RIGHT@0'.
       gbcli measure VBlank..MainLoop.wait --session base --at start --iterations 60
  4. Try a change without rebuilding, measured on a copy of the stop; here,
     the frame without the music tick (its call patched to three nops). The
     patch rides on the head; the measures here start --at start, without it:
       gbcli run --session base --at start --patch VBlank.done=0,0,0 --frames 1 --measure VBlank..MainLoop.wait
  5. Edit src/, then build the winner to another file (its .sym,
     build/fast.sym, lands beside it), measure it in its own session, and
     hold the gain with an assertion that exits 1:
       gbcli build --out build/fast.gb
       gbcli run build/fast.gb --session fast --frames 60 --checkpoint start
       gbcli measure VBlank..MainLoop.wait --session fast --at start --iterations 60 --max 6000
  6. Check it behaves as the baseline, frame for frame:
       gbcli equiv build/fast.gb --session base --frames 300 --keys 'RIGHT@0'

A change that alters timing. equiv compares the screen at the end of every
frame, so a correct change that draws sooner, or a start-up that takes
longer, fails it: the screens differ only because they were drawn at
different moments. A frame_offset row says when the divergent frame ended at
a different time on each side. A renderer that reads the keys once a view
also sees them at other frames when it gets faster, so its views differ, not
only arrive earlier. Three checks hold:
  - Lock the output in a test mode: a frame-locked bench mode in the ROM
    (say, SELECT held at power-on draws a view only every 32 frames), so
    both builds show each view in the same frame; equiv the builds in it.
  - Compare timing-free state, with no screen: the positions and state the
    game decides, not when it draws them. Hold no keys while views are
    drawn, or the two builds read them at different frames:
       gbcli equiv build/fast.gb --session base --frames 300 --screen off --ram wShadowOAM,160
  - Check the output against a reference: a renderer on the host that draws
    the same view, compared with `gbcli shot` at a set of poses.

A frame is 70,224 T-cycles; `gbcli help budgets` if a run never arrives.
";
