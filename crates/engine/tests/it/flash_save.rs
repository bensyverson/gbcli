//! A real GB Studio batteryless save on the flash model (leaf L00RuF): GB
//! Studio's default project, exported with "Save game data to ROM", saved at
//! its cave save point and continued after a power cycle. On a board with
//! D0/D1 swapped (the sequence gbvm hard-codes) the save survives; on a
//! straight board it is lost, and the chip's notes say why.
//!
//! The ROM is a committed export (MIT; `tests/roms/gbstudio/README.md`):
//! `GBCLI_GBSTUDIO_BATTERYLESS_ROM` names another, with its `.sym` beside it.

use std::path::PathBuf;

use gbcli_core::flash::{BusyTicks, FlashProfile, NoteKind};
use gbcli_core::{Budget, CoreConfig, Keys, Model, Region};
use gbcli_engine::{Backend, Battery, Flash, Machine, RunOptions};

/// Where GB Studio's save lands: banks 12-15 (`__start_save` = 12).
const SAVE_AREA: std::ops::Range<usize> = 0x3_0000..0x4_0000;
/// The cave's save point stands at (14, 11); the player talks to it from
/// (12, 11), facing right, and a loaded save puts the player back there.
const SAVED_AT: (u8, u8) = (12, 11);

fn rom_path() -> PathBuf {
    match std::env::var_os("GBCLI_GBSTUDIO_BATTERYLESS_ROM") {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/roms/gbstudio/demo.gb"),
    }
}

/// The ROM and its `.sym`, or a panic with the build steps.
fn demo() -> (Vec<u8>, String) {
    let path = rom_path();
    let missing = |what: &str, err: std::io::Error| -> ! {
        panic!(
            "the GB Studio batteryless {what} {} is missing ({err}); the committed copy is \
             crates/engine/tests/roms/gbstudio/ (see its README), or name a ROM with \
             GBCLI_GBSTUDIO_BATTERYLESS_ROM (the .sym must sit beside it)",
            path.display()
        )
    };
    let rom = std::fs::read(&path).unwrap_or_else(|e| missing("ROM", e));
    let sym_path = path.with_extension("sym");
    let sym = std::fs::read_to_string(&sym_path).unwrap_or_else(|e| missing("ROM's .sym", e));
    (rom, sym)
}

/// The WRAM offset of `_actors`, from the `.sym`.
fn actors_offset(sym: &str) -> usize {
    let line = sym
        .lines()
        .find(|l| l.split_whitespace().nth(1) == Some("_actors"))
        .expect("_actors in the .sym");
    let addr = line.split_whitespace().next().unwrap();
    let addr = u16::from_str_radix(addr.split(':').nth(1).unwrap(), 16).unwrap();
    usize::from(addr - 0xC000)
}

struct Game {
    m: Machine,
    actors: usize,
}

impl Game {
    fn new(profile: FlashProfile) -> Game {
        let (rom, sym) = demo();
        let mut config = CoreConfig::new(Model::DmgB, rom);
        config.flash = Some(FlashProfile {
            busy: BusyTicks::TYPICAL,
            ..profile
        });
        let m = Machine::new(Backend::sameboy(), config, Some(&sym)).expect("machine");
        Game {
            m,
            actors: actors_offset(&sym),
        }
    }

    fn frames(&mut self, n: u64) {
        self.m
            .run(&RunOptions::new(Budget::frames(n)))
            .expect("run");
    }

    fn press(&mut self, keys: Keys, wait: u64) {
        self.m.set_keys(keys);
        self.frames(4);
        self.m.set_keys(Keys::NONE);
        self.frames(wait);
    }

    /// The player's tile, as the engine keeps it: x at `_actors+2`, y at
    /// `_actors+4` (observed by walking the player, not from gbvm's source).
    fn at(&self) -> (u8, u8) {
        let wram = self.m.core().region(Region::Wram).unwrap().bytes;
        (wram[self.actors + 2], wram[self.actors + 4])
    }

