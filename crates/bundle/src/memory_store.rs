//! The memory backing of [`ProjectStore`]: a map from key to blob or
//! directory, for a caller with no filesystem (a Rust tool's throwaway
//! sessions, a web page). The caller seeds it with the project's files (the
//! ROM, its `.sym`) under project keys when it builds it, and makes the
//! bundle's directory; nothing persists it.
//!
//! The project has a path, `root`, that need not exist: the server resolves
//! a request's paths against its host's working directory before the store
//! sees them, so a host whose working directory sits under `root` names
//! project keys, and any other path is a key outside the project. Paths are
//! resolved lexically (`.` and `..`); there are no aliases, so
//! [`ProjectStore::canonical_key`] is [`ProjectStore::key_of`].

mod state;

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use crate::file_store::normalize;
use crate::project_store::{
    ChangeToken, Clock, Entry, Guard, Key, Kind, LockError, ProjectStore, PutAllError,
};
use state::{
    Data, Node, State, check, is_a_dir, is_top, lock, not_a_dir, not_found, parent, under,
};

/// A project held in memory, as a [`ProjectStore`].
#[derive(Debug)]
pub struct MemoryStore {
    root: PathBuf,
    clock: Arc<dyn Clock>,
    state: Arc<Mutex<State>>,
}

/// Why a [`MemoryStore`] wasn't made: its project path is relative, so no
/// host (whose working directory is absolute) could sit under it, and every
/// request path would be a key outside the project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelativeRoot(pub PathBuf);

impl fmt::Display for RelativeRoot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the memory store's project path {:?} isn't absolute",
            self.0.display().to_string()
        )
    }
}

impl std::error::Error for RelativeRoot {}

impl MemoryStore {
    /// An empty store for the project at `root`, a path that need not exist
    /// but must be absolute, so a host can sit under it. `clock` dates the
    /// writes, for the last-use fallbacks.
    pub fn new(
        root: impl Into<PathBuf>,
        clock: Arc<dyn Clock>,
    ) -> Result<MemoryStore, RelativeRoot> {
        let root = root.into();
        if !root.is_absolute() {
            return Err(RelativeRoot(root));
        }
        let state = State::new(clock.now_secs());
        Ok(MemoryStore {
            root: normalize(&root),
            clock,
            state: Arc::new(Mutex::new(state)),
        })
    }

    fn state(&self) -> MutexGuard<'_, State> {
        lock(&self.state)
    }

    /// The key for a normalized path: relative when it is inside the
    /// project.
    fn key_from(&self, path: &Path) -> Key {
        match path.strip_prefix(&self.root) {
            Ok(inside) => Key::new(slashed(inside)),
            Err(_) => Key::new(slashed(path)),
        }
    }
}

/// A path's parts joined by `/`, a leading `/` kept.
fn slashed(path: &Path) -> String {
    let parts: Vec<String> = path
        .iter()
        .filter(|part| *part != "/")
        .map(|part| part.to_string_lossy().into_owned())
        .collect();
    let joined = parts.join("/");
    if path.has_root() {
        format!("/{joined}")
    } else {
        joined
    }
}

impl ProjectStore for MemoryStore {
    fn get(&self, key: &Key) -> io::Result<Vec<u8>> {
        self.state().blob(key).map(<[u8]>::to_vec)
    }

    fn get_range(&self, key: &Key, offset: u64, len: u64) -> io::Result<Vec<u8>> {
        let state = self.state();
        let bytes = state.blob(key)?;
        let start = usize::try_from(offset)
            .unwrap_or(usize::MAX)
            .min(bytes.len());
        let end = start
            .saturating_add(usize::try_from(len).unwrap_or(usize::MAX))
            .min(bytes.len());
        Ok(bytes[start..end].to_vec())
    }

    fn size(&self, key: &Key) -> io::Result<u64> {
        self.state().blob(key).map(|bytes| bytes.len() as u64)
    }

    /// Always settled: the version is the write count at the last write.
    fn change_token(&self, key: &Key) -> io::Result<ChangeToken> {
        check(key)?;
        match self.state().nodes.get(key) {
            Some(Node {
                data: Data::Blob { version, .. },
                ..
            }) => Ok(ChangeToken::settled([*version])),
            Some(_) => Err(is_a_dir(key)),
            None => Err(not_found(key)),
        }
    }

