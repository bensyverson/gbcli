//! A temporary project with the committed V1 ROM and its `.sym`, for the
//! store's tests. It ignores `GBCLI_DIR`, so a developer's environment can't
//! redirect it.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use gbcli_core::Model;

use crate::bundle::Bundle;
use crate::session::RomSource;
use crate::store::SessionStore;

pub(crate) const ROM: &[u8] = include_bytes!("../../engine/tests/roms/rebuild_v1.gb");
pub(crate) const SYM: &str = include_str!("../../engine/tests/roms/rebuild_v1.sym");

pub(crate) struct Fixture {
    pub(crate) root: PathBuf,
    pub(crate) bundle: Bundle,
}

impl Fixture {
    pub(crate) fn new(name: &str) -> Fixture {
        let root =
            std::env::temp_dir().join(format!("gbcli-store-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("demo.gb"), ROM).unwrap();
        let bundle = Bundle::open_or_create_in(&root, None).unwrap();
        Fixture { root, bundle }
    }

    pub(crate) fn source(&self) -> RomSource {
        RomSource {
            rom: self.root.join("demo.gb"),
            sym: None,
            model: Model::DmgB,
            seed: 0,
        }
    }

    /// The `demo` session, created.
    pub(crate) fn demo(&self) -> SessionStore {
        let store = self.bundle.session("demo").unwrap();
        store.create(&self.source()).unwrap();
        store
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Backdates a file's or a directory's modification time.
pub(crate) fn backdate(path: &std::path::Path, by: Duration) {
    fs::File::open(path)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now() - by))
        .unwrap();
}

pub(crate) const DAY: Duration = Duration::from_secs(24 * 60 * 60);
