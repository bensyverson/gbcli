//! One session on disk (design §1): where its files live, its metadata,
//! listing and removal. Heads and history are in `history.rs`, named
//! checkpoints in `checkpoints.rs`, the event log in `events.rs`, the lock in `lock.rs`, creation in
//! `session.rs` and start states in `boot.rs`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use gbcli_protocol::{SessionFile, SessionRow, TimeBase};

use crate::bundle::{Bundle, now_secs, validate_name};
use crate::error::StoreError;
use crate::events::EVENTS_ROTATE_AT;
use crate::io::{atomic_write, dir_size, io_at};
use crate::session::new_incarnation;

/// A session directory. `name` is validated when the store is made.
#[derive(Clone, Debug)]
pub struct SessionStore {
    bundle: Bundle,
    name: String,
    /// Where `events.jsonl` rotates; [`EVENTS_ROTATE_AT`] unless set.
    events_rotate_at: u64,
}

impl SessionStore {
    pub fn new(bundle: Bundle, name: &str) -> Result<SessionStore, StoreError> {
        validate_name(name)?;
        Ok(SessionStore {
            bundle,
            name: name.to_string(),
            events_rotate_at: EVENTS_ROTATE_AT,
        })
    }

    /// Sets the size past which the event log rotates (design §10).
    pub fn set_events_rotate_at(&mut self, bytes: u64) {
        self.events_rotate_at = bytes;
    }

