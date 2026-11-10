//! The store a project's files live in: keys and blobs, below every bundle
//! decision. The bundle (sessions, the history ring, checkpoints, pruning,
//! events, the gap ledger, the ROM stamp) is written against
//! [`ProjectStore`]; [`crate::FileStore`] backs it with the project
//! directory, and a backing that isn't the filesystem (memory, a browser's
//! storage, an agent's own) implements the same trait outside this crate.
//!
//! A [`Key`] is rooted at the project, not at the bundle: the bundle lives
//! under its own prefix (`.gbcli/`), and the ROM, its `.sym`, battery saves
//! and the images a request writes are keys like any other.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// A name in a [`ProjectStore`]: a `/`-separated path relative to the
/// project, or, with a leading `/`, a host path outside it (a ROM or a shot
/// named by an absolute path). The empty key is the project itself.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Key(String);

impl Key {
    /// A key from its text, as given: no normalization.
    pub fn new(text: impl Into<String>) -> Key {
        Key(text.into())
    }

    /// The project itself.
    pub fn root() -> Key {
        Key(String::new())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The key `part` under this one; under the root, `part` itself.
    pub fn join(&self, part: &str) -> Key {
        if self.0.is_empty() {
            Key(part.to_string())
        } else if self.0.ends_with('/') {
            Key(format!("{}{part}", self.0))
        } else {
            Key(format!("{}/{part}", self.0))
        }
    }

    /// A host path outside the project.
    pub fn is_absolute(&self) -> bool {
        self.0.starts_with('/')
    }

    /// The last `/`-separated part.
    pub fn name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or_default()
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a key holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Blob,
    /// A prefix other keys sit under. A backing whose keys are flat still
    /// answers it for a prefix it was asked to make (`make_dir`), so a write
    /// under a removed session fails rather than recreating it.
    Dir,
}

/// One child of a listed key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The child's last part, not its whole key.
    pub name: String,
    pub kind: Kind,
}

/// Why a lock wasn't taken.
#[derive(Debug)]
pub enum LockError {
    /// Another holder kept it past the wait.
    Busy {
        waited: Duration,
    },
    Io(io::Error),
}

/// Why [`ProjectStore::put_all`] failed: which blob, and the error.
#[derive(Debug)]
pub struct PutAllError {
    pub index: usize,
    pub error: io::Error,
}

/// Held while a session is read and written; dropping it unlocks.
pub type Guard = Box<dyn fmt::Debug + Send>;

/// Host time, in Unix seconds, for the decisions that need it (last use,
/// pruning, the logs' `at`). Emulation never reads it (design §10). The
/// filesystem's caller passes [`crate::SystemClock`]; a memory caller passes
/// its own.
pub trait Clock: Send + Sync + fmt::Debug {
    fn now_secs(&self) -> u64;
}

/// Keys and blobs, with directories, a lock per session and a record of
/// last use: everything the bundle needs from storage, and no decision.
///
/// Errors are `io::Error`s; a missing key is one of kind `NotFound`, so a
/// caller tells "not there" from "couldn't read" as it would on a file. The
/// writes never create a key's parent: a write under a prefix another
/// process just removed fails instead of resurrecting it.
pub trait ProjectStore: Send + Sync + fmt::Debug {
    /// The blob at `key`.
    fn get(&self, key: &Key) -> io::Result<Vec<u8>>;

    /// Up to `len` bytes of the blob at `key`, from `offset`.
    fn get_range(&self, key: &Key, offset: u64, len: u64) -> io::Result<Vec<u8>>;

    /// The blob's length in bytes.
    fn size(&self, key: &Key) -> io::Result<u64>;

    /// Replaces the blob at `key`, atomically: a reader sees the old blob or
    /// the new one, never part of either. The crash-consistency of heads,
    /// metadata and checkpoints rests on this.
    fn put(&self, key: &Key, bytes: &[u8]) -> io::Result<()>;

    /// Replaces several blobs, each atomically: every one is written before
    /// any replaces its key, so a reported failure leaves them all as they
    /// were (short of one replace refused after another succeeded). Not
    /// atomic as a set against a crash between the replaces.
    fn put_all(&self, blobs: &[(Key, &[u8])]) -> Result<(), PutAllError>;

    /// Appends to the blob at `key`, creating the blob but not its parent.
    fn append(&self, key: &Key, bytes: &[u8]) -> io::Result<()>;

    /// Removes the blob at `key`.
    fn remove(&self, key: &Key) -> io::Result<()>;

    /// Removes `key` and everything under it.
    fn remove_tree(&self, key: &Key) -> io::Result<()>;

    /// Moves `from`, and everything under it, to `to` in one step,
    /// replacing a blob at `to`.
    fn rename(&self, from: &Key, to: &Key) -> io::Result<()>;

    /// The keys directly under `key`, in no particular order; `NotFound`
    /// when `key` isn't a directory that exists.
    fn list(&self, key: &Key) -> io::Result<Vec<Entry>>;

    /// What `key` holds, if anything.
    fn kind(&self, key: &Key) -> Option<Kind>;

    /// Makes `key` a directory; an existing one is fine. Its parent must
    /// exist, for the reason the trait gives.
    fn make_dir(&self, key: &Key) -> io::Result<()>;

    /// Makes `key` a directory, and its parents.
    fn make_dirs(&self, key: &Key) -> io::Result<()>;

    /// Takes the lock named `key`, waiting up to `wait` for another holder.
    /// The lock's parent must exist. Taking it records no use.
    fn lock(&self, key: &Key, wait: Duration) -> Result<Guard, LockError>;

    /// Records a use of `key` at `now` (Unix seconds).
    fn touch(&self, key: &Key, now: u64) -> io::Result<()>;

    /// When `key` was last used or written, in Unix seconds, if the backing
    /// knows. Zero means a lock that no use has been recorded on.
    fn last_used(&self, key: &Key) -> Option<u64>;

    /// A value no earlier call returned, here or in another process sharing
    /// the store: what tells a recreated session from the one it replaced.
    fn new_incarnation(&self) -> u64;

    /// The key for `path`: relative to the project when it is relative, or
    /// when it names a place inside the project; else the path itself.
    fn key_of(&self, path: &Path) -> Key;

    /// Where `key` lives, for messages and for tools that need a host path.
    fn locate(&self, key: &Key) -> PathBuf;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joining_under_the_root_is_the_part_itself() {
        assert_eq!(Key::root().join(".gbcli"), Key::new(".gbcli"));
        assert_eq!(
            Key::new(".gbcli").join("sessions").join("demo"),
            Key::new(".gbcli/sessions/demo")
        );
        assert_eq!(Key::new("/").join("x.gb"), Key::new("/x.gb"));
    }

    #[test]
    fn a_key_knows_its_name_and_whether_it_is_outside_the_project() {
        let key = Key::new(".gbcli/sessions/demo/session.json");
        assert_eq!(key.name(), "session.json");
        assert!(!key.is_absolute());
        assert!(Key::new("/tmp/game.gb").is_absolute());
    }
}
