//! The filesystem backing of [`ProjectStore`]: a key is a path under the
//! project directory (or, absolute, outside it). Atomic replace is a temp
//! file and a rename, the lock is `flock`, and a lock file's modification
//! time is its last use. Finding the project (walking up for `.gbcli/`, or
//! `GBCLI_DIR`) is here too. This module, with `io.rs`, is where the bundle
//! touches files and the host clock; the decisions above it don't.

use std::collections::hash_map::RandomState;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::hash::{BuildHasher, Hasher};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::io::{append, atomic_write, ensure_dir, temp_path};
mod open;

use crate::project_store::{Clock, Entry, Guard, Key, Kind, LockError, ProjectStore, PutAllError};

/// A project directory as a [`ProjectStore`].
#[derive(Debug)]
pub struct FileStore {
    root: PathBuf,
}

impl FileStore {
    /// The store rooted at `root`, the project directory. `root` should be
    /// absolute: keys are located by joining them to it.
    pub fn new(root: PathBuf) -> FileStore {
        FileStore { root }
    }

    fn path(&self, key: &Key) -> PathBuf {
        if key.is_absolute() {
            PathBuf::from(key.as_str())
        } else if key.as_str().is_empty() {
            self.root.clone()
        } else {
            self.root.join(key.as_str())
        }
    }
}

/// The host's clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_secs(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
    }
}

/// Distinguishes incarnations drawn in one process.
static INCARNATION_SEQ: AtomicU64 = AtomicU64::new(0);

impl ProjectStore for FileStore {
    fn get(&self, key: &Key) -> io::Result<Vec<u8>> {
        fs::read(self.path(key))
    }

    fn get_range(&self, key: &Key, offset: u64, len: u64) -> io::Result<Vec<u8>> {
        let mut file = File::open(self.path(key))?;
        file.seek(SeekFrom::Start(offset))?;
        let mut bytes = Vec::new();
        file.take(len).read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    fn size(&self, key: &Key) -> io::Result<u64> {
        fs::metadata(self.path(key)).map(|m| m.len())
    }

    fn put(&self, key: &Key, bytes: &[u8]) -> io::Result<()> {
        atomic_write(&self.path(key), bytes)
    }

    fn put_all(&self, blobs: &[(Key, &[u8])]) -> Result<(), PutAllError> {
        let paths: Vec<PathBuf> = blobs.iter().map(|(key, _)| self.path(key)).collect();
        let temps: Vec<PathBuf> = paths.iter().map(|path| temp_path(path)).collect();
        let clean = || {
            for temp in &temps {
                let _ = fs::remove_file(temp);
            }
        };
        for (index, ((_, bytes), temp)) in blobs.iter().zip(&temps).enumerate() {
            if let Err(error) = fs::write(temp, bytes) {
                clean();
                return Err(PutAllError { index, error });
            }
        }
        for (index, (path, temp)) in paths.iter().zip(&temps).enumerate() {
            if let Err(error) = fs::rename(temp, path) {
                clean();
                return Err(PutAllError { index, error });
            }
        }
        Ok(())
    }

    fn append(&self, key: &Key, bytes: &[u8]) -> io::Result<()> {
        append(&self.path(key), bytes)
    }

    fn remove(&self, key: &Key) -> io::Result<()> {
        fs::remove_file(self.path(key))
    }

    fn remove_tree(&self, key: &Key) -> io::Result<()> {
        fs::remove_dir_all(self.path(key))
    }

    fn rename(&self, from: &Key, to: &Key) -> io::Result<()> {
        fs::rename(self.path(from), self.path(to))
    }

    fn list(&self, key: &Key) -> io::Result<Vec<Entry>> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(self.path(key))? {
            let entry = entry?;
            // A name that isn't UTF-8 can't be a key.
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            let kind = if entry.file_type().is_ok_and(|t| t.is_dir()) {
                Kind::Dir
            } else {
                Kind::Blob
            };
            entries.push(Entry { name, kind });
        }
        Ok(entries)
    }

    fn kind(&self, key: &Key) -> Option<Kind> {
        let meta = fs::metadata(self.path(key)).ok()?;
        Some(if meta.is_dir() { Kind::Dir } else { Kind::Blob })
    }

    fn make_dir(&self, key: &Key) -> io::Result<()> {
        ensure_dir(&self.path(key))
    }

