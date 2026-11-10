//! A temporary project with the committed V1 ROM and its `.sym`, for the
//! store's tests. It ignores `GBCLI_DIR`, so a developer's environment can't
//! redirect it.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use gbcli_core::Model;

use crate::bundle::Bundle;
use crate::file_store::{FileStore, SystemClock};
use crate::project_store::{
    ChangeToken, Entry, Guard, Key, Kind, LockError, ProjectStore, PutAllError,
};
use crate::session::RomSource;
use crate::store::SessionStore;

pub(crate) const ROM: &[u8] = include_bytes!("../../engine/tests/roms/rebuild_v1.gb");
pub(crate) const SYM: &str = include_str!("../../engine/tests/roms/rebuild_v1.sym");

pub(crate) struct Fixture {
    pub(crate) root: PathBuf,
    pub(crate) bundle: Bundle,
}

impl Fixture {
    pub(crate) fn new(name: &str) -> Fixture {
        let root =
            std::env::temp_dir().join(format!("gbcli-store-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("demo.gb"), ROM).unwrap();
        let bundle = Bundle::open_or_create_in(&root, None).unwrap();
        Fixture { root, bundle }
    }

    pub(crate) fn source(&self) -> RomSource {
        RomSource {
            rom: self.root.join("demo.gb"),
            sym: None,
            model: Some(Model::DmgB),
            seed: Some(0),
            flash: None,
        }
    }

    /// The `demo` session, created.
    pub(crate) fn demo(&self) -> SessionStore {
        let store = self.bundle.session("demo").unwrap();
        store.create(&self.source()).unwrap();
        store
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// A [`FileStore`] that counts the blob reads (`get`, `get_range`) of each
/// key, to show what a check costs.
#[derive(Debug)]
pub(crate) struct CountingStore {
    inner: FileStore,
    reads: Mutex<HashMap<Key, usize>>,
}

impl CountingStore {
    /// A bundle at `root`'s `.gbcli`, which must exist, over a counting store.
    pub(crate) fn bundle(root: &Path) -> (Bundle, Arc<CountingStore>) {
        let store = Arc::new(CountingStore {
            inner: FileStore::new(root.to_path_buf()),
            reads: Mutex::new(HashMap::new()),
        });
        let bundle = Bundle::new(store.clone(), Key::new(".gbcli"), Arc::new(SystemClock));
        (bundle, store)
    }

    /// Blob reads of `key` since the last call, and resets them.
    pub(crate) fn take_reads(&self, key: &str) -> usize {
        self.reads
            .lock()
            .unwrap()
            .remove(&Key::new(key))
            .unwrap_or(0)
    }

    fn count(&self, key: &Key) {
        *self.reads.lock().unwrap().entry(key.clone()).or_default() += 1;
    }
}

impl ProjectStore for CountingStore {
    fn get(&self, key: &Key) -> io::Result<Vec<u8>> {
        self.count(key);
        self.inner.get(key)
    }
    fn get_range(&self, key: &Key, offset: u64, len: u64) -> io::Result<Vec<u8>> {
        self.count(key);
        self.inner.get_range(key, offset, len)
    }
    fn size(&self, key: &Key) -> io::Result<u64> {
        self.inner.size(key)
    }
    fn change_token(&self, key: &Key) -> io::Result<ChangeToken> {
        self.inner.change_token(key)
    }
    fn put(&self, key: &Key, bytes: &[u8]) -> io::Result<()> {
        self.inner.put(key, bytes)
    }
    fn put_all(&self, blobs: &[(Key, &[u8])]) -> Result<(), PutAllError> {
        self.inner.put_all(blobs)
    }
    fn append(&self, key: &Key, bytes: &[u8]) -> io::Result<()> {
        self.inner.append(key, bytes)
    }
    fn remove(&self, key: &Key) -> io::Result<()> {
        self.inner.remove(key)
    }
    fn remove_tree(&self, key: &Key) -> io::Result<()> {
        self.inner.remove_tree(key)
    }
    fn rename(&self, from: &Key, to: &Key) -> io::Result<()> {
        self.inner.rename(from, to)
    }
    fn list(&self, key: &Key) -> io::Result<Vec<Entry>> {
        self.inner.list(key)
    }
    fn kind(&self, key: &Key) -> Option<Kind> {
        self.inner.kind(key)
    }
    fn make_dir(&self, key: &Key) -> io::Result<()> {
        self.inner.make_dir(key)
    }
    fn make_dirs(&self, key: &Key) -> io::Result<()> {
        self.inner.make_dirs(key)
    }
    fn lock(&self, key: &Key, wait: Duration) -> Result<Guard, LockError> {
        self.inner.lock(key, wait)
    }
    fn touch(&self, key: &Key, now: u64) -> io::Result<()> {
        self.inner.touch(key, now)
    }
    fn last_used(&self, key: &Key) -> Option<u64> {
        self.inner.last_used(key)
    }
    fn new_incarnation(&self) -> u64 {
        self.inner.new_incarnation()
    }
    fn key_of(&self, path: &Path) -> Key {
        self.inner.key_of(path)
    }
    fn canonical_key(&self, path: &Path) -> Key {
        self.inner.canonical_key(path)
    }
    fn locate(&self, key: &Key) -> PathBuf {
        self.inner.locate(key)
    }
}

/// Backdates a file's or a directory's modification time.
pub(crate) fn backdate(path: &std::path::Path, by: Duration) {
    fs::File::open(path)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now() - by))
        .unwrap();
}

pub(crate) const DAY: Duration = Duration::from_secs(24 * 60 * 60);
