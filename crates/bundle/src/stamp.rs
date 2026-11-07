//! Which ROM and `.sym` files a machine was built from, cheaply rechecked
//! (design §1): a server keeps a machine warm only while the files on disk
//! are still the ones it read, so a rebuild is loaded as a fresh process
//! would load it.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use gbcli_engine::sha256;
use gbcli_protocol::{SessionFile, Sha256};

use crate::store::SessionStore;

/// A file written this close to its stamp could be rewritten within the same
/// mtime tick (two seconds on FAT), so its contents are hashed as well.
const RACY: Duration = Duration::from_secs(2);

/// The session's ROM and `.sym` as they were when stamped.
#[derive(Clone, Debug)]
pub struct FilesStamp {
    files: Vec<FileStamp>,
}

#[derive(Clone, Debug)]
struct FileStamp {
    path: PathBuf,
    seen: Option<Seen>,
}

/// What a stat (and, when racy, a read) found; `None` above is a file that
/// couldn't be read.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Seen {
    len: u64,
    modified: SystemTime,
    /// The contents' hash, kept only while the stamp is racy.
    digest: Option<Sha256>,
}

impl SessionStore {
    /// Stamps the files a machine for `meta` is built from. Take it before
    /// reading them: a rebuild between the two then reads as a change.
    pub fn files_stamp(&self, meta: &SessionFile) -> FilesStamp {
        let mut paths = vec![self.resolve(Path::new(&meta.rom))];
        paths.extend(meta.sym.as_deref().map(|p| self.resolve(Path::new(p))));
        FilesStamp {
            files: paths
                .into_iter()
                .map(|path| FileStamp {
                    seen: look(&path, SystemTime::now()),
                    path,
                })
                .collect(),
        }
    }
}

impl FilesStamp {
    /// Whether every file is still the one stamped. A file that couldn't be
    /// read is never current. A racy stamp that still matches, and is no
    /// longer racy, drops its hash so later checks are a stat.
    pub fn still_current(&mut self) -> bool {
        self.files.iter_mut().all(FileStamp::still_current)
    }
}

impl FileStamp {
    fn still_current(&mut self) -> bool {
        let Some(seen) = &mut self.seen else {
            return false;
        };
        let Ok(stat) = fs::metadata(&self.path) else {
            return false;
        };
        let Ok(modified) = stat.modified() else {
            return false;
        };
        if stat.len() != seen.len || modified != seen.modified {
            return false;
        }
        let Some(digest) = &seen.digest else {
            return true;
        };
        let same = fs::read(&self.path).is_ok_and(|bytes| sha256(&bytes) == *digest);
        if same && !racy(modified, SystemTime::now()) {
            seen.digest = None;
        }
        same
    }
}

fn look(path: &Path, now: SystemTime) -> Option<Seen> {
    let stat = fs::metadata(path).ok()?;
    let modified = stat.modified().ok()?;
    let digest = if racy(modified, now) {
        Some(sha256(&fs::read(path).ok()?))
    } else {
        None
    };
    Some(Seen {
        len: stat.len(),
        modified,
        digest,
    })
}

/// Whether a file modified at `modified` could change again, by `now`,
/// without its mtime changing.
fn racy(modified: SystemTime, now: SystemTime) -> bool {
    now.duration_since(modified).map_or(true, |age| age < RACY)
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use super::*;
    use crate::test_fixture::{Fixture, ROM};

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

    #[test]
    fn an_old_file_touched_to_a_new_mtime_is_a_change() {
        let f = Fixture::new("stamp-mtime");
        let rom = f.root.join("demo.gb");
        set_modified(&rom, SystemTime::now() - Duration::from_secs(3600));
        let mut s = stamp(&f);
        set_modified(&rom, SystemTime::now());
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
