//! Which ROM and `.sym` a machine was built from, rechecked cheaply
//! (design §1): a server keeps a machine warm only while the store still
//! holds the bytes it read, so a rebuild is loaded as a fresh process would
//! load it.
//!
//! A check asks the store for each file's [`ChangeToken`] first, and reads
//! and hashes the file only when the token doesn't vouch for the one
//! stamped. The token is opaque here: the stamp reads no clock and no file
//! times, and a backing with neither answers it the same way.

use std::path::Path;
use std::sync::Arc;

use gbcli_engine::{Machine, sha256};
use gbcli_protocol::{SessionFile, Sha256};

use crate::project_store::{ChangeToken, Key, ProjectStore};
use crate::store::SessionStore;

/// The session's ROM and `.sym` as they were when stamped.
#[derive(Clone, Debug)]
pub struct FilesStamp {
    store: Arc<dyn ProjectStore>,
    /// The ROM first, then the `.sym` if the session has one.
    files: Vec<FileStamp>,
}

#[derive(Clone, Debug)]
struct FileStamp {
    /// The session's own path, so a symbolic link is followed as it stands
    /// at each check.
    key: Key,
    /// The token last seen with these bytes; `None` for a file that
    /// couldn't be read.
    token: Option<ChangeToken>,
    /// The bytes' hash, when known: what tells a touch from a rebuild once
    /// the token changes.
    digest: Option<Sha256>,
}

impl SessionStore {
    /// Stamps the files a machine for `meta` is built from. Take it before
    /// reading them, and hand the built machine to [`FilesStamp::built`]: a
    /// rebuild in between then reads as a change.
    ///
    /// The ROM isn't read here, since the build reads and hashes it; the
    /// `.sym`, which the build doesn't hash, is.
    pub fn files_stamp(&self, meta: &SessionFile) -> FilesStamp {
        let store = self.bundle().store().clone();
        let rom = self.bundle().file_key(Path::new(&meta.rom));
        let mut files = vec![FileStamp::new(store.as_ref(), rom, false)];
        if let Some(sym) = meta.sym.as_deref() {
            let key = self.bundle().file_key(Path::new(sym));
            files.push(FileStamp::new(store.as_ref(), key, true));
        }
        FilesStamp { store, files }
    }
}

impl FilesStamp {
    /// Records the ROM's hash from the machine built after the stamp, so a
    /// later check can tell a touch from a rebuild without the stamp hashing
    /// the ROM a second time.
    pub fn built(&mut self, machine: &Machine) {
        if let Some(rom) = self.files.first_mut() {
            rom.digest = Some(machine.rom_sha256());
        }
    }

    /// Whether every file still holds the bytes stamped. A file that
    /// couldn't be read is never current, and neither is one whose token
    /// changed when its hash isn't known.
    pub fn still_current(&mut self) -> bool {
        let store = self.store.as_ref();
        self.files.iter_mut().all(|file| file.still_current(store))
    }
}

impl FileStamp {
    fn new(store: &dyn ProjectStore, key: Key, hash: bool) -> FileStamp {
        let token = store.change_token(&key).ok();
        // Read after the token: a write in between then reads as a change.
        let digest = if hash && token.is_some() {
            digest(store, &key)
        } else {
            None
        };
        FileStamp { key, token, digest }
    }

