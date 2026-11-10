//! Which ROM and `.sym` a machine was built from, rechecked by content
//! (design §1): a server keeps a machine warm only while the store still
//! holds the bytes it read, so a rebuild is loaded as a fresh process would
//! load it. The stamp hashes what the store returns, so it needs no clock
//! and no file times, and a backing with neither answers it the same way.

use std::path::Path;
use std::sync::Arc;

use gbcli_engine::sha256;
use gbcli_protocol::{SessionFile, Sha256};

use crate::project_store::{Key, ProjectStore};
use crate::store::SessionStore;

/// The session's ROM and `.sym` as they were when stamped.
#[derive(Clone, Debug)]
pub struct FilesStamp {
    store: Arc<dyn ProjectStore>,
    files: Vec<FileStamp>,
}

#[derive(Clone, Debug)]
struct FileStamp {
    key: Key,
    /// The contents' hash; `None` for a file that couldn't be read.
    digest: Option<Sha256>,
}

impl SessionStore {
    /// Stamps the files a machine for `meta` is built from. Take it before
    /// reading them: a rebuild between the two then reads as a change.
    pub fn files_stamp(&self, meta: &SessionFile) -> FilesStamp {
        let store = self.bundle().store().clone();
        let mut keys = vec![self.bundle().file_key(Path::new(&meta.rom))];
        keys.extend(
            meta.sym
                .as_deref()
                .map(|p| self.bundle().file_key(Path::new(p))),
        );
        let files = keys
            .into_iter()
            .map(|key| FileStamp {
                digest: digest(store.as_ref(), &key),
                key,
            })
            .collect();
        FilesStamp { store, files }
    }
}

impl FilesStamp {
    /// Whether every file still holds the bytes stamped. A file that
    /// couldn't be read is never current.
    pub fn still_current(&mut self) -> bool {
        self.files.iter().all(|file| {
            file.digest.is_some() && digest(self.store.as_ref(), &file.key) == file.digest
        })
    }
}

fn digest(store: &dyn ProjectStore, key: &Key) -> Option<Sha256> {
    store.get(key).ok().map(|bytes| sha256(&bytes))
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::time::{Duration, SystemTime};

    use super::*;
    use crate::test_fixture::{Fixture, ROM, SYM};

    fn stamp(f: &Fixture) -> FilesStamp {
        let store = f.demo();
        store.files_stamp(&store.read_meta().unwrap())
    }

    fn set_modified(path: &Path, at: SystemTime) {
        File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(at)
            .unwrap();
    }

    #[test]
    fn untouched_files_are_current() {
        let f = Fixture::new("stamp-same");
        let mut s = stamp(&f);
        assert!(s.still_current());
        assert!(s.still_current());
    }

    #[test]
    fn a_rewrite_of_another_length_is_a_change() {
        let f = Fixture::new("stamp-len");
        let mut s = stamp(&f);
        let mut longer = ROM.to_vec();
        longer.extend_from_slice(&[0; 16]);
        fs::write(f.root.join("demo.gb"), longer).unwrap();
        assert!(!s.still_current());
    }

    #[test]
    fn a_rewrite_within_the_same_mtime_tick_is_a_change() {
        let f = Fixture::new("stamp-racy");
        let rom = f.root.join("demo.gb");
        let mut s = stamp(&f);
        let modified = fs::metadata(&rom).unwrap().modified().unwrap();
        let mut edited = ROM.to_vec();
        edited[0x150] ^= 0xFF;
        fs::write(&rom, edited).unwrap();
        set_modified(&rom, modified);
        assert!(!s.still_current(), "same size and mtime, other bytes");
    }

    /// The stamp is the bytes, not the file's times: a touch that leaves
    /// them alone builds the same machine, so the warm one stays.
    #[test]
    fn a_touch_that_keeps_the_bytes_is_not_a_change() {
        let f = Fixture::new("stamp-mtime");
        let rom = f.root.join("demo.gb");
        set_modified(&rom, SystemTime::now() - Duration::from_secs(3600));
        let mut s = stamp(&f);
        set_modified(&rom, SystemTime::now());
        assert!(s.still_current());
    }

    #[test]
    fn a_sym_rewritten_to_the_same_length_is_a_change() {
        let f = Fixture::new("stamp-sym");
        fs::write(f.root.join("demo.sym"), SYM).unwrap();
        let mut s = stamp(&f);
        assert!(s.still_current());
        let edited = SYM.replacen('0', "1", 1);
        assert_eq!(edited.len(), SYM.len());
        fs::write(f.root.join("demo.sym"), edited).unwrap();
        assert!(!s.still_current());
    }

    #[test]
    fn a_removed_rom_is_a_change() {
        let f = Fixture::new("stamp-gone");
        let mut s = stamp(&f);
        fs::remove_file(f.root.join("demo.gb")).unwrap();
        assert!(!s.still_current());
    }
}