    fn make_dirs(&self, key: &Key) -> io::Result<()> {
        fs::create_dir_all(self.path(key))
    }

    fn lock(&self, key: &Key, wait: Duration) -> Result<Guard, LockError> {
        let file = open_lock_file(&self.path(key)).map_err(LockError::Io)?;
        let start = Instant::now();
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Box::new(FileLock { file })),
                Err(TryLockError::WouldBlock) if start.elapsed() < wait => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(TryLockError::WouldBlock) => {
                    return Err(LockError::Busy {
                        waited: start.elapsed(),
                    });
                }
                Err(TryLockError::Error(e)) => return Err(LockError::Io(e)),
            }
        }
    }

    fn touch(&self, key: &Key, now: u64) -> io::Result<()> {
        OpenOptions::new()
            .write(true)
            .open(self.path(key))?
            .set_modified(UNIX_EPOCH + Duration::from_secs(now))
    }

    fn last_used(&self, key: &Key) -> Option<u64> {
        let modified = fs::metadata(self.path(key)).ok()?.modified().ok()?;
        Some(
            modified
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs()),
        )
    }

    /// Random, from the standard library's per-process random hash keys,
    /// mixed with a counter: unique across processes without a clock or a
    /// process id.
    fn new_incarnation(&self) -> u64 {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u64(INCARNATION_SEQ.fetch_add(1, Ordering::Relaxed));
        hasher.finish()
    }

    /// Resolves symbolic links, as the project's own path is resolved, so
    /// one file named two ways is one key. A path that doesn't exist yet
    /// (an image about to be written) is judged by its directory.
    fn key_of(&self, path: &Path) -> Key {
        let joined = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        };
        let resolved = fs::canonicalize(&joined)
            .ok()
            .or_else(|| {
                let parent = fs::canonicalize(joined.parent()?).ok()?;
                Some(parent.join(joined.file_name()?))
            })
            .unwrap_or_else(|| normalize(&joined));
        let project = fs::canonicalize(&self.root).unwrap_or_else(|_| self.root.clone());
        match resolved.strip_prefix(&project) {
            Ok(inside) => Key::new(inside.to_string_lossy()),
            Err(_) => Key::new(resolved.to_string_lossy()),
        }
    }

    fn locate(&self, key: &Key) -> PathBuf {
        self.path(key)
    }
}