    /// Holds `keys` until the player reaches `to`.
    fn walk(&mut self, keys: Keys, to: (u8, u8)) {
        self.m.set_keys(keys);
        for _ in 0..600 {
            if self.at() == to {
                self.m.set_keys(Keys::NONE);
                self.frames(2);
                return;
            }
            self.frames(1);
        }
        panic!("walking toward {to:?}, the player stuck at {:?}", self.at());
    }

    /// From power-on to the title's menu.
    fn open_menu(&mut self) {
        self.frames(900);
        self.press(Keys::START, 150);
    }

    /// New Game, walk to the cave's save point and save.
    fn save_in_the_cave(&mut self) {
        self.open_menu();
        self.press(Keys::A, 150); // New Game: outside, at (27, 26)
        // Around the pond's west side, along row 9, into the cave's mouth.
        self.walk(Keys::LEFT, (2, 26));
        self.walk(Keys::UP, (2, 9));
        self.walk(Keys::RIGHT, (24, 9));
        self.walk(Keys::UP, (9, 15)); // the cave, at its entrance
        self.frames(60);
        self.walk(Keys::UP, (9, 11));
        self.walk(Keys::RIGHT, SAVED_AT);
        self.press(Keys::A, 90); // "Save Game / Cancel"
        self.press(Keys::A, 240); // Save Game: erase and program
        for _ in 0..3 {
            self.press(Keys::A, 90); // dismiss the two messages
        }
    }

    /// Off and on, then Continue.
    fn power_cycle_and_continue(&mut self) {
        self.m
            .power_cycle(Battery::Keep, Flash::Keep, 0)
            .expect("power cycle");
        self.open_menu();
        self.press(Keys::DOWN, 20);
        self.press(Keys::A, 240); // Continue
    }

    fn save_area(&self) -> Vec<u8> {
        self.m.core().region(Region::Rom).unwrap().bytes[SAVE_AREA].to_vec()
    }
}

fn profile(name: &str) -> FlashProfile {
    FlashProfile::named(name).expect("named profile")
}

#[test]
#[ignore = "pre-release: drives a GB Studio ROM ~3,000 frames (10-12 s) and needs a local export (Ben, 2026-11-07)"]
fn a_gbstudio_save_survives_a_power_cycle_on_a_swapped_data_board() {
    let mut game = Game::new(profile("generic-wr-aaa-a9"));
    game.save_in_the_cave();
    assert!(
        game.save_area().iter().any(|&b| b != 0xFF),
        "the save was programmed into banks 12-15"
    );
    let notes = game.m.core_mut().take_flash_notes().unwrap();
    assert!(notes.notes.is_empty(), "{:?}", notes.notes);

    game.power_cycle_and_continue();
    assert_eq!(
        game.at(),
        SAVED_AT,
        "Continue put the player back at the save point"
    );
}

#[test]
#[ignore = "pre-release: drives a GB Studio ROM ~3,000 frames (10-12 s) and needs a local export (Ben, 2026-11-07)"]
fn the_same_save_fails_on_a_straight_board_and_the_chip_says_why() {
    let mut game = Game::new(profile("generic-wr-aaa-aa"));
    game.save_in_the_cave();
    assert!(
        game.save_area().iter().all(|&b| b == 0xFF),
        "nothing was programmed"
    );
    let notes = game.m.core_mut().take_flash_notes().unwrap();
    let first = notes.notes.first().expect("a note on the failed save");
    assert!(
        matches!(first.kind, NoteKind::DataLines { .. }),
        "{:?}",
        notes.notes
    );
    let text = first.to_string();
    assert!(text.contains("D0 and D1 swapped"), "{text}");
    assert!(text.contains("do nothing here"), "{text}");

    game.power_cycle_and_continue();
    assert_ne!(game.at(), SAVED_AT, "no save to continue from");
}
