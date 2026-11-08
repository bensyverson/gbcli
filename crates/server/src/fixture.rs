//! Test fixtures: a temporary project, a bundle, and a session over the
//! committed V1 ROM (one Work loop that counts frames in `wFrame`).

use std::fs;
use std::path::PathBuf;

use gbcli_bundle::{Bundle, RomSource};
use gbcli_core::Model;

pub struct Fixture {
    pub dir: PathBuf,
    pub bundle: Bundle,
    /// The flash board sessions are created on, if any.
    pub flash: Option<String>,
}

pub const V1: &[u8] = include_bytes!("../../engine/tests/roms/rebuild_v1.gb");
pub const V1_SYM: &[u8] = include_bytes!("../../engine/tests/roms/rebuild_v1.sym");
/// V1 with an instruction inserted, so `Main.loop` and `Work` move.
pub const V2: &[u8] = include_bytes!("../../engine/tests/roms/rebuild_v2.gb");
pub const V2_SYM: &[u8] = include_bytes!("../../engine/tests/roms/rebuild_v2.sym");
/// V1 with a routine appended, so no live symbol moves.
pub const V3: &[u8] = include_bytes!("../../engine/tests/roms/rebuild_v3.gb");
pub const V3_SYM: &[u8] = include_bytes!("../../engine/tests/roms/rebuild_v3.sym");
/// Logs the held buttons at every vblank (`joypad.asm`).
pub const JOYPAD: &[u8] = include_bytes!("../../engine/tests/roms/joypad.gb");
/// Writes SCX and BGP on known lines every frame, and four registers on line
/// 144 (`raster.asm`).
pub const RASTER: &[u8] = include_bytes!("../../engine/tests/roms/raster.gb");
pub const RASTER_SYM: &str = include_str!("../../engine/tests/roms/raster.sym");
/// Executes STOP with the direction row selected, so only a direction key
/// wakes it (`stop_wake.asm`).
pub const STOP_WAKE: &[u8] = include_bytes!("../../sameboy/tests/roms/stop_wake.gb");
/// Executes an illegal opcode after a few instructions (`illegal.asm`).
pub const ILLEGAL: &[u8] = include_bytes!("../../sameboy/tests/roms/illegal.gb");
/// Halts with IE = 0, so nothing can wake it (`halt.asm`).
pub const HALT: &[u8] = include_bytes!("../../engine/tests/roms/halt.gb");
/// Executes STOP with neither joypad row selected, so no key wakes it
/// (`stop.asm`).
pub const STOP: &[u8] = include_bytes!("../../sameboy/tests/roms/stop.gb");
/// Plays notes on all four channels for about 32 frames, then turns the APU
/// off and writes a register while it is off (`tune.asm`).
pub const TUNE: &[u8] = include_bytes!("../../audio/tests/roms/tune.gb");
/// Saves $42 to bank 12 of a flash chip with GB Studio's swapped-data
/// sequence, counting it in $C000 once it reads back (`flash_save.asm`).
pub const FLASH_SAVE: &[u8] = include_bytes!("../../engine/tests/roms/flash_save.gb");

impl Fixture {
    /// A project directory with `demo.gb` and a `demo` session created.
    pub fn new(name: &str) -> Fixture {
        Fixture::make(name, false)
    }

    /// As [`Fixture::new`], with `demo.sym` beside the ROM.
    pub fn with_sym(name: &str) -> Fixture {
        Fixture::make(name, true)
    }

    /// As [`Fixture::new`], over `rom` instead of V1.
    pub fn with_rom(name: &str, rom: &[u8]) -> Fixture {
        Fixture::build(name, rom, false, Model::DmgB, None)
    }

    /// As [`Fixture::with_rom`], on another model.
    pub fn with_model(name: &str, rom: &[u8], model: Model) -> Fixture {
        Fixture::build(name, rom, false, model, None)
    }

    /// As [`Fixture::with_rom`], the cart on a flash board.
    pub fn with_flash(name: &str, rom: &[u8], board: &str) -> Fixture {
        Fixture::build(name, rom, false, Model::DmgB, Some(board))
    }

    fn make(name: &str, sym: bool) -> Fixture {
        Fixture::build(name, V1, sym, Model::DmgB, None)
    }

    fn build(name: &str, rom: &[u8], sym: bool, model: Model, flash: Option<&str>) -> Fixture {
        let dir = std::env::temp_dir().join(format!("gbcli-server-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp project");
        fs::write(dir.join("demo.gb"), rom).expect("rom");
        if sym {
            fs::write(dir.join("demo.sym"), V1_SYM).expect("sym");
        }
        // Explicit: a GBCLI_DIR in the environment must not move the bundle.
        let bundle = Bundle::open_or_create_in(&dir, None).expect("bundle");
        let fixture = Fixture {
            dir,
            bundle,
            flash: flash.map(str::to_string),
        };
        fixture.create_session_on(sym, model);
        fixture
    }

    /// Creates the `demo` session over whatever `demo.gb` now holds.
    pub fn create_session(&self, sym: bool) {
        self.create_session_on(sym, Model::DmgB);
    }

    fn create_session_on(&self, sym: bool, model: Model) {
        let session = self.bundle.session("demo").expect("name");
        session
            .create(&RomSource {
                rom: self.dir.join("demo.gb"),
                sym: sym.then(|| self.dir.join("demo.sym")),
                model: Some(model),
                seed: Some(0),
                flash: self.flash.clone(),
            })
            .expect("session");
    }

    /// Rebuilds: replaces `demo.gb` and `demo.sym` in place.
    pub fn rebuild(&self, rom: &[u8], sym: &[u8]) {
        fs::write(self.dir.join("demo.gb"), rom).expect("rom");
        fs::write(self.dir.join("demo.sym"), sym).expect("sym");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}