    fn still_current(&mut self, store: &dyn ProjectStore) -> bool {
        let Some(stamped) = &self.token else {
            return false;
        };
        let Ok(now) = store.change_token(&self.key) else {
            return false;
        };
        if now.vouches_for(stamped) {
            return true;
        }
        let Some(known) = self.digest else {
            return false;
        };
        let same = digest(store, &self.key) == Some(known);
        if same {
            // Taken before the read, so it can only understate what was read.
            self.token = Some(now);
        }
        same
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
    use crate::test_fixture::{CountingStore, Fixture, ROM, SYM};

    /// A stamp as the server takes one: before the build, then told it.
    fn stamp_built(store: &SessionStore) -> FilesStamp {
        let meta = store.read_meta().unwrap();
        let mut stamp = store.files_stamp(&meta);
        stamp.built(&store.machine(&meta).unwrap());
        stamp
    }

    fn stamp(f: &Fixture) -> FilesStamp {
        stamp_built(&f.demo())
    }

    fn set_modified(path: &Path, at: SystemTime) {
        File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(at)
            .unwrap();
    }

    fn an_hour_ago() -> SystemTime {
        SystemTime::now() - Duration::from_secs(3600)
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
        // In place, as rgbfix writes: same inode.
        fs::write(&rom, edited).unwrap();
        set_modified(&rom, modified);
        assert!(
            !s.still_current(),
            "same size, mtime and inode, other bytes"
        );
    }

    /// The stamp is the bytes, not the file's times: a touch that leaves
    /// them alone builds the same machine, so the warm one stays.
    #[test]
    fn a_touch_that_keeps_the_bytes_is_not_a_change() {
        let f = Fixture::new("stamp-mtime");
        let rom = f.root.join("demo.gb");
        set_modified(&rom, an_hour_ago());
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

    /// A build that writes a new file and retargets a link at the session's
    /// ROM path is a change: the link is followed at each check.
    #[test]
    fn a_retargeted_rom_symlink_is_a_change() {
        let f = Fixture::new("stamp-symlink");
        let store = f.demo();
        let rom = f.root.join("demo.gb");
        fs::write(f.root.join("v2.gb"), ROM).unwrap();
        fs::remove_file(&rom).unwrap();
        std::os::unix::fs::symlink("v2.gb", &rom).unwrap();
        let mut s = stamp_built(&store);
        assert!(s.still_current());
        let mut v3 = ROM.to_vec();
        v3[0x200] ^= 0xFF;
        fs::write(f.root.join("v3.gb"), &v3).unwrap();
        fs::remove_file(&rom).unwrap();
        std::os::unix::fs::symlink("v3.gb", &rom).unwrap();
        assert!(!s.still_current(), "the stamp still reads v2");
    }

    /// The warm path's cost: an unchanged ROM past its racy window is
    /// checked by its token alone, not read.
    #[test]
    fn an_unchanged_settled_rom_is_checked_without_reading_it() {
        let f = Fixture::new("stamp-cost");
        fs::write(f.root.join("demo.sym"), SYM).unwrap();
        set_modified(&f.root.join("demo.gb"), an_hour_ago());
        set_modified(&f.root.join("demo.sym"), an_hour_ago());
        f.demo();
        let (bundle, counts) = CountingStore::bundle(&f.root);
        let store = bundle.session("demo").unwrap();
        let mut s = stamp_built(&store);
        counts.take_reads("demo.gb");
        counts.take_reads("demo.sym");
        for _ in 0..3 {
            assert!(s.still_current());
        }
        assert_eq!(counts.take_reads("demo.gb"), 0);
        assert_eq!(counts.take_reads("demo.sym"), 0);
    }

    /// A cold start reads (and hashes) the ROM once, in the build.
    #[test]
    fn a_cold_start_reads_the_rom_once() {
        let f = Fixture::new("stamp-cold");
        f.demo();
        fs::write(f.root.join("demo.gb"), ROM).unwrap();
        let (bundle, counts) = CountingStore::bundle(&f.root);
        let store = bundle.session("demo").unwrap();
        counts.take_reads("demo.gb");
        stamp_built(&store);
        assert_eq!(counts.take_reads("demo.gb"), 1);
    }

    /// A file stamped while racy is read until a check finds it settled
    /// with the same bytes; from then on a check is its token.
    #[test]
    fn a_racy_rom_is_read_until_it_settles() {
        let f = Fixture::new("stamp-settle");
        f.demo();
        fs::write(f.root.join("demo.gb"), ROM).unwrap();
        let (bundle, counts) = CountingStore::bundle(&f.root);
        let store = bundle.session("demo").unwrap();
        let mut s = stamp_built(&store);
        counts.take_reads("demo.gb");
        assert!(s.still_current());
        assert_eq!(counts.take_reads("demo.gb"), 1, "racy: read");
        set_modified(&f.root.join("demo.gb"), an_hour_ago());
        assert!(s.still_current());
        assert_eq!(counts.take_reads("demo.gb"), 1, "settled: read once more");
        assert!(s.still_current());
        assert_eq!(counts.take_reads("demo.gb"), 0, "then its token");
    }

    /// Without the built machine's hash a changed token can't be told from
    /// a rebuild, so it is one.
    #[test]
    fn a_changed_token_without_a_known_hash_is_a_change() {
        let f = Fixture::new("stamp-unbuilt");
        let rom = f.root.join("demo.gb");
        set_modified(&rom, an_hour_ago());
        let store = f.demo();
        let mut s = store.files_stamp(&store.read_meta().unwrap());
        assert!(s.still_current());
        set_modified(&rom, SystemTime::now());
        assert!(!s.still_current());
    }
}
