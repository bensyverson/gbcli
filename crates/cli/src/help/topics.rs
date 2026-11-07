//! `gbcli help TOPIC`. Short on purpose: each says what an agent needs to
//! type, and links out for the rest.

/// The help topics (design §12).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Topic {
    Addresses,
    Expressions,
    Time,
    Checkpointing,
    Writes,
    Budgets,
    ExitCodes,
    Hardware,
    Traps,
    Optimize,
    Rgbds,
    Music,
    Images,
}

impl Topic {
    pub const ALL: [Topic; 13] = [
        Topic::Addresses,
        Topic::Expressions,
        Topic::Time,
        Topic::Checkpointing,
        Topic::Writes,
        Topic::Budgets,
        Topic::ExitCodes,
        Topic::Hardware,
        Topic::Traps,
        Topic::Optimize,
        Topic::Rgbds,
        Topic::Music,
        Topic::Images,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Topic::Addresses => "addresses",
            Topic::Expressions => "expressions",
            Topic::Time => "time",
            Topic::Checkpointing => "checkpointing",
            Topic::Writes => "writes",
            Topic::Budgets => "budgets",
            Topic::ExitCodes => "exit-codes",
            Topic::Hardware => "hardware",
            Topic::Traps => "traps",
            Topic::Optimize => "optimize",
            Topic::Rgbds => "rgbds",
            Topic::Music => "music",
            Topic::Images => "images",
        }
    }

    pub fn from_name(name: &str) -> Option<Topic> {
        Topic::ALL.into_iter().find(|t| t.name() == name)
    }

    pub fn text(self) -> &'static str {
        match self {
            Topic::Addresses => ADDRESSES,
            Topic::Expressions => EXPRESSIONS,
            Topic::Time => TIME,
            Topic::Checkpointing => CHECKPOINTING,
            Topic::Writes => WRITES,
            Topic::Budgets => BUDGETS,
            Topic::ExitCodes => EXIT_CODES,
            Topic::Hardware => super::hardware::HARDWARE,
            Topic::Traps => super::traps::TRAPS,
            Topic::Optimize => OPTIMIZE,
            Topic::Rgbds => RGBDS,
            Topic::Music => super::music::text(),
            Topic::Images => super::images::IMAGES,
        }
    }
}

const ADDRESSES: &str = "\
Addresses — one grammar for every verb and expression.

  $C0A0, 0xC0A0        as the CPU sees it, in the bank mapped now
  03:4123, $03:$4123   that address in that bank (RGBDS .sym style); use it
                       for breakpoints and patches in a switchable area
  wram:0x1A0           an offset into a whole region, across banks, in hex
                       with or without $ or 0x (sram:10 is offset $10); regions:
                       rom wram sram vram hram oam io ie bootrom bgpal objpal
  wFrame, Main.loop    a symbol from the ROM's .sym (found beside the ROM, or
                       --sym when the session is created)
  Main.loop+$3         a symbol plus or minus an offset: decimal (Main+16), or
                       hex with $ or 0x (Main+$10), as RGBDS reads it
  rLY, rLCDC, rDMA     hardware registers by their hardware.inc names; no .sym
                       needed
  oam[0], oam[0].x     an OAM entry and one of its bytes, in reads and
                       conditions only (gbcli help expressions); poke and
                       watch take the offset: oam:1 is entry 0's x

A bad address says what fits: the region's size, the offsets a symbol allows,
or the nearest symbol names.
";

const EXPRESSIONS: &str = "\
Expressions — for --until, --if, --read and `gbcli read`. They never touch the
bus, so evaluating one has no side effects.

  numbers     $FF, 0xFF, 255, %1010
  registers   a f b c d e h l af bc de hl sp pc; flags zf nf hf cf; ime
  memory      [ADDR] reads a byte, u16[ADDR] a little-endian word; ADDR is any
              address form (gbcli help addresses) or an expression: [hl]
  sprites     oam[N].y, .x, .tile, .attr: a byte of OAM entry N (0-39);
              oam[N] alone, in --read and `gbcli read`, prints the entry as
              fields: y x tile attr priority yflip xflip palette (and
              cgb_bank cgb_palette in CGB mode)
  machine    frame, t (T-cycles since the time base), ly, bank
  watches     old and new: the byte before and after the write
  operators   + - * / % & | ^ ~ << >>  == != < <= > >=  && || !  (C precedence)

