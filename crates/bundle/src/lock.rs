//! The session lock (design §1): `flock` on `.gbcli/locks/<name>.lock`.
//! Every session request takes it, and taking it for a request records the
//! session's last use, which is what pruning judges idleness by.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::error::StoreError;
use crate::io::{ensure_dir, io_at};
use crate::store::SessionStore;

/// How long a writer waits for another command's lock before `session_busy`.
pub const LOCK_WAIT: Duration = Duration::from_secs(5);

/// Held while a command reads and writes a session; dropping it unlocks.
#[derive(Debug)]
pub struct SessionLock {
    file: File,
}

impl Drop for SessionLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

impl SessionLock {
    /// Records a use of the session: the lock file's modification time.
    pub(crate) fn record_use(&self, path: &Path) -> Result<(), StoreError> {
        self.file
            .set_modified(SystemTime::now())
            .map_err(io_at(path))
    }
}

impl SessionStore {
    pub(crate) fn lock_path(&self) -> PathBuf {
        self.bundle()
            .locks_dir()
            .join(format!("{}.lock", self.name()))
    }

    /// A request's lock: waits up to `wait`, then `session_busy`. A missing
    /// session is `no_session`, and nothing is written for it.
    pub fn lock(&self, wait: Duration) -> Result<SessionLock, StoreError> {
        if !self.exists() {
            return Err(self.no_session());
        }
        let lock = self.acquire(wait)?;
        // It may have been removed while we waited.
        if !self.exists() {
            return Err(self.no_session());
        }
        lock.record_use(&self.lock_path())?;
        Ok(lock)
    }

    /// The lock without the existence check or the use record: for creating,
    /// removing and pruning.
    pub(crate) fn acquire(&self, wait: Duration) -> Result<SessionLock, StoreError> {
        ensure_dir(&self.bundle().locks_dir())?;
        let path = self.lock_path();
        let file = open_lock_file(&path).map_err(io_at(&path))?;
        // WEB SPIKE CUT: wasip1 has no flock ("try_lock() not supported");
        // a page is the only writer of its store, so the file alone serves.
        if cfg!(target_family = "wasm") {
            return Ok(SessionLock { file });
        }
        let start = Instant::now();
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(SessionLock { file }),
                Err(TryLockError::WouldBlock) if start.elapsed() < wait => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(TryLockError::WouldBlock) => {
                    return Err(StoreError::Busy {
                        name: self.name().to_string(),
                        waited: start.elapsed(),
                    });
                }
                Err(TryLockError::Error(source)) => {
                    return Err(StoreError::Io { path, source });
                }
            }
        }
    }

    /// Host time of the session's last recorded use, in Unix seconds: when a
    /// request last took its lock, or the session was created. Falls back to
    /// `session.json`'s and then the directory's modification time when no use
    /// is recorded. `None` when nothing is on disk.
    pub fn last_used(&self) -> Option<u64> {
        let secs = |path: &Path| {
            fs::metadata(path)
                .and_then(|m| m.modified())
                .ok()
                .map(|t| t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()))
        };
        secs(&self.lock_path())
            .filter(|&t| t > 0)
            .or_else(|| secs(&self.meta_path()))
            .or_else(|| secs(&self.dir()))
    }
}

/// Opens the lock file, creating it if needed. A new one is dated to the Unix
/// epoch, meaning no use is recorded yet: taking the lock to prune or remove
/// must not make a session look used.
fn open_lock_file(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    match options.clone().create_new(true).open(path) {
        Ok(file) => {
            file.set_modified(UNIX_EPOCH)?;
            Ok(file)
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => options.open(path),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixture::Fixture;

    #[test]
    fn the_lock_is_exclusive_and_released_on_drop() {
        let f = Fixture::new("lock");
        let store = f.demo();
        let held = store.lock(Duration::from_millis(1)).unwrap();
        let err = store.lock(Duration::from_millis(10)).unwrap_err();
        assert!(matches!(err, StoreError::Busy { .. }), "{err:?}");
        drop(held);
        assert!(store.lock(Duration::from_millis(10)).is_ok());
    }

    #[test]
    fn locking_a_missing_session_is_no_session_and_leaves_no_trace() {
        let f = Fixture::new("lock-missing");
        let before = list_tree(f.bundle.root());
        let store = f.bundle.session("typo").unwrap();
        let err = store.lock(Duration::from_millis(10)).unwrap_err();
        assert!(matches!(err, StoreError::NoSession { .. }), "{err:?}");
        assert_eq!(list_tree(f.bundle.root()), before);
    }

    #[test]
    fn after_a_missing_session_is_locked_it_can_still_be_created() {
        let f = Fixture::new("lock-then-create");
        let store = f.bundle.session("demo").unwrap();
        let _ = store.lock(Duration::from_millis(10));
        store.create(&f.source()).unwrap();
        assert!(store.read_meta().is_ok());
    }

    fn list_tree(root: &std::path::Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path.clone());
                }
                out.push(path);
            }
        }
        out.sort();
        out
    }
}
