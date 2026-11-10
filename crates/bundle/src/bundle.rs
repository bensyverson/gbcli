//! What a project's bundle holds and where (design §1): sessions, the boot
//! cache, and the gap ledger, as keys in the project's [`ProjectStore`]
//! under the bundle's own prefix (`.gbcli/`). Opening a bundle on disk is
//! the file backing's (`file_store/open.rs`).

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gbcli_protocol::{Rev, SessionRow};

use crate::error::StoreError;
use crate::project_store::{Clock, Key, Kind, ProjectStore, PutAllError};
use crate::store::SessionStore;

/// The project's bundle: its store, the prefix the bundle lives under, and
/// the clock its decisions read the time from.
#[derive(Clone, Debug)]
pub struct Bundle {
    store: Arc<dyn ProjectStore>,
    dir: Key,
    clock: Arc<dyn Clock>,
    /// Where the store puts the bundle and the project, for messages.
    root: PathBuf,
    project: PathBuf,
}

impl Bundle {
    /// A bundle under `dir` in `store` (`.gbcli` for a project's own),
    /// reading host time from `clock`.
    pub fn new(store: Arc<dyn ProjectStore>, dir: Key, clock: Arc<dyn Clock>) -> Bundle {
        let root = store.locate(&dir);
        let project = store.locate(&Key::root());
        Bundle {
            store,
            dir,
            clock,
            root,
            project,
        }
    }

    pub(crate) fn store(&self) -> &Arc<dyn ProjectStore> {
        &self.store
    }

    /// Host time in Unix seconds, from the bundle's clock. Only logs,
    /// listing and pruning read it; emulation never does (design §10).
    pub fn now(&self) -> u64 {
        self.clock.now_secs()
    }

    /// The bundle directory itself, where the store puts it.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The project directory, where the store puts it. Stored ROM and `.sym`
    /// paths are relative to it, and so are relative paths in a
    /// [`crate::RomSource`].
    pub fn project(&self) -> &Path {
        &self.project
    }

    /// A key under the bundle.
    pub(crate) fn key(&self, part: &str) -> Key {
        self.dir.join(part)
    }

    pub(crate) fn sessions_key(&self) -> Key {
        self.key("sessions")
    }

    pub fn sessions_dir(&self) -> PathBuf {
        self.store.locate(&self.sessions_key())
    }

    /// Session locks live here, outside the session directories, so
    /// removing a session never unlinks a lock another process holds or
    /// waits on. They are never deleted.
    pub(crate) fn locks_key(&self) -> Key {
        self.key("locks")
    }

    /// A session store by name; the session need not exist yet.
    pub fn session(&self, name: &str) -> Result<SessionStore, StoreError> {
        SessionStore::new(self.clone(), name)
    }

    /// Where a removal moves a session directory before deleting it, so a
    /// removal killed midway never leaves part of a session under its name.
    pub(crate) fn removed_key(&self) -> Key {
        self.key("removed")
    }

    #[cfg(test)]
    pub(crate) fn removed_dir(&self) -> PathBuf {
        self.store.locate(&self.removed_key())
    }

    /// Deletes what removals left in [`Bundle::removed_key`], best effort.
    pub(crate) fn sweep_removed(&self) {
        let _ = self.store.remove_tree(&self.removed_key());
    }

    /// The sessions: directories with a valid name and a `session.json`,
    /// sorted. Anything else under `sessions/` is not a session.
    pub fn session_names(&self) -> Result<Vec<String>, StoreError> {
        Ok(self
            .session_dirs()?
            .into_iter()
            .filter(|name| self.session(name).is_ok_and(|s| s.exists()))
            .collect())
    }

