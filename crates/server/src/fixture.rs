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

const V1: &[u8] = include_bytes!("../../engine/tests/roms/rebuild_v1.gb");

impl Fixture {
    /// A project directory with `demo.gb` and a `demo` session created.
    pub fn new(name: &str) -> Fixture {
        let dir = std::env::temp_dir().join(format!("gbcli-server-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp project");
        fs::write(dir.join("demo.gb"), V1).expect("rom");
        let bundle = Bundle::open_or_create(&dir).expect("bundle");
        let session = bundle.session("demo").expect("name");
        session
            .create(&RomSource {
                rom: dir.join("demo.gb"),
                sym: None,
                model: Model::DmgB,
                seed: 0,
            })
            .expect("session");
        Fixture { dir, bundle }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}
