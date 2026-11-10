//! The memory store's map, and the rules its keys follow: which are
//! normal, which directory each sits in, and what a write may replace.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::sync::{Mutex, MutexGuard};

use crate::project_store::Key;

#[derive(Debug, Default)]
pub(super) struct State {
    pub(super) nodes: BTreeMap<Key, Node>,
    /// The locks held now.
    pub(super) held: BTreeSet<Key>,
    /// Writes so far: a blob's version is the count at its last write, so
    /// its change token moves with every write and never comes back.
    pub(super) writes: u64,
    pub(super) incarnations: u64,
}

#[derive(Debug)]
pub(super) struct Node {
    pub(super) data: Data,
    /// Unix seconds of the last write or recorded use; a directory's moves
    /// when a child is added or removed, as a directory's mtime does.
    pub(super) used: u64,
}

#[derive(Debug)]
pub(super) enum Data {
    Blob { bytes: Vec<u8>, version: u64 },
    Dir,
}

impl Node {
    pub(super) fn is_dir(&self) -> bool {
        matches!(self.data, Data::Dir)
    }
}

/// A poisoned lock only means a panic elsewhere; every change is checked
/// before it is made, so the map is never half changed.
pub(super) fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(|e| e.into_inner())
}

/// Refuses a key that isn't normal: `Key::new` keeps `a/../b` as given,
/// and in a map it would be another blob than `b`.
pub(super) fn check(key: &Key) -> io::Result<()> {
    let text = key.as_str();
    let body = text.strip_prefix('/').unwrap_or(text);
    if body.is_empty()
        || body
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
    {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("`{text}` isn't a normal key: no `.`, `..` or empty parts"),
        ))
    }
}

/// The project and `/` are directories that always exist.
pub(super) fn is_top(key: &Key) -> bool {
    matches!(key.as_str(), "" | "/")
}

pub(super) fn parent(key: &Key) -> Option<Key> {
    let text = key.as_str();
    if is_top(key) {
        return None;
    }
    Some(match text.rfind('/') {
        Some(0) => Key::new("/"),
        Some(at) => Key::new(&text[..at]),
        None => Key::root(),
    })
}

/// Whether `key` is strictly under `dir`.
pub(super) fn under(key: &Key, dir: &Key) -> bool {
    let (key, dir) = (key.as_str(), dir.as_str());
    match dir {
        "" => !key.is_empty() && !key.starts_with('/'),
        "/" => key.len() > 1 && key.starts_with('/'),
        _ => key.len() > dir.len() && key.starts_with(dir) && key.as_bytes()[dir.len()] == b'/',
    }
}

pub(super) fn not_found(key: &Key) -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, format!("no `{key}` in memory"))
}

pub(super) fn is_a_dir(key: &Key) -> io::Error {
    io::Error::new(
        io::ErrorKind::IsADirectory,
        format!("`{key}` is a directory"),
    )
}

pub(super) fn not_a_dir(key: &Key) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotADirectory,
        format!("`{key}` isn't a directory"),
    )
}

impl State {
    /// An empty store whose two top directories, the project and `/`, were
    /// made at `now`: they always exist, and their last use moves like any
    /// directory's.
    pub(super) fn new(now: u64) -> State {
        let mut state = State::default();
        for top in [Key::root(), Key::new("/")] {
            let node = Node {
                data: Data::Dir,
                used: now,
            };
            state.nodes.insert(top, node);
        }
        state
    }

    pub(super) fn is_dir(&self, key: &Key) -> bool {
        is_top(key) || self.nodes.get(key).is_some_and(Node::is_dir)
    }

    pub(super) fn blob(&self, key: &Key) -> io::Result<&[u8]> {
        check(key)?;
        match self.nodes.get(key) {
            Some(Node {
                data: Data::Blob { bytes, .. },
                ..
            }) => Ok(bytes),
            Some(_) => Err(is_a_dir(key)),
            None if is_top(key) => Err(is_a_dir(key)),
            None => Err(not_found(key)),
        }
    }

    /// Why `key` can't take a blob now, if it can't: it isn't normal, its
    /// parent isn't a directory, or it is one.
    pub(super) fn writable(&self, key: &Key) -> io::Result<()> {
        check(key)?;
        if self.is_dir(key) {
            return Err(is_a_dir(key));
        }
        match parent(key) {
            Some(dir) if self.is_dir(&dir) => Ok(()),
            _ => Err(not_found(key)),
        }
    }

    /// Records a change under `key`'s directory at `now`.
    pub(super) fn stamp_parent(&mut self, key: &Key, now: u64) {
        if let Some(node) = parent(key).and_then(|dir| self.nodes.get_mut(&dir)) {
            node.used = now;
        }
    }

    pub(super) fn write(&mut self, key: &Key, bytes: Vec<u8>, now: u64) {
        self.writes += 1;
        let data = Data::Blob {
            bytes,
            version: self.writes,
        };
        self.nodes.insert(key.clone(), Node { data, used: now });
        self.stamp_parent(key, now);
    }

    pub(super) fn insert_dir(&mut self, key: &Key, now: u64) -> io::Result<()> {
        if self.is_dir(key) {
            return Ok(());
        }
        if self.nodes.contains_key(key) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("a blob is at `{key}`"),
            ));
        }
        match parent(key) {
            Some(dir) if self.is_dir(&dir) => {}
            _ => return Err(not_found(key)),
        }
        let node = Node {
            data: Data::Dir,
            used: now,
        };
        self.nodes.insert(key.clone(), node);
        self.stamp_parent(key, now);
        Ok(())
    }

    pub(super) fn keys_under(&self, dir: &Key) -> Vec<Key> {
        self.nodes
            .keys()
            .filter(|key| under(key, dir))
            .cloned()
            .collect()
    }
}