A bare RAM symbol is its address: write [wFrame] >= 60, not wFrame >= 60.
Reading a byte the program never wrote is an error (exit 3) in --read and
`gbcli read`; pass --allow-uninit to see its power-on value. In --until and
--if it means \"not yet\": the run keeps going.

  gbcli run --session dx --until 'u16[wFrame] >= 60 && ly == 0'
";

const TIME: &str = "\
Time — every frame and cycle count says what it counts from.

  boot        the default: the boot ROM's handoff to the cartridge (PC=$0100).
              Before it, counts come from power-on and say so.
  power-on    tick 0
  NAME        a named checkpoint

--since BASE changes the base for run and read; stop rows print since=.
The first run of a ROM runs the real boot ROM once and caches the handoff in
.gbcli/cache/; later runs start there. --from power-on starts cold instead.
";

const CHECKPOINTING: &str = "\
Checkpointing — saved machine states, in the session.

  gbcli checkpoint NAME            save the head as NAME
  gbcli run --checkpoint NAME …    save a run's stop as NAME
  gbcli run --at NAME …            run from NAME (the head moves to the stop)
  gbcli restore NAME               make NAME the head
  ~1 … ~8                          the head 1 to 8 changes back (--at, restore)
  gbcli undo                       step the head back one change

A checkpoint records the ROM's hash and patches. Loaded under a rebuilt ROM,
it is checked by symbol: if PC's routine, a RAM variable, a return address or
a breakpoint moved, the load is refused (exit 3) and the message lists what
moved; --allow-rom-change loads anyway. Breakpoints and watches belong to the
session, not the checkpoint, so restoring never changes what you watch.
`sessions prune` never removes a session that holds named checkpoints.
";

const WRITES: &str = "\
Writes — three kinds, each its own verb and its own run flag.

  gbcli write rLCDC=0x91   bus write, as the CPU would: DIV resets, DMA starts,
                           banks switch (run --write)
  gbcli poke wSpeed=2      backing memory, no side effects; ROM is refused
                           (run --poke)
  gbcli patch 01:4123=0,0  ROM bytes, recorded in every later checkpoint; the
                           header ($0100-$014F) is refused (run --patch)

BYTES are comma-separated bytes ($91, 0x91, 145, %10010001); a word goes low
byte first: wX=$34,$12. In run, writes apply in the order given, after the
start state loads and before the machine moves.
";

const BUDGETS: &str = "\
Budgets — every run ends.

  --frames N, --instructions N, --ticks N   whichever ends first stops the run
  run --until / --to                        600 frames unless you give one
  step                                      one unit

A run with a target (--until, --to) that runs out of budget fails with
budget_exhausted (exit 3) and still prints the stop it reached, which becomes
the head; raise the budget or check the predicate. If the CPU locks up first
(illegal_opcode, halt_forever), it fails the same way, as target_not_reached.
If a breakpoint or watch you set stops it first, it succeeds with a warning.
`run --frames N` alone succeeds when the frames are done. A 60 s wall-clock
guard backs every request up; a stop it causes says wall_guard and is not
reproducible.
";