    /// Every directory under `sessions/`, sorted, session or not.
    pub(crate) fn session_dirs(&self) -> Result<Vec<String>, StoreError> {
        let key = self.sessions_key();
        let entries = match self.store.list(&key) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => return Err(self.io_error(&key, source)),
        };
        let mut names: Vec<String> = entries
            .into_iter()
            .filter(|e| e.kind == Kind::Dir)
            .map(|e| e.name)
            .collect();
        names.sort();
        Ok(names)
    }

    /// One row per session, as `gbcli sessions` prints; directories that
    /// aren't sessions are left out.
    pub fn list(&self) -> Result<Vec<SessionRow>, StoreError> {
        let mut rows = Vec::new();
        for name in self.session_names()? {
            let Ok(store) = self.session(&name) else {
                continue;
            };
            let Ok(meta) = store.read_meta() else {
                continue;
            };
            rows.push(store.row(&meta)?);
        }
        Ok(rows)
    }

    pub(crate) fn gaps_key(&self) -> Key {
        self.key("gaps.jsonl")
    }

    pub fn gaps_path(&self) -> PathBuf {
        self.store.locate(&self.gaps_key())
    }

    pub(crate) fn cache_key(&self) -> Key {
        self.key("cache")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.store.locate(&self.cache_key())
    }

    /// An `io::Error` about `key`, naming where the store puts it.
    pub(crate) fn io_error(&self, key: &Key, source: io::Error) -> StoreError {
        StoreError::Io {
            path: self.store.locate(key),
            source,
        }
    }

    /// A closure for `map_err` that wraps an `io::Error` about `key`.
    pub(crate) fn io_at<'a>(&'a self, key: &'a Key) -> impl FnOnce(io::Error) -> StoreError + 'a {
        move |source| self.io_error(key, source)
    }

    /// Bytes under `key`, skipping `exclude`.
    pub(crate) fn tree_size(&self, key: &Key, exclude: &Key) -> u64 {
        let Ok(entries) = self.store.list(key) else {
            return 0;
        };
        let mut total = 0;
        for entry in entries {
            let child = key.join(&entry.name);
            if &child == exclude {
                continue;
            }
            total += match entry.kind {
                Kind::Dir => self.tree_size(&child, exclude),
                Kind::Blob => self.store.size(&child).unwrap_or(0),
            };
        }
        total
    }
}

/// A project file a request names (a ROM, a `.sym`, a shot, a `.sav`): a
/// path relative to the project or absolute, read and written through the
/// store, so a backing that isn't the filesystem holds them too.
impl Bundle {
    /// The store's key for a project file.
    pub fn file_key(&self, path: &Path) -> Key {
        self.store.key_of(path)
    }

    pub fn read_file(&self, path: &Path) -> io::Result<Vec<u8>> {
        self.store.get(&self.file_key(path))
    }

    /// Replaces a file atomically: a reader never sees half of it.
    pub fn write_file(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        self.store.put(&self.file_key(path), bytes)
    }

    /// Replaces several files, each atomically, as
    /// [`ProjectStore::put_all`] does.
    pub fn write_files(&self, files: &[(&Path, &[u8])]) -> Result<(), PutAllError> {
        let blobs: Vec<(Key, &[u8])> = files
            .iter()
            .map(|(path, bytes)| (self.file_key(path), *bytes))
            .collect();
        self.store.put_all(&blobs)
    }

    pub fn is_dir(&self, path: &Path) -> bool {
        self.store.kind(&self.file_key(path)) == Some(Kind::Dir)
    }

    pub fn is_file(&self, path: &Path) -> bool {
        self.store.kind(&self.file_key(path)) == Some(Kind::Blob)
    }

    pub fn exists(&self, path: &Path) -> bool {
        self.store.kind(&self.file_key(path)).is_some()
    }
}

/// A session name: `[a-z0-9-]{1,40}`, not starting with `-`, which a CLI would read as a flag.
pub fn validate_name(name: &str) -> Result<(), StoreError> {
    if gbcli_protocol::is_session_name(name) {
        Ok(())
    } else {
        Err(StoreError::BadSessionName {
            name: name.to_string(),
        })
    }
}

/// The revision a session would have after its next head write.
pub fn next(rev: Rev) -> Rev {
    Rev(rev.0 + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_lowercase_digits_and_dashes() {
        for name in ["dx", "demo-1", "a", "a-", &"x".repeat(40)] {
            assert!(validate_name(name).is_ok(), "{name}");
        }
        for name in ["", "DX", "demo_1", "a b", &"x".repeat(41)] {
            assert!(validate_name(name).is_err(), "{name}");
        }
    }

    #[test]
    fn a_name_cannot_start_with_a_dash_a_cli_would_read_as_a_flag() {
        for name in ["-", "-x", "--rom"] {
            assert!(validate_name(name).is_err(), "{name}");
        }
    }
}
