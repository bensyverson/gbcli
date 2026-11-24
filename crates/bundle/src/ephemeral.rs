//! A bundle whose sessions live in memory beside a project that is still on
//! disk, for a harness that drives one `gbcli serve` per test and has no use
//! for what a session leaves behind: its history checkpoints, `session.json`,
//! event log and lock file. The bundle's keys (`.gbcli/…`) go to a
//! [`MemoryStore`]; every other key (the ROM, its `.sym`, a shot, a build's
//! files) goes to the project's [`FileStore`], so requests that read and write
//! the host's files behave as they do on a disk bundle.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::bundle::Bundle;
use crate::file_store::{FileStore, SystemClock, absolute};
use crate::memory_store::MemoryStore;
use crate::project_store::{
    ChangeToken, Clock, Entry, Guard, Key, Kind, LockError, ProjectStore, PutAllError,
};

/// The bundle directory's name under the project.
const BUNDLE_DIR: &str = ".gbcli";

impl Bundle {
    /// A bundle for the project at `project` (made absolute; it need not
    /// have a `.gbcli/`, and none is made) whose sessions, history,
    /// checkpoints, events, locks and gap ledger are kept in memory and
    /// gone when the process ends. Files outside the bundle directory are
    /// the project's own. `GBCLI_DIR` and an existing `.gbcli/` are not
    /// looked at, so no other process's sessions are seen.
    pub fn ephemeral(project: &Path) -> Bundle {
        let project = absolute(project);
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let dir = Key::new(BUNDLE_DIR);
        let memory = MemoryStore::new(project.clone(), clock.clone())
            .expect("an absolute project path is a valid memory root");
        memory
            .make_dir(&dir)
            .expect("the memory store's top directory exists");
        let store = SplitStore {
            files: FileStore::new(project),
            memory,
            memory_prefix: dir.clone(),
        };
        Bundle::new(Arc::new(store), dir, clock)
    }
}

/// Keys under `memory_prefix` in `memory`, every other key in `files`.
#[derive(Debug)]
struct SplitStore {
    files: FileStore,
    memory: MemoryStore,
    memory_prefix: Key,
}

impl SplitStore {
    fn in_memory(&self, key: &Key) -> bool {
        let (key, prefix) = (key.as_str(), self.memory_prefix.as_str());
        key == prefix
            || key
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with('/'))
    }

    fn of(&self, key: &Key) -> &dyn ProjectStore {
        if self.in_memory(key) {
            &self.memory
        } else {
            &self.files
        }
    }
}

impl ProjectStore for SplitStore {
    fn get(&self, key: &Key) -> io::Result<Vec<u8>> {
        self.of(key).get(key)
    }

    fn get_range(&self, key: &Key, offset: u64, len: u64) -> io::Result<Vec<u8>> {
        self.of(key).get_range(key, offset, len)
    }

    fn size(&self, key: &Key) -> io::Result<u64> {
        self.of(key).size(key)
    }

    fn change_token(&self, key: &Key) -> io::Result<ChangeToken> {
        self.of(key).change_token(key)
    }

    fn put(&self, key: &Key, bytes: &[u8]) -> io::Result<()> {
        self.of(key).put(key, bytes)
    }

    fn put_all(&self, blobs: &[(Key, &[u8])]) -> Result<(), PutAllError> {
        let memory = blobs.iter().filter(|(k, _)| self.in_memory(k)).count();
        if memory == 0 {
            return self.files.put_all(blobs);
        }
        if memory == blobs.len() {
            return self.memory.put_all(blobs);
        }
        // A set spanning both is no more atomic than the contract promises
        // across a crash: each blob is replaced atomically, one by one.
        for (index, (key, bytes)) in blobs.iter().enumerate() {
            self.put(key, bytes)
                .map_err(|error| PutAllError { index, error })?;
        }
        Ok(())
    }

    fn append(&self, key: &Key, bytes: &[u8]) -> io::Result<()> {
        self.of(key).append(key, bytes)
    }

    fn remove(&self, key: &Key) -> io::Result<()> {
        self.of(key).remove(key)
    }

    fn remove_tree(&self, key: &Key) -> io::Result<()> {
        self.of(key).remove_tree(key)
    }

    fn rename(&self, from: &Key, to: &Key) -> io::Result<()> {
        if self.in_memory(from) != self.in_memory(to) {
            return Err(io::Error::new(
                io::ErrorKind::CrossesDevices,
                format!("can't move `{from}` to `{to}`: one is in memory, one is on disk"),
            ));
        }
        self.of(from).rename(from, to)
    }

    fn list(&self, key: &Key) -> io::Result<Vec<Entry>> {
        self.of(key).list(key)
    }

    fn kind(&self, key: &Key) -> Option<Kind> {
        self.of(key).kind(key)
    }

    fn make_dir(&self, key: &Key) -> io::Result<()> {
        self.of(key).make_dir(key)
    }

