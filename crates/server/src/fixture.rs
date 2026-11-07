//! Test fixtures: a temporary project, a bundle, and a session over the
//! committed V1 ROM (one Work loop that counts frames in `wFrame`).

use std::fs;
use std::path::PathBuf;

use gbcli_bundle::{Bundle, RomSource};
use gbcli_core::Model;

pub struct Fixture {
    pub dir: PathBuf,
    pub bundle: Bundle,
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
        Fixture::build(name, rom, false)
    }

    fn make(name: &str, sym: bool) -> Fixture {
        Fixture::build(name, V1, sym)
    }

    fn build(name: &str, rom: &[u8], sym: bool) -> Fixture {
        let dir = std::env::temp_dir().join(format!("gbcli-server-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp project");
        fs::write(dir.join("demo.gb"), rom).expect("rom");
        if sym {
            fs::write(dir.join("demo.sym"), V1_SYM).expect("sym");
        }
        // Explicit: a GBCLI_DIR in the environment must not move the bundle.
        let bundle = Bundle::open_or_create_in(&dir, None).expect("bundle");
        let fixture = Fixture { dir, bundle };
        fixture.create_session(sym);
        fixture
    }

    /// Creates the `demo` session over whatever `demo.gb` now holds.
    pub fn create_session(&self, sym: bool) {
        let session = self.bundle.session("demo").expect("name");
        session
            .create(&RomSource {
                rom: self.dir.join("demo.gb"),
                sym: sym.then(|| self.dir.join("demo.sym")),
                model: Model::DmgB,
                seed: 0,
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
