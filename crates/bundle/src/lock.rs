//! The session lock (design §1): the store's lock named
//! `.gbcli/locks/<name>.lock`. Every session request takes it, and taking it
//! for a request records the session's last use, which is what pruning
//! judges idleness by.

use std::time::Duration;

use crate::error::StoreError;
use crate::project_store::{Guard, Key, LockError};
use crate::store::SessionStore;

/// How long a writer waits for another command's lock before `session_busy`.
pub const LOCK_WAIT: Duration = Duration::from_secs(5);

/// Held while a command reads and writes a session; dropping it unlocks.
#[derive(Debug)]
pub struct SessionLock {
    _guard: Guard,
}

impl SessionStore {
    pub(crate) fn lock_key(&self) -> Key {
        self.bundle()
            .locks_key()
            .join(&format!("{}.lock", self.name()))
    }

    #[cfg(test)]
    pub(crate) fn lock_path(&self) -> std::path::PathBuf {
        self.files().locate(&self.lock_key())
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
        self.record_use(&lock)?;
        Ok(lock)
    }

    /// Records a use of the session, now by the bundle's clock; `_held`
    /// shows the caller holds its lock.
    pub(crate) fn record_use(&self, _held: &SessionLock) -> Result<(), StoreError> {
        let key = self.lock_key();
        self.files()
            .touch(&key, self.bundle().now())
            .map_err(self.bundle().io_at(&key))
    }

    /// The lock without the existence check or the use record: for creating,
    /// removing and pruning.
    pub(crate) fn acquire(&self, wait: Duration) -> Result<SessionLock, StoreError> {
        let locks = self.bundle().locks_key();
        self.files()
            .make_dir(&locks)
            .map_err(self.bundle().io_at(&locks))?;
        let key = self.lock_key();
        match self.files().lock(&key, wait) {
            Ok(guard) => Ok(SessionLock { _guard: guard }),
            Err(LockError::Busy { waited }) => Err(StoreError::Busy {
                name: self.name().to_string(),
                waited,
            }),
            Err(LockError::Io(source)) => Err(self.bundle().io_error(&key, source)),
        }
    }

    /// Host time of the session's last recorded use, in Unix seconds: when a
    /// request last took its lock, or the session was created. Falls back to
    /// what the store knows of `session.json`'s and then the directory's last
    /// write when no use is recorded. `None` when the store knows nothing.
    pub fn last_used(&self) -> Option<u64> {
        let files = self.files();
        files
            .last_used(&self.lock_key())
            .filter(|&t| t > 0)
            .or_else(|| files.last_used(&self.meta_key()))
            .or_else(|| files.last_used(&self.dir_key()))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

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

    /// Two processes never both hold one session's lock: two bundles (two
    /// stores, separate descriptors) on one project, in threads.
    #[test]
    fn two_stores_never_hold_one_session_lock_together() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

        let f = Fixture::new("two-lockers");
        f.demo();
        let held = Arc::new(AtomicBool::new(false));
        let overlaps = Arc::new(AtomicUsize::new(0));
        let threads: Vec<_> = (0..2)
            .map(|_| {
                let bundle = crate::Bundle::open_in(&f.root, None).unwrap();
                let held = held.clone();
                let overlaps = overlaps.clone();
                std::thread::spawn(move || {
                    let store = bundle.session("demo").unwrap();
                    for _ in 0..200 {
                        let _lock = store.lock(Duration::from_secs(5)).unwrap();
                        if held.swap(true, Ordering::SeqCst) {
                            overlaps.fetch_add(1, Ordering::SeqCst);
                        }
                        let meta = store.read_meta().unwrap();
                        store.write_meta(&meta).unwrap();
                        held.store(false, Ordering::SeqCst);
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(overlaps.load(Ordering::SeqCst), 0);
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