    fn put(&self, key: &Key, bytes: &[u8]) -> io::Result<()> {
        let now = self.clock.now_secs();
        let mut state = self.state();
        state.writable(key)?;
        state.write(key, bytes.to_vec(), now);
        Ok(())
    }

    /// Atomic as a set, too: every blob is checked before any is written.
    fn put_all(&self, blobs: &[(Key, &[u8])]) -> Result<(), PutAllError> {
        let now = self.clock.now_secs();
        let mut state = self.state();
        for (index, (key, _)) in blobs.iter().enumerate() {
            state
                .writable(key)
                .map_err(|error| PutAllError { index, error })?;
        }
        for (key, bytes) in blobs {
            state.write(key, bytes.to_vec(), now);
        }
        Ok(())
    }

    fn append(&self, key: &Key, bytes: &[u8]) -> io::Result<()> {
        let now = self.clock.now_secs();
        let mut state = self.state();
        state.writable(key)?;
        let mut whole = state.blob(key).map(<[u8]>::to_vec).unwrap_or_default();
        whole.extend_from_slice(bytes);
        state.write(key, whole, now);
        Ok(())
    }

    fn remove(&self, key: &Key) -> io::Result<()> {
        let now = self.clock.now_secs();
        let mut state = self.state();
        state.blob(key)?;
        state.nodes.remove(key);
        state.stamp_parent(key, now);
        Ok(())
    }

    fn remove_tree(&self, key: &Key) -> io::Result<()> {
        check(key)?;
        let now = self.clock.now_secs();
        let mut state = self.state();
        if !state.is_dir(key) {
            return Err(match state.nodes.get(key) {
                Some(_) => not_a_dir(key),
                None => not_found(key),
            });
        }
        for child in state.keys_under(key) {
            state.nodes.remove(&child);
        }
        if is_top(key) {
            // It always exists: emptied, and dated like any change in it.
            if let Some(top) = state.nodes.get_mut(key) {
                top.used = now;
            }
        } else {
            state.nodes.remove(key);
            state.stamp_parent(key, now);
        }
        Ok(())
    }

