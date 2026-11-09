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
    Pixels,
    Flash,
}

impl Topic {
    pub const ALL: [Topic; 15] = [
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
        Topic::Pixels,
        Topic::Flash,
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
            Topic::Pixels => "pixels",
            Topic::Flash => "flash",
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
            Topic::Optimize => super::optimize::OPTIMIZE,
            Topic::Rgbds => RGBDS,
            Topic::Music => super::music::text(),
            Topic::Images => super::images::IMAGES,
            Topic::Pixels => super::pixels::text(),
            Topic::Flash => super::flash::FLASH,
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
              address form (gbcli help addresses) or an expression: [hl].
              Widths: u8[ADDR] (the same as [ADDR]) and u16 unsigned, s8 and
              s16 signed (sign-extended: s8[wDX] < 0 when bit 7 is set)
  sprites     oam[N].y, .x, .tile, .attr: a byte of OAM entry N (0-39);
              oam[N] alone, in --read and `gbcli read`, prints the entry as
              fields: y x tile attr priority yflip xflip, then palette, or
              cgb_bank cgb_palette in CGB mode. Both read OAM itself, not
              what the CPU sees: during PPU modes 2 and 3 (and OAM DMA) the
              program reads $FF there, but oam[N] shows the stored byte
  machine    frame, t (T-cycles since the time base), ly, bank
  watches     old and new: the byte before and after the write
  operators   + - * / % & | ^ ~ << >>  == != < <= > >=  && || !  (C precedence)

A bare RAM symbol is its address: write [wFrame] >= 60, not wFrame >= 60.
Reading a byte the program never wrote is an error: `gbcli read` fails with
exit 3, and a run's --read says so in that read's row (error=, message=,
remedy=) while the run, its stop and its other reads stand. Pass
--allow-uninit to see its power-on value. In --until and --if it means
\"not yet\": the run keeps going.

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
Either --from skips the head: --from boot is how a session starts over on a
rebuilt ROM, keeping its breakpoints and watches.

Frames and keys. A frame begins as VBlank does (ly=144). A run --frames N
stops there, at the VBlank interrupt vector ($0040, ly=144), before the
handler runs: the stop shows the frame just drawn, but what the handler does
for it (OAM DMA, a frame counter, a joypad read) happens in the next run, so
run one more frame to see it. A key token K@N changes the buttons at the
start of the run's frame N (offsets count from the run's start), so every
joypad read the program makes after that VBlank sees it, and none before.
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
it is checked by symbol: if PC's routine, a RAM variable or a return address
moved, the load is refused (exit 3) and the message lists what moved;
--allow-rom-change loads anyway. Breakpoints and watches belong to the
session, not the checkpoint, so restoring never changes what you watch; one
set by a symbol follows it into a rebuild, with a warning when it moves.
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

The frame budget (run --frame-budget) is the other kind: how much of each
frame the CPU worked. A lag frame never halted with the LCD on. A frame the
LCD was off for (startup, a screen change) counts as lcd_off, not lag, and
stays out of active_min/max/mean and busiest; --json marks each frame's
lcd_off. A game that busy-waits instead of halting lags every frame: measure
its loop with gbcli measure.
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
     pixels_unreadable (an image is missing or malformed: see the problem row)
  5  environment: unsupported no_session boot_rom_missing toolchain_missing
     io_failed internal
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
  gbcli build game --gbc       the same, for the GBC in dual mode, to
                               build/gbc/game.gbc + .sym + .map (its own
                               folder, so the plain build's stay): rgbfix -c
                               sets $0143 = $80, so the ROM is GBC-enhanced
                               and still runs on the DMG. (Pan Docs calls
                               the GBC the CGB.) rgbfix -C (GBC only, $C0)
                               isn't offered. A session runs it on the DMG
                               unless made with --model gbc (a GBC in CGB
                               mode), so check a dual ROM in two sessions
  gbcli assets game --json     the images alone, converted as build would,
                               with rgbgfx alone and nothing written: tile
                               maps and palettes; --manifest FILE instead
                               of a project (gbcli help images)
  The loop, from inside game/:
    gbcli run build/game.gb --session game --frames 60 --checkpoint start
    (edit src/)
    gbcli build --session game --restore start
    gbcli measure VBlank..VBlank.done --session game

Each error is a `diagnostic` row with file= and line=; the same message on the
same or the next line, back to back, is one row with lines=FIRST-LAST and
count=N, even when that is two typos; errors= counts rows. build_failed
exits 4 and leaves the last good ROM in place. --restore loads the checkpoint
into the rebuilt ROM only if nothing it was running moved; rom_changed (exit
3) lists what did, and --allow-rom-change loads it anyway. To start over on
the new ROM instead, `gbcli run --session game --from boot` (breakpoints and
watches kept, following their labels).

Constants other files assemble against (in ds, IF, REPT) belong in include/:
an EXPORTed symbol is resolved only at link time, so it isn't constant there.
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
        assert_eq!(seen, 28, "every code in the closed set");
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
            RgbgfxAgrees,
            SharedShade,
            PaletteFile,
            ShadeStretched,
            GbcPalettes,
            GbcTileColors,
            DmgOverrideSize,
            ShadesUnpinned,
            PixelsText,
        ] {
            assert!(text.contains(limit.name()), "{} missing", limit.name());
        }
        // The shade rules as shades.rs applies them (Ben's rulings,
        // 2026-11-07): declared, known, stretched, one shade per color in
        // the whole project, shared shades warned, RGB555 colors one.
        for needle in [
            "assets/palettes.txt",
            "pea greens",
            "GB Studio",
            "Pocket",
            "stretched",
            "whole project",
            "shades=",
            "shared_shade",
            "RGB555",
            // Shared only across images: one warning without an image
            // (leaf GTXXyr); and a comment is '#' then a space or a tab.
            "across images",
            "space or tab",
            // The named-palette file (leaf WmNCxc): the reserved dmg line,
            // #rgb, assignments, a fifth color an error, and what a DMG
            // build does with the rest.
            "dmg:",
            "#rgb is #rrggbb",
            "bg/town.png = forest",
            "a fifth",
            "as DMG art",
        ] {
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

    /// The GBC section (leaf YI5gQ6): the two kinds of art, the override,
    /// every GBC label, where GBCLI_GBC comes from, a palette swap, and one
    /// ROM verified in a session per model.
    #[test]
    fn the_images_topic_carries_the_gbc_convention_labels_and_recipes() {
        let text = Topic::from_name("images").expect("an images topic").text();
        for needle in [
            "build --gbc",
            "dual mode",
            "color art",
            "indexed art",
            "town.dmg.png",
            "TownTilesGbc",
            "TownAttrmap",
            "TownPalettes",
            "HeroTilesGbc",
            "ObjPalettesGbc",
            "PaletteForest",
            "FontAttrs",
            "rgbasm -D GBCLI_GBC=1",
            "IF DEF(GBCLI_GBC)",
            // A palette swap: the named palette into palette RAM.
            "rBCPS",
            "PaletteEmber",
            // An indexed sheet's swap: its own copy, reordered by its OBP.
            "HeroPaletteIce",
            "reordered by HERO_OBP",
            "color art only",
            // One ROM, two sessions (restore refuses a model change).
            "--model dmg",
            "--model gbc",
            // Agent eval run 4: tile indices count from the image's own
            // first tile, so a program loading tiles at a base offsets them.
            "own first tile",
            "_TILES",
            // GB Studio's key green, named, never keyed.
            "#65ff00",
        ] {
            assert!(text.contains(needle), "{needle} missing:\n{text}");
        }
        assert!(text.lines().all(|l| l.chars().count() <= 80), "{text}");
    }

    /// The examples' stumbles (leaves GQFHVi, zqarpL): color art in a plain
    /// build, shared shades across named palettes, 8x8 sheets, a
    /// manifest's palettes.dmg, --gbc's ROM cost, and what each summary
    /// row counts.
    #[test]
    fn the_images_topic_says_what_the_example_games_needed() {
        let text = Topic::from_name("images").expect("an images topic").text();
        for needle in [
            "a plain build derives",
            "both builds",
            "different named palettes",
            "shot.8x8.png",
            "8x8 objects",
            "palettes.dmg",
            "roughly doubles",
            "toolchain diagnostics",
        ] {
            assert!(text.contains(needle), "{needle} missing:\n{text}");
        }
        assert!(
            !text.contains("error\nwithout --gbc") && !text.contains("an error without --gbc"),
            "{text}"
        );
        assert!(text.lines().all(|l| l.chars().count() <= 80), "{text}");
    }

    /// Ben's later rulings of 2026-11-08 (leaves mjBkTn, Vho2Gc): the
    /// project-wide object size and where a manifest sets it, the plain
    /// build's image-wide shared shades, and that plain and --gbc tile data
    /// may differ.
    #[test]
    fn the_images_topic_says_the_object_size_and_the_plain_builds_shares() {
        let text = Topic::from_name("images").expect("an images topic").text();
        for needle in [
            "objects: 8x8",
            "LCDC_OBJ_16",
            "a manifest's objects",
            "anywhere in the image",
            "tile data may differ",
        ] {
            assert!(text.contains(needle), "{needle} missing:\n{text}");
        }
        assert!(text.lines().all(|l| l.chars().count() <= 80), "{text}");
        let schema = super::super::schema::SCHEMA;
        assert!(schema.contains("objects?: \"8x8\"|\"8x16\""), "{schema}");
    }

    /// Review pix15 and img15: without the objects line, sheets cut in
    /// both sizes warn; a .pixels name is found in any case; a hardware
    /// problem's line= is the block that drew the tile's colors.
    #[test]
    fn the_images_topic_says_mixed_object_sizes_warn_and_how_pixel_text_is_located() {
        let text = Topic::from_name("images").expect("an images topic").text();
        for needle in [
            "sheets cut in both sizes warn",
            "in any case, as .png",
            "drew the most legend characters of the\n                      first tile",
        ] {
            assert!(text.contains(needle), "{needle} missing:\n{text}");
        }
        // Review of 448a716: shared_shade warns once a shade in one image
        // (shared::find), once an image only under --gbc (per tile).
        let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
        for needle in [
            "in a plain build, warns once a shade",
            "Indexed art (and a .dmg.png) warns once a shade",
        ] {
            assert!(flat.contains(needle), "{needle} missing:\n{text}");
        }
        assert!(text.lines().all(|l| l.chars().count() <= 80), "{text}");
    }

    /// Review pix15 item 9: --tiles keys are legend tiles, which the build
    /// may merge (RGB555, one DMG shade), so the help doesn't call them the
    /// build's tile numbers outright.
    #[test]
    fn the_pixels_topic_says_when_tile_keys_are_the_builds_tile_numbers() {
        let text = Topic::from_name("pixels").expect("a pixels topic").text();
        for needle in ["RGB555", "#fefefe", "one DMG shade"] {
            assert!(text.contains(needle), "{needle} missing:\n{text}");
        }
        assert!(text.lines().all(|l| l.chars().count() <= 80), "{text}");
    }

    /// "GBC" is the user-facing name (Ben, 2026-11-07); the topic maps it to
    /// Pan Docs' CGB and to rgbfix's -c (dual mode) and -C (GBC only).
    #[test]
    fn the_rgbds_topic_maps_gbc_to_cgb_and_rgbfixs_flags() {
        let text = Topic::from_name("rgbds").expect("an rgbds topic").text();
        for needle in [
            "gbcli build game --gbc",
            "rgbfix -c",
            "$0143 = $80",
            "the GBC the CGB",
            "rgbfix -C (GBC only",
            "--model gbc",
        ] {
            assert!(text.contains(needle), "{needle} missing");
        }
        assert!(text.lines().all(|l| l.chars().count() <= 80), "{text}");
    }

    #[test]
    fn the_flash_topic_lists_every_board_and_says_how_to_use_one() {
        let text = Topic::from_name("flash").expect("a flash topic").text();
        for name in gbcli_core::flash::FlashProfile::names() {
            assert!(text.contains(&format!("  {name} ")), "{name} missing");
        }
        for needle in ["--flash", "power-cycle --sav", ".flash.gb", "warning:"] {
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