const EXIT_CODES: &str = "\
Exit codes — one table for every command; errors print to stderr as
`gbcli: CODE: message` and `remedy: next command`.

  0  success
  1  the answer is no: predicate_false frame_not_normal (the shot's frame
     wasn't a normal one; --any-frame takes it anyway)
  2  usage: bad_request bad_address bad_expression ambiguous_symbol
     unknown_symbol refused (a flag is needed)
  3  budget or conflict: budget_exhausted rev_mismatch session_busy
     rom_changed uninitialized_read target_not_reached (a run's --to or
     --until ended short: the CPU locked up first; the stop says how)
  4  something didn't load: rom_unreadable checkpoint_unreadable
     checkpoint_mismatch symbols_unreadable session_unreadable
     build_failed (RGBDS rejected the sources: see the diagnostic rows)
     song_unreadable (a song file is missing or malformed: see the problem row)
  5  environment: unsupported no_session boot_rom_missing toolchain_missing
     io_failed internal
";

const OPTIMIZE: &str = "\
Optimize — measure, patch, measure again.

  1. Find where the time goes:
       gbcli run --session dx --frames 60 --profile --top 10 --frame-budget
  2. Measure the routine: cycles from each arrival at one address to the next
     arrival at another (the head doesn't move):
       gbcli measure Work..Work.done --session dx --iterations 60
  3. Try a change without rebuilding, measured on a copy of the stop:
       gbcli run --session dx --patch 00:0165=0 --frames 1 --measure Work..Work.done
  4. Rebuild with the winner and hold the gain with an assertion that exits 1:
       gbcli measure Work..Work.done --session dx --iterations 60 --max 90
  5. Check the rebuild still behaves: gbcli equiv fast.gb --frames 600

A frame is 70,224 T-cycles; `gbcli help budgets` if a run never arrives.
";

const RGBDS: &str = "\
RGBDS — the loop: edit, build, restore, measure, with RGBDS from PATH.

  gbcli doctor                 RGBDS at the pinned version? (exit 5 names the
                               install command)
  gbcli new game               a starter: header, VBlank handler, OAM DMA in
                               HRAM, hardware.inc, a sprite on screen
  gbcli build game             rgbasm every src/**/*.asm (include/ on the
                               path), rgblink to build/game.gb + .sym + .map,
                               rgbfix -v -p 0xFF. No build file: the layout
                               is the build; --src, --include, --out change it;
                               PNGs in assets/ too (gbcli help images)
  The loop, from inside game/:
    gbcli run build/game.gb --session game --frames 60 --checkpoint start
    (edit src/)
    gbcli build --session game --restore start
    gbcli measure VBlank..VBlank.done --session game

Each error is a `diagnostic` row with file= and line=; build_failed exits 4
and leaves the last good ROM in place. --restore loads the checkpoint into the
rebuilt ROM only if nothing it was running moved; rom_changed (exit 3) lists
what did, and --allow-rom-change loads it anyway.
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_topic_round_trips_by_name_and_has_text() {
        for topic in Topic::ALL {
            assert_eq!(Topic::from_name(topic.name()), Some(topic));
            assert!(topic.text().lines().count() >= 5, "{topic:?}");
        }
    }

    #[test]
    fn the_exit_code_topic_lists_every_code_under_its_exit() {
        let mut exit = None;
        let mut seen = 0;
        for line in EXIT_CODES.lines().skip(3) {
            let words: Vec<&str> = line.split_whitespace().collect();
            let names = match words.first().and_then(|w| w.parse::<u8>().ok()) {
                Some(n) => {
                    exit = Some(n);
                    &words[1..]
                }
                None => &words[..],
            };
            for name in names.iter().filter(|w| w.contains('_') || w.len() > 3) {
                let quoted = format!("\"{}\"", name.trim_matches(|c| c == '(' || c == ')'));
                if let Ok(code) = serde_json::from_str::<gbcli_protocol::ErrorCode>(&quoted) {
                    assert_eq!(Some(code.exit_code()), exit, "{name}");
                    seen += 1;
                }
            }
        }
        assert_eq!(seen, 27, "every code in the closed set");
    }

    #[test]
    fn the_images_topic_names_the_convention_every_limit_and_what_the_program_gets() {
        use gbcli_protocol::ImageLimit::*;
        let text = Topic::from_name("images").expect("an images topic").text();
        for limit in [
            BgTilesMap,
            VramTiles,
            ObjTiles,
            DmgPalette,
            SizeMultiple8,
            ObjPair8x16,
            ObjNoTransparency,
            ObjPerLine,
            ObjTotal,
            FrameGrid,
            FrameSpan,
            RomxBank,
            Alpha,
        ] {
            assert!(text.contains(limit.name()), "{} missing", limit.name());
        }
        // The shade rule as shades.rs applies it: nearest lightness, unless
        // two colors would share a shade, and colors one in RGB555 are one.
        for needle in ["nearest its lightness", "spread", "RGB555"] {
            assert!(text.contains(needle), "{needle} missing");
        }
        for needle in [
            "assets/bg/",
            "assets/sprites/",
            "hero.16x16.png",
            "INCLUDE \"images.inc\"",
            "TownTiles",
            "TOWN_WIDTH",
            "HeroFrames",
            ".preview.png",
        ] {
            assert!(text.contains(needle), "{needle} missing");
        }
        assert!(text.lines().all(|l| l.chars().count() <= 80), "{text}");
    }

    #[test]
    fn the_music_topic_lists_every_effect_and_carries_the_worked_example() {
        let text = Topic::from_name("music").expect("a music topic").text();
        for effect in gbcli_music::EFFECTS {
            let line = format!("  {} {}", effect.spelling, effect.name);
            assert!(text.contains(&line), "{line:?} missing");
        }
        assert!(text.contains(gbcli_music::EXAMPLE), "the example, verbatim");
        assert!(text.contains(gbcli_music::FORMAT_LINE));
        assert!(text.lines().all(|l| l.chars().count() <= 80), "{text}");
    }
}