/// An `flock` held on a lock file; dropping it unlocks.
#[derive(Debug)]
struct FileLock {
    file: File,
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

/// Opens the lock file, creating it if needed. A new one is dated to the Unix
/// epoch, meaning no use is recorded yet: taking the lock to prune or remove
/// must not make a session look used.
fn open_lock_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    match options.clone().create_new(true).open(path) {
        Ok(file) => {
            file.set_modified(UNIX_EPOCH)?;
            Ok(file)
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => options.open(path),
        Err(e) => Err(e),
    }
}

pub(super) fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Scratch {
            let dir =
                std::env::temp_dir().join(format!("gbcli-files-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Scratch(dir)
        }

        fn store(&self) -> FileStore {
            FileStore::new(self.0.clone())
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_key_is_a_path_under_the_root_or_an_absolute_one() {
        let s = Scratch::new("locate");
        let store = s.store();
        assert_eq!(store.locate(&Key::root()), s.0);
        assert_eq!(
            store.locate(&Key::new(".gbcli/sessions")),
            s.0.join(".gbcli").join("sessions")
        );
        assert_eq!(
            store.locate(&Key::new("/elsewhere/game.gb")),
            PathBuf::from("/elsewhere/game.gb")
        );
    }

    #[test]
    fn a_path_inside_the_project_is_a_relative_key_and_one_outside_stays_absolute() {
        let s = Scratch::new("key-of");
        let store = s.store();
        fs::create_dir_all(s.0.join("build")).unwrap();
        fs::write(s.0.join("build").join("game.gb"), b"rom").unwrap();
        assert_eq!(
            store.key_of(&s.0.join("build").join("game.gb")),
            Key::new("build/game.gb")
        );
        assert_eq!(
            store.key_of(Path::new("./build/../build/game.gb")),
            Key::new("build/game.gb")
        );
        // Not there yet: its directory decides.
        assert_eq!(
            store.key_of(&s.0.join("build").join("shot.png")),
            Key::new("build/shot.png")
        );
        let outside = Path::new("/nowhere-gbcli/x.gb");
        assert_eq!(store.key_of(outside), Key::new("/nowhere-gbcli/x.gb"));
    }

    #[test]
    fn put_replaces_a_blob_and_never_creates_its_parent() {
        let s = Scratch::new("put");
        let store = s.store();
        let key = Key::new("a.bin");
        store.put(&key, b"one").unwrap();
        store.put(&key, b"two").unwrap();
        assert_eq!(store.get(&key).unwrap(), b"two");
        assert_eq!(store.size(&key).unwrap(), 3);
        assert_eq!(store.get_range(&key, 1, 8).unwrap(), b"wo");
        let orphan = Key::new("gone/session.json");
        assert!(store.put(&orphan, b"{}").is_err());
        assert!(store.append(&orphan, b"{}").is_err());
        assert_eq!(store.kind(&Key::new("gone")), None);
        let missing = store.get(&Key::new("nope")).unwrap_err();
        assert_eq!(missing.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn put_all_writes_nothing_when_one_blob_fails() {
        let s = Scratch::new("put-all");
        let store = s.store();
        store.put(&Key::new("a.sav"), b"old").unwrap();
        let blobs: [(Key, &[u8]); 2] = [
            (Key::new("a.sav"), b"new"),
            (Key::new("missing-dir/a.flash.gb"), b"chip"),
        ];
        let err = store.put_all(&blobs).unwrap_err();
        assert_eq!(err.index, 1);
        assert_eq!(store.get(&Key::new("a.sav")).unwrap(), b"old");
        let names: Vec<String> = store
            .list(&Key::root())
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert_eq!(names, vec!["a.sav".to_string()], "a temp file is left");
    }

    #[test]
    fn directories_list_their_children_and_move_and_go_as_a_tree() {
        let s = Scratch::new("tree");
        let store = s.store();
        let dir = Key::new("sessions");
        assert_eq!(
            store.list(&dir).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        store.make_dir(&dir).unwrap();
        store.make_dir(&dir).unwrap();
        assert!(store.make_dir(&Key::new("x/y")).is_err(), "a parent made");
        store.make_dir(&dir.join("demo")).unwrap();
        store
            .put(&dir.join("demo").join("session.json"), b"{}")
            .unwrap();
        store.put(&dir.join("note"), b"").unwrap();
        let mut listed = store.list(&dir).unwrap();
        listed.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(
            listed,
            vec![
                Entry {
                    name: "demo".into(),
                    kind: Kind::Dir
                },
                Entry {
                    name: "note".into(),
                    kind: Kind::Blob
                },
            ]
        );
        store.make_dirs(&Key::new("removed/deep")).unwrap();
        store
            .rename(&dir.join("demo"), &Key::new("removed/demo-1"))
            .unwrap();
        assert_eq!(store.kind(&dir.join("demo")), None);
        assert_eq!(
            store.kind(&Key::new("removed/demo-1/session.json")),
            Some(Kind::Blob)
        );
        store.remove_tree(&Key::new("removed")).unwrap();
        assert_eq!(store.kind(&Key::new("removed")), None);
        store.remove(&dir.join("note")).unwrap();
        assert!(store.list(&dir).unwrap().is_empty());
    }

    #[test]
    fn a_lock_is_exclusive_records_no_use_until_touched_and_frees_on_drop() {
        let s = Scratch::new("lock");
        let store = s.store();
        let key = Key::new("demo.lock");
        let held = store.lock(&key, Duration::from_millis(1)).unwrap();
        assert_eq!(store.last_used(&key), Some(0), "a new lock is unused");
        let Err(LockError::Busy { .. }) = store.lock(&key, Duration::from_millis(10)) else {
            panic!("a second holder got the lock");
        };
        store.touch(&key, 1_700_000_000).unwrap();
        assert_eq!(store.last_used(&key), Some(1_700_000_000));
        drop(held);
        assert!(store.lock(&key, Duration::from_millis(10)).is_ok());
    }

    #[test]
    fn incarnations_differ_call_to_call() {
        let s = Scratch::new("incarnation");
        let store = s.store();
        let a = store.new_incarnation();
        let b = store.new_incarnation();
        assert_ne!(a, b);
    }

    #[test]
    fn the_system_clock_is_after_2026() {
        assert!(SystemClock.now_secs() > 1_767_225_600);
    }
}