    fn rename(&self, from: &Key, to: &Key) -> io::Result<()> {
        check(from)?;
        check(to)?;
        let now = self.clock.now_secs();
        let mut state = self.state();
        if from == to {
            return state
                .nodes
                .get(from)
                .map(|_| ())
                .ok_or_else(|| not_found(from));
        }
        if is_top(from) || is_top(to) || under(to, from) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("can't move `{from}` to `{to}`"),
            ));
        }
        let Some(moving) = state.nodes.get(from) else {
            return Err(not_found(from));
        };
        let moving_dir = moving.is_dir();
        match parent(to) {
            Some(dir) if state.is_dir(&dir) => {}
            _ => return Err(not_found(to)),
        }
        match (moving_dir, state.nodes.get(to).map(Node::is_dir)) {
            (false, Some(true)) => return Err(is_a_dir(to)),
            (true, Some(false)) => return Err(not_a_dir(to)),
            (true, Some(true)) if !state.keys_under(to).is_empty() => {
                return Err(io::Error::new(
                    io::ErrorKind::DirectoryNotEmpty,
                    format!("`{to}` isn't empty"),
                ));
            }
            _ => {}
        }
        let mut moved = vec![(from.clone(), to.clone())];
        for child in state.keys_under(from) {
            let rest = &child.as_str()[from.as_str().len()..];
            moved.push((child.clone(), Key::new(format!("{to}{rest}"))));
        }
        for (old, new) in moved {
            if let Some(mut node) = state.nodes.remove(&old) {
                if let Data::Blob { version, .. } = &mut node.data {
                    state.writes += 1;
                    *version = state.writes;
                }
                state.nodes.insert(new, node);
            }
        }
        state.stamp_parent(from, now);
        state.stamp_parent(to, now);
        Ok(())
    }

    fn list(&self, key: &Key) -> io::Result<Vec<Entry>> {
        check(key)?;
        let state = self.state();
        if !state.is_dir(key) {
            return Err(match state.nodes.get(key) {
                Some(_) => not_a_dir(key),
                None => not_found(key),
            });
        }
        Ok(state
            .nodes
            .iter()
            .filter(|(child, _)| under(child, key) && parent(child).as_ref() == Some(key))
            .map(|(child, node)| Entry {
                name: child.name().to_string(),
                kind: if node.is_dir() { Kind::Dir } else { Kind::Blob },
            })
            .collect())
    }

    fn kind(&self, key: &Key) -> Option<Kind> {
        check(key).ok()?;
        let state = self.state();
        if state.is_dir(key) {
            return Some(Kind::Dir);
        }
        state.nodes.get(key).map(|_| Kind::Blob)
    }

    fn make_dir(&self, key: &Key) -> io::Result<()> {
        check(key)?;
        let now = self.clock.now_secs();
        self.state().insert_dir(key, now)
    }

    fn make_dirs(&self, key: &Key) -> io::Result<()> {
        check(key)?;
        let now = self.clock.now_secs();
        let mut state = self.state();
        let mut chain = vec![key.clone()];
        while let Some(up) = chain.last().and_then(parent) {
            chain.push(up);
        }
        // A blob on the way is in the path's way, as on a filesystem; one
        // at `key` itself only has its name.
        if let Some(blob) = chain[1..]
            .iter()
            .find(|dir| state.nodes.get(dir).is_some_and(|node| !node.is_dir()))
        {
            return Err(not_a_dir(blob));
        }
        for dir in chain.iter().rev() {
            state.insert_dir(dir, now)?;
        }
        Ok(())
    }

    /// Succeeds, or answers `Busy` at once: one owner drives a memory store
    /// from one thread, so no other holder could let go while it waited.
    /// Like a lock file, the lock is a blob, created with no use recorded,
    /// whose creation dates its directory.
    fn lock(&self, key: &Key, _wait: Duration) -> Result<Guard, LockError> {
        let now = self.clock.now_secs();
        let mut state = self.state();
        if !state.nodes.get(key).is_some_and(|node| !node.is_dir()) {
            state.writable(key).map_err(LockError::Io)?;
            state.write(key, Vec::new(), 0);
            state.stamp_parent(key, now);
        }
        if !state.held.insert(key.clone()) {
            return Err(LockError::Busy {
                waited: Duration::ZERO,
            });
        }
        Ok(Box::new(MemoryLock {
            state: self.state.clone(),
            key: key.clone(),
        }))
    }

    fn touch(&self, key: &Key, now: u64) -> io::Result<()> {
        check(key)?;
        let mut state = self.state();
        let node = state.nodes.get_mut(key).ok_or_else(|| not_found(key))?;
        // A use is recorded on a blob (a lock, `session.json`), as a file's
        // mtime is set by opening it.
        if node.is_dir() {
            return Err(is_a_dir(key));
        }
        node.used = now;
        Ok(())
    }

    fn last_used(&self, key: &Key) -> Option<u64> {
        check(key).ok()?;
        self.state().nodes.get(key).map(|node| node.used)
    }

    /// A counter: the store lives in one process, so it is unique there.
    /// A backing that persisted this store must persist the counter too,
    /// never restart it: a recreated session must never reuse the
    /// incarnation of the one it replaced.
    fn new_incarnation(&self) -> u64 {
        let mut state = self.state();
        state.incarnations += 1;
        state.incarnations
    }

    fn key_of(&self, path: &Path) -> Key {
        let joined = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        };
        self.key_from(&normalize(&joined))
    }

    /// [`ProjectStore::key_of`]: memory has no aliases.
    fn canonical_key(&self, path: &Path) -> Key {
        self.key_of(path)
    }

    fn locate(&self, key: &Key) -> PathBuf {
        if key.is_absolute() {
            PathBuf::from(key.as_str())
        } else if key.as_str().is_empty() {
            self.root.clone()
        } else {
            self.root.join(key.as_str())
        }
    }
}

/// A held lock; dropping it lets the key be locked again.
#[derive(Debug)]
struct MemoryLock {
    state: Arc<Mutex<State>>,
    key: Key,
}

impl Drop for MemoryLock {
    fn drop(&mut self) {
        lock(&self.state).held.remove(&self.key);
    }
}

#[cfg(test)]
mod tests;