    fn make_dirs(&self, key: &Key) -> io::Result<()> {
        self.of(key).make_dirs(key)
    }

    fn lock(&self, key: &Key, wait: Duration) -> Result<Guard, LockError> {
        self.of(key).lock(key, wait)
    }

    fn touch(&self, key: &Key, now: u64) -> io::Result<()> {
        self.of(key).touch(key, now)
    }

    fn last_used(&self, key: &Key) -> Option<u64> {
        self.of(key).last_used(key)
    }

    fn new_incarnation(&self) -> u64 {
        self.memory.new_incarnation()
    }

    fn key_of(&self, path: &Path) -> Key {
        self.files.key_of(path)
    }

    fn canonical_key(&self, path: &Path) -> Key {
        self.files.canonical_key(path)
    }

    /// Where the key would be on disk, for messages: a bundle key names a
    /// place that holds nothing.
    fn locate(&self, key: &Key) -> PathBuf {
        self.files.locate(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Scratch {
            let dir =
                std::env::temp_dir().join(format!("gbcli-ephemeral-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Scratch(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn split(dir: &Path) -> SplitStore {
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let memory = MemoryStore::new(dir, clock).unwrap();
        memory.make_dir(&Key::new(".gbcli")).unwrap();
        SplitStore {
            files: FileStore::new(dir.to_path_buf()),
            memory,
            memory_prefix: Key::new(".gbcli"),
        }
    }

    #[test]
    fn bundle_keys_stay_in_memory_and_the_rest_reaches_the_disk() {
        let s = Scratch::new("routes");
        let store = split(&s.0);
        store.put(&Key::new("rom.gb"), b"rom").unwrap();
        store.make_dir(&Key::new(".gbcli/sessions")).unwrap();
        store
            .put(&Key::new(".gbcli/sessions/x.json"), b"meta")
            .unwrap();
        assert_eq!(std::fs::read(s.0.join("rom.gb")).unwrap(), b"rom");
        assert!(!s.0.join(".gbcli").exists(), "the bundle reached the disk");
        assert_eq!(
            store.get(&Key::new(".gbcli/sessions/x.json")).unwrap(),
            b"meta"
        );
        assert_eq!(store.get(&Key::new("rom.gb")).unwrap(), b"rom");
    }

    #[test]
    fn a_key_that_only_starts_with_the_prefix_is_the_projects() {
        let s = Scratch::new("prefix");
        let store = split(&s.0);
        store.put(&Key::new(".gbcli-notes"), b"mine").unwrap();
        assert_eq!(std::fs::read(s.0.join(".gbcli-notes")).unwrap(), b"mine");
    }

    #[test]
    fn a_set_spanning_both_is_written_to_both() {
        let s = Scratch::new("put-all");
        let store = split(&s.0);
        let blobs: [(Key, &[u8]); 2] = [
            (Key::new("out.png"), b"png"),
            (Key::new(".gbcli/a"), b"mem"),
        ];
        store.put_all(&blobs).unwrap();
        assert_eq!(std::fs::read(s.0.join("out.png")).unwrap(), b"png");
        assert_eq!(store.get(&Key::new(".gbcli/a")).unwrap(), b"mem");
        assert!(!s.0.join(".gbcli").exists());
        let bad: [(Key, &[u8]); 2] = [(Key::new(".gbcli/b"), b"x"), (Key::new("no-dir/c"), b"y")];
        let failed = store.put_all(&bad).unwrap_err();
        assert_eq!(failed.index, 1, "the failing blob is named");
    }

    #[test]
    fn a_move_between_memory_and_disk_is_refused() {
        let s = Scratch::new("rename");
        let store = split(&s.0);
        store.put(&Key::new("rom.gb"), b"rom").unwrap();
        let moved = store.rename(&Key::new("rom.gb"), &Key::new(".gbcli/rom.gb"));
        assert_eq!(moved.unwrap_err().kind(), io::ErrorKind::CrossesDevices);
        assert!(s.0.join("rom.gb").exists(), "the source is untouched");
    }

    #[test]
    fn an_ephemeral_bundle_makes_a_session_without_a_bundle_directory() {
        let s = Scratch::new("bundle");
        std::fs::write(s.0.join("demo.gb"), crate::test_fixture::ROM).unwrap();
        let bundle = Bundle::ephemeral(&s.0);
        let source = crate::RomSource {
            rom: s.0.join("demo.gb"),
            sym: None,
            model: None,
            seed: None,
            flash: None,
        };
        let opened = bundle.start_session("x", Some(&source)).unwrap();
        assert!(opened.created.is_some());
        assert_eq!(bundle.session_names().unwrap(), vec!["x".to_string()]);
        let again = Bundle::ephemeral(&s.0);
        assert!(again.session_names().unwrap().is_empty());
        assert!(!s.0.join(".gbcli").exists());
    }
}