    pub(crate) fn events_rotate_at(&self) -> u64 {
        self.events_rotate_at
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn bundle(&self) -> &Bundle {
        &self.bundle
    }

    pub fn dir(&self) -> PathBuf {
        self.bundle.sessions_dir().join(&self.name)
    }

    /// Whether the session exists: its `session.json` is there. A directory
    /// without one is not a session.
    pub fn exists(&self) -> bool {
        self.meta_path().is_file()
    }

    pub(crate) fn meta_path(&self) -> PathBuf {
        self.dir().join("session.json")
    }

    pub(crate) fn history_dir(&self) -> PathBuf {
        self.dir().join("history")
    }

    pub(crate) fn checkpoints_dir(&self) -> PathBuf {
        self.dir().join("checkpoints")
    }

    pub(crate) fn no_session(&self) -> StoreError {
        StoreError::NoSession {
            name: self.name.clone(),
            known: self.bundle.session_names().unwrap_or_default(),
        }
    }

    /// The session's metadata; `no_session` when it is missing.
    pub fn read_meta(&self) -> Result<SessionFile, StoreError> {
        let path = self.meta_path();
        let text = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(self.no_session());
            }
            Err(source) => return Err(StoreError::Io { path, source }),
        };
        serde_json::from_slice(&text).map_err(|e| StoreError::SessionUnreadable {
            name: self.name.clone(),
            path,
            reason: e.to_string(),
        })
    }

    pub fn write_meta(&self, meta: &SessionFile) -> Result<(), StoreError> {
        let bytes = serde_json::to_vec_pretty(meta).expect("session metadata always serializes");
        atomic_write(&self.meta_path(), &bytes)
    }

    /// Removes the session once it holds the lock, waiting up to `wait`.
    /// Refuses while named checkpoints exist unless `with_checkpoints`. A
    /// directory with no `session.json` can be removed too. The lock file stays.
    pub fn remove(&self, with_checkpoints: bool, wait: Duration) -> Result<(), StoreError> {
        if !self.dir().is_dir() {
            return Err(self.no_session());
        }
        let _lock = self.acquire(wait)?;
        if !self.dir().is_dir() {
            return Err(self.no_session());
        }
        let checkpoints = self.checkpoint_names()?;
        if !checkpoints.is_empty() && !with_checkpoints {
            return Err(StoreError::CheckpointsPresent {
                session: self.name.clone(),
                checkpoints,
            });
        }
        self.remove_locked()
    }

    /// Removes the session directory; the caller holds the lock. The
    /// directory is first renamed into `removed/`, one atomic step, so a
    /// process killed while deleting leaves no part of a session under its
    /// name (a `session.json` without its head). Deleting is then best
    /// effort: what is left goes with the next `rm` or `prune`.
    pub(crate) fn remove_locked(&self) -> Result<(), StoreError> {
        self.bundle.sweep_removed();
        let dir = self.dir();
        let removed = self.bundle.removed_dir();
        fs::create_dir_all(&removed).map_err(io_at(&removed))?;
        let aside = removed.join(format!("{}-{}", self.name, new_incarnation()));
        fs::rename(&dir, &aside).map_err(io_at(&dir))?;
        let _ = fs::remove_dir_all(&aside);
        Ok(())
    }

    pub fn row(&self, meta: &SessionFile) -> Result<SessionRow, StoreError> {
        // As a stop record counts with the default base: from the boot
        // handoff, or from power-on before it has been seen.
        let head = self.read_head(meta.rev).ok().flatten().map(|cp| {
            let h = cp.header;
            match h.boot {
                Some(boot) => (h.frame as i64 - boot.frame as i64, TimeBase::Boot),
                None => (h.frame as i64, TimeBase::PowerOn),
            }
        });
        let (head_frame, head_since) = head.unzip();
        Ok(SessionRow {
            name: self.name.clone(),
            rom: meta.rom.clone(),
            model: meta.model,
            cgb: self
                .rom_header(meta)
                .and_then(|h| gbcli_engine::cart::cgb_flag(&h)),
            head_frame,
            head_since,
            rev: meta.rev,
            size_bytes: dir_size(&self.dir(), &self.checkpoints_dir()),
            last_used: self.last_used().unwrap_or_else(now_secs),
            checkpoints: self.checkpoint_names()?.len(),
        })
    }

    /// The first $150 bytes of the session's ROM file as it is on disk now,
    /// or `None` when it can't be read: enough for the header, without
    /// reading a whole cart per row.
    fn rom_header(&self, meta: &SessionFile) -> Option<Vec<u8>> {
        use std::io::Read;
        let file = fs::File::open(self.resolve(Path::new(&meta.rom))).ok()?;
        let mut header = Vec::with_capacity(0x150);
        file.take(0x150).read_to_end(&mut header).ok()?;
        Some(header)
    }

    /// Resolves a stored or given path against the project directory.
    pub(crate) fn resolve(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.bundle.project().join(path)
        }
    }

    /// Stores a path relative to the project when it is inside it.
    pub(crate) fn stored(&self, path: &Path) -> String {
        let absolute = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let project = fs::canonicalize(self.bundle.project())
            .unwrap_or_else(|_| self.bundle.project().to_path_buf());
        absolute
            .strip_prefix(&project)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| absolute.to_string_lossy().into_owned())
    }

    /// A path under the session directory, for display.
    pub fn display_path(&self, path: &Path) -> String {
        path.strip_prefix(self.bundle.project())
            .unwrap_or(path)
            .display()
            .to_string()
    }

    /// Makes the session look unused for `by`.
    #[cfg(test)]
    pub(crate) fn backdate_use(&self, by: Duration) {
        crate::test_fixture::backdate(&self.lock_path(), by);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::test_fixture::{DAY, Fixture};

    const SHORT: Duration = Duration::from_millis(20);

    #[test]
    fn named_checkpoints_are_listed_and_removal_needs_a_flag() {
        let f = Fixture::new("checkpoints");
        let store = f.demo();
        let mut machine = store.machine(&store.read_meta().unwrap()).unwrap();
        store
            .write_checkpoint("f30", &machine.capture(Default::default()))
            .unwrap();
        assert_eq!(store.checkpoint_names().unwrap(), vec!["f30"]);
        assert!(matches!(
            store.remove(false, SHORT),
            Err(StoreError::CheckpointsPresent { .. })
        ));
        assert!(store.exists(), "a refused removal changes nothing");
        store.remove(true, SHORT).unwrap();
        assert!(!store.exists());
    }

    #[test]
    fn removal_waits_for_a_held_lock_then_reports_busy() {
        let f = Fixture::new("rm-busy");
        let store = f.demo();
        let held = store.lock(SHORT).unwrap();
        let err = store.remove(true, SHORT).unwrap_err();
        assert!(matches!(err, StoreError::Busy { .. }), "{err:?}");
        assert!(store.exists());
        drop(held);
        store.remove(true, SHORT).unwrap();
        assert!(!store.exists());
    }

    #[test]
    fn removal_keeps_the_lock_file_so_a_waiting_writer_and_a_new_one_share_it() {
        let f = Fixture::new("rm-lock-file");
        let store = f.demo();
        drop(store.lock(SHORT).unwrap());
        let lock_file = store.lock_path();
        assert!(!lock_file.starts_with(store.dir()), "{lock_file:?}");
        store.remove(true, SHORT).unwrap();
        assert!(lock_file.is_file());
    }

    #[test]
    fn a_directory_without_a_session_file_is_not_a_session() {
        let f = Fixture::new("broken-exists");
        let store = f.bundle.session("typo").unwrap();
        fs::create_dir_all(store.dir()).unwrap();
        assert!(!store.exists());
    }

    /// A row names the model and what the ROM on disk was built for (leaf
    /// jGOxRH), so a DMG session over a `--gbc` build shows it.
    #[test]
    fn a_listing_row_names_the_model_and_the_roms_cgb_flag() {
        use gbcli_core::Model;
        use gbcli_protocol::CgbFlag;
        let f = Fixture::new("list-model");
        f.demo();
        let row = &f.bundle.list().unwrap()[0];
        assert_eq!((row.model, row.cgb), (Model::DmgB, Some(CgbFlag::Dmg)));
        // Rebuilt as dual mode: the row reads the file as it is now.
        let mut rom = crate::test_fixture::ROM.to_vec();
        rom[0x143] = 0x80;
        fs::write(f.root.join("demo.gb"), &rom).unwrap();
        assert_eq!(f.bundle.list().unwrap()[0].cgb, Some(CgbFlag::Dual));
        fs::remove_file(f.root.join("demo.gb")).unwrap();
        let gone = &f.bundle.list().unwrap()[0];
        assert_eq!(
            (gone.model, gone.cgb),
            (Model::DmgB, None),
            "no file, no flag"
        );
    }

    #[test]
    fn listing_skips_directories_that_are_not_sessions() {
        let f = Fixture::new("list-strays");
        f.demo();
        for stray in ["Bad_Name", "typo"] {
            fs::create_dir_all(f.bundle.sessions_dir().join(stray)).unwrap();
        }
        let rows = f.bundle.list().unwrap();
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["demo"]);
        let StoreError::NoSession { known, .. } = f.bundle.session("nope").unwrap().no_session()
        else {
            panic!()
        };
        assert_eq!(known, vec!["demo".to_string()], "only real sessions");
    }

    #[test]
    fn a_removal_that_stops_midway_leaves_nothing_under_the_sessions_name() {
        use gbcli_protocol::Rev;
        use std::os::unix::fs::PermissionsExt;
        let f = Fixture::new("rm-midway");
        let store = f.demo();
        let mut machine = store.machine(&store.read_meta().unwrap()).unwrap();
        let mut meta = store.read_meta().unwrap();
        store
            .write_head(&machine.capture(Default::default()), Rev(1), None)
            .unwrap();
        meta.rev = Rev(1);
        store.write_meta(&meta).unwrap();
        // The head can't be unlinked, so deleting the files stops partway,
        // as a kill would stop it.
        let mode = |dir: &Path, mode| {
            fs::set_permissions(dir, fs::Permissions::from_mode(mode)).unwrap();
        };
        mode(&store.history_dir(), 0o555);
        let result = store.remove(true, SHORT);
        let moved: Vec<PathBuf> = fs::read_dir(f.bundle.removed_dir())
            .map(|d| d.map(|e| e.unwrap().path()).collect())
            .unwrap_or_default();
        for dir in &moved {
            mode(&dir.join("history"), 0o755);
        }
        if store.history_dir().exists() {
            mode(&store.history_dir(), 0o755);
        }
        assert!(result.is_ok(), "{result:?}");
        assert!(!store.dir().exists(), "part of the session is left");
        assert_eq!(moved.len(), 1, "{moved:?}");
        // What the removal couldn't delete goes with the next prune.
        f.bundle.prune(DAY, SHORT).unwrap();
        assert!(!f.bundle.removed_dir().exists());
    }
}
