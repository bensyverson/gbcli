//! One session on disk (design §1): its metadata, head, history, named
//! checkpoints, event log and lock. Every write is atomic, so a killed command
//! leaves the previous head rather than half a file.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gbcli_core::{BootRom, Budget, CoreConfig, Model};
use gbcli_engine::{
    Backend, Checkpoint, LoadOptions, Machine, RunOptions, SaveMeta, SetupError, sha256,
};
use gbcli_protocol::{Location, Rev, SessionFile, SessionRow, model_name};

use crate::bundle::{Bundle, now_secs, validate_name};
use crate::error::StoreError;

/// How long a writer waits for another command's lock before `session_busy`.
pub const LOCK_WAIT: Duration = Duration::from_secs(5);

/// Frames the open boot ROM may take before the handoff. The real one takes
/// about 84; a ROM that never writes FF50 is refused with a remedy.
const BOOT_FRAMES: u64 = 600;

/// The address a normal boot hands over to.
const HANDOFF_PC: u16 = 0x0100;

/// Where a machine starts from when a request doesn't say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    /// Run the boot ROM to the handoff, using the cache when it exists.
    Boot,
    /// Start at power-on; `--from power-on` (design §9).
    PowerOn,
}

/// How to build a new session.
#[derive(Clone, Debug)]
pub struct RomSource {
    pub rom: PathBuf,
    /// The `.sym`; by default the ROM's basename with `.sym` beside it.
    pub sym: Option<PathBuf>,
    pub model: Model,
    pub seed: u64,
}

/// A session directory. `name` is validated when the store is made.
#[derive(Clone, Debug)]
pub struct SessionStore {
    bundle: Bundle,
    name: String,
}

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

impl SessionStore {
    pub fn new(bundle: Bundle, name: &str) -> Result<SessionStore, StoreError> {
        validate_name(name)?;
        Ok(SessionStore {
            bundle,
            name: name.to_string(),
        })
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

    pub fn exists(&self) -> bool {
        self.dir().is_dir()
    }

    fn meta_path(&self) -> PathBuf {
        self.dir().join("session.json")
    }

    fn head_path(&self) -> PathBuf {
        self.dir().join("head.gbck")
    }

    fn history_dir(&self) -> PathBuf {
        self.dir().join("history")
    }

    fn checkpoints_dir(&self) -> PathBuf {
        self.dir().join("checkpoints")
    }

    fn events_path(&self) -> PathBuf {
        self.dir().join("events.jsonl")
    }

    fn lock_path(&self) -> PathBuf {
        self.dir().join("lock")
    }

    fn history_path(&self, rev: Rev) -> PathBuf {
        self.history_dir().join(format!("{rev}.gbck"))
    }

    fn checkpoint_path(&self, name: &str) -> PathBuf {
        self.checkpoints_dir().join(format!("{name}.gbck"))
    }

    /// Creates the session from a ROM. Refuses an existing name.
    pub fn create(&self, source: &RomSource) -> Result<Machine, StoreError> {
        if self.exists() {
            return Err(StoreError::SessionExists {
                name: self.name.clone(),
            });
        }
        let rom_path = self.resolve(&source.rom);
        let rom = fs::read(&rom_path).map_err(|e| StoreError::RomUnreadable {
            path: rom_path.clone(),
            reason: e.to_string(),
        })?;
        let sym_path = source.sym.clone().or_else(|| {
            let sibling = rom_path.with_extension("sym");
            sibling.is_file().then_some(sibling)
        });
        let sym = match &sym_path {
            None => None,
            Some(path) => Some(read_symbols(self, path)?),
        };
        let model = source.model;
        let seed = source.seed;
        let config = default_config();
        let core = CoreConfig {
            model,
            rom,
            boot_rom: BootRom::BuiltIn,
            seed,
            rtc: config.rtc,
            rendering: config.rendering,
            joypad_bounce: config.joypad_bounce,
            write_tracking: config.write_tracking,
        };
        let meta = SessionFile {
            name: self.name.clone(),
            rom: self.stored(&rom_path),
            sym: sym_path.as_deref().map(|p| self.stored(p)),
            model,
            seed,
            rev: Rev(0),
            config,
            debugger: Default::default(),
            last_stop: None,
        };
        let machine =
            Machine::new(Backend::sameboy(), core, sym.as_deref()).map_err(|e| match e {
                SetupError::Symbols(_) => StoreError::SymbolsUnreadable {
                    path: sym_path.clone().unwrap_or_default(),
                    reason: e.to_string(),
                },
                SetupError::Core(_) => StoreError::Setup(e),
            })?;
        for dir in [self.dir(), self.history_dir(), self.checkpoints_dir()] {
            fs::create_dir_all(&dir).map_err(|source| StoreError::Io { path: dir, source })?;
        }
        self.write_meta(&meta)?;
        Ok(machine)
    }

    /// Builds a machine from the session's metadata at power-on.
    pub fn machine(&self, meta: &SessionFile) -> Result<Machine, StoreError> {
        let rom_path = self.resolve(Path::new(&meta.rom));
        let rom = fs::read(&rom_path).map_err(|e| StoreError::RomUnreadable {
            path: rom_path,
            reason: e.to_string(),
        })?;
        let sym = meta
            .sym
            .as_deref()
            .map(|p| read_symbols(self, &self.resolve(Path::new(p))))
            .transpose()?;
        let core = CoreConfig {
            model: meta.model,
            rom,
            boot_rom: BootRom::BuiltIn,
            seed: meta.seed,
            rtc: meta.config.rtc,
            rendering: meta.config.rendering,
            joypad_bounce: meta.config.joypad_bounce,
            write_tracking: meta.config.write_tracking,
        };
        Machine::new(Backend::sameboy(), core, sym.as_deref()).map_err(StoreError::Setup)
    }

    /// A machine at the start state a request asks for: a named checkpoint or
    /// history entry when `at` is given, else the head, else the boot handoff.
    pub fn start_machine(
        &self,
        meta: &SessionFile,
        at: Option<&str>,
        from: Start,
        load: &LoadOptions,
    ) -> Result<(Machine, Vec<String>), StoreError> {
        let mut machine = self.machine(meta)?;
        let mut warnings = Vec::new();
        if let Some(at) = at {
            let cp = self.checkpoint_or_history(meta.rev, at)?;
            warnings.extend(self.restore(&mut machine, &cp, load)?);
        } else if from == Start::PowerOn {
            // Cold: the head and the boot cache are both skipped (design §9).
        } else if let Some(head) = self.read_head()? {
            warnings.extend(self.restore(&mut machine, &head, load)?);
        } else {
            warnings.extend(self.to_handoff(&mut machine, load)?);
        }
        Ok((machine, warnings))
    }

    /// Restores a checkpoint into a machine, turning load warnings into text.
    pub fn restore(
        &self,
        machine: &mut Machine,
        cp: &Checkpoint,
        opts: &LoadOptions,
    ) -> Result<Vec<String>, StoreError> {
        let warnings = machine.restore(cp, opts).map_err(StoreError::Checkpoint)?;
        Ok(warnings.iter().map(StoreError::warning_text).collect())
    }

    /// Runs the boot ROM to the handoff: from the cache when it exists, else
    /// cold, writing the cache afterwards (design §9).
    pub fn to_handoff(
        &self,
        machine: &mut Machine,
        load: &LoadOptions,
    ) -> Result<Vec<String>, StoreError> {
        let probe = machine.capture(SaveMeta::default());
        let path = self.boot_cache_path(&probe);
        if let Ok(bytes) = fs::read(&path)
            && let Ok(cp) = Checkpoint::decode(&bytes)
            && let Ok(warnings) = self.restore(machine, &cp, load)
        {
            return Ok(warnings);
        }
        let _ = fs::remove_file(&path);
        let run = RunOptions::new(Budget::frames(BOOT_FRAMES));
        machine
            .run_to(
                Location {
                    addr: HANDOFF_PC,
                    bank: Some(0),
                    symbol: None,
                },
                &run,
            )
            .map_err(StoreError::BootFailed)?;
        if machine.boot().is_none() {
            return Err(StoreError::NoHandoff);
        }
        let cp = machine.capture(SaveMeta::default());
        self.write_boot_cache(&cp)?;
        Ok(Vec::new())
    }

    /// Keyed on the core identity too, so an entry another build of the core
    /// wrote is never reused: its state may have another layout.
    fn boot_cache_path(&self, cp: &Checkpoint) -> PathBuf {
        let h = &cp.header;
        let core = sha256(format!("{}\0{}", h.core.backend, h.core.commit).as_bytes());
        let key = format!(
            "{}-{}-{}-{}-{}",
            &h.rom_sha256.to_string()[..16],
            model_name(h.model),
            &h.boot_rom_sha256.to_string()[..16],
            h.seed,
            &core.to_string()[..16]
        );
        self.bundle.cache_dir().join(format!("boot-{key}.gbck"))
    }

    fn write_boot_cache(&self, cp: &Checkpoint) -> Result<(), StoreError> {
        let dir = self.bundle.cache_dir();
        fs::create_dir_all(&dir).map_err(|source| StoreError::Io {
            path: dir.clone(),
            source,
        })?;
        atomic_write(&self.boot_cache_path(cp), &cp.encode())
    }

    /// The session's metadata; `no_session` when the directory is missing.
    pub fn read_meta(&self) -> Result<SessionFile, StoreError> {
        let path = self.meta_path();
        let text = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(StoreError::NoSession {
                    name: self.name.clone(),
                    known: self.bundle.session_names().unwrap_or_default(),
                });
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

    /// The current head, if the session has one.
    pub fn read_head(&self) -> Result<Option<Checkpoint>, StoreError> {
        let path = self.head_path();
        match fs::read(&path) {
            Ok(bytes) => Checkpoint::decode(&bytes)
                .map(Some)
                .map_err(StoreError::Checkpoint),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(StoreError::Io { path, source }),
        }
    }

    /// Writes the new head, keeping the old one as history and pruning to the
    /// last 8 (design §1).
    pub fn write_head(&self, cp: &Checkpoint, rev: Rev) -> Result<(), StoreError> {
        if let Ok(old) = fs::read(self.head_path()) {
            let old_rev = Rev(rev.0.saturating_sub(1));
            atomic_write(&self.history_path(old_rev), &old)?;
        }
        atomic_write(&self.head_path(), &cp.encode())?;
        self.prune_history()
    }

    /// A named checkpoint.
    pub fn checkpoint(&self, name: &str) -> Result<Checkpoint, StoreError> {
        let path = self.checkpoint_path(name);
        let bytes = fs::read(&path).map_err(|_| StoreError::CheckpointMissing {
            name: name.to_string(),
            known: self.checkpoint_names().unwrap_or_default(),
        })?;
        Checkpoint::decode(&bytes).map_err(StoreError::Checkpoint)
    }

    /// A checkpoint by name, or `~N` for N heads back.
    pub fn checkpoint_or_history(&self, head_rev: Rev, at: &str) -> Result<Checkpoint, StoreError> {
        if let Some(back) = at.strip_prefix('~') {
            let back: u64 = back.parse().map_err(|_| StoreError::BadHistoryRef {
                text: at.to_string(),
            })?;
            if back == 0 {
                return self
                    .read_head()?
                    .ok_or_else(|| StoreError::CheckpointMissing {
                        name: at.to_string(),
                        known: Vec::new(),
                    });
            }
            if back > head_rev.0 {
                return Err(StoreError::CheckpointMissing {
                    name: at.to_string(),
                    known: self.history_names(),
                });
            }
            let path = self.history_path(Rev(head_rev.0 - back));
            let bytes = fs::read(&path).map_err(|_| StoreError::CheckpointMissing {
                name: at.to_string(),
                known: self.history_names(),
            })?;
            return Checkpoint::decode(&bytes).map_err(StoreError::Checkpoint);
        }
        self.checkpoint(at)
    }

    pub fn write_checkpoint(&self, name: &str, cp: &Checkpoint) -> Result<(), StoreError> {
        validate_checkpoint_name(name)?;
        let dir = self.checkpoints_dir();
        fs::create_dir_all(&dir).map_err(|source| StoreError::Io {
            path: dir.clone(),
            source,
        })?;
        atomic_write(&self.checkpoint_path(name), &cp.encode())
    }

    pub fn checkpoint_names(&self) -> Result<Vec<String>, StoreError> {
        let dir = self.checkpoints_dir();
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => return Err(StoreError::Io { path: dir, source }),
        };
        let mut names: Vec<String> = entries
            .filter_map(Result::ok)
            .filter_map(|e| {
                let path = e.path();
                if path.extension().is_some_and(|x| x == "gbck") {
                    path.file_stem()
                        .and_then(|s| s.to_str())
                        .map(str::to_string)
                } else {
                    None
                }
            })
            .collect();
        names.sort();
        Ok(names)
    }

    /// Removes the session; refuses while named checkpoints exist.
    pub fn remove(&self, with_checkpoints: bool) -> Result<(), StoreError> {
        let checkpoints = self.checkpoint_names()?;
        if !checkpoints.is_empty() && !with_checkpoints {
            return Err(StoreError::CheckpointsPresent {
                session: self.name.clone(),
                checkpoints,
            });
        }
        let dir = self.dir();
        fs::remove_dir_all(&dir).map_err(|source| StoreError::Io { path: dir, source })
    }

    /// Host time of the last write to the session, for pruning.
    pub fn last_used(&self) -> Result<u64, StoreError> {
        let path = self.meta_path();
        let modified = fs::metadata(&path)
            .and_then(|m| m.modified())
            .map_err(|source| StoreError::Io {
                path: path.clone(),
                source,
            })?;
        Ok(modified
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()))
    }

    pub fn row(&self, meta: &SessionFile) -> Result<SessionRow, StoreError> {
        let head_frame = self.read_head().ok().flatten().map(|cp| cp.header.frame);
        Ok(SessionRow {
            name: self.name.clone(),
            rom: meta.rom.clone(),
            head_frame,
            rev: meta.rev,
            size_bytes: dir_size(&self.dir(), &self.checkpoints_dir()),
            last_used: self.last_used().unwrap_or_else(|_| now_secs()),
            checkpoints: self.checkpoint_names()?.len(),
        })
    }

    /// A writer's lock; waits up to `wait`, then `session_busy`.
    pub fn lock(&self, wait: Duration) -> Result<SessionLock, StoreError> {
        let dir = self.dir();
        fs::create_dir_all(&dir).map_err(|source| StoreError::Io {
            path: dir.clone(),
            source,
        })?;
        let path = self.lock_path();
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&path)
            .map_err(|source| StoreError::Io {
                path: path.clone(),
                source,
            })?;
        let start = Instant::now();
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(SessionLock { file }),
                Err(TryLockError::WouldBlock) if start.elapsed() < wait => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(TryLockError::WouldBlock) => {
                    return Err(StoreError::Busy {
                        name: self.name.clone(),
                        waited: start.elapsed(),
                    });
                }
                Err(TryLockError::Error(source)) => {
                    return Err(StoreError::Io { path, source });
                }
            }
        }
    }

    pub fn append_events(&self, lines: &[gbcli_protocol::EventLogLine]) -> Result<(), StoreError> {
        let path = self.events_path();
        let mut text = String::new();
        for line in lines {
            text.push_str(&serde_json::to_string(line).expect("an event line serializes"));
            text.push('\n');
        }
        append(&path, &text)
    }

    pub fn events(
        &self,
        limit: Option<usize>,
    ) -> Result<Vec<gbcli_protocol::EventLogLine>, StoreError> {
        let path = self.events_path();
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => return Err(StoreError::Io { path, source }),
        };
        let mut lines: Vec<_> = text
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        if let Some(limit) = limit {
            let skip = lines.len().saturating_sub(limit);
            lines.drain(..skip);
        }
        Ok(lines)
    }

    fn history_names(&self) -> Vec<String> {
        let Ok(entries) = fs::read_dir(self.history_dir()) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .filter_map(Result::ok)
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .collect();
        names.sort();
        names
    }

    /// Keeps the eight newest history entries.
    fn prune_history(&self) -> Result<(), StoreError> {
        let mut stored: Vec<(u64, PathBuf)> = self
            .history_names()
            .into_iter()
            .filter_map(|name| {
                let rev = name
                    .strip_suffix(".gbck")?
                    .strip_prefix('s')?
                    .parse()
                    .ok()?;
                Some((rev, self.history_dir().join(name)))
            })
            .collect();
        stored.sort_by_key(|(rev, _)| *rev);
        for (_, path) in stored.iter().rev().skip(8) {
            let _ = fs::remove_file(path);
        }
        Ok(())
    }

    /// Resolves a stored path against the project directory.
    fn resolve(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.bundle.project().join(path)
        }
    }

    /// Stores a path relative to the project when it is inside it.
    fn stored(&self, path: &Path) -> String {
        let absolute = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let project = std::fs::canonicalize(self.bundle.project())
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
}

fn read_symbols(store: &SessionStore, path: &Path) -> Result<String, StoreError> {
    fs::read_to_string(path).map_err(|e| StoreError::SymbolsUnreadable {
        path: store.resolve(path),
        reason: e.to_string(),
    })
}

fn default_config() -> gbcli_protocol::RenderConfig {
    gbcli_protocol::RenderConfig {
        rtc: gbcli_core::RtcMode::Accurate,
        rendering: gbcli_core::Rendering::On,
        joypad_bounce: gbcli_core::JoypadBounce::Off,
        write_tracking: gbcli_core::WriteTracking::On,
    }
}

/// A checkpoint name: letters, digits, `-`, `_` and `.`, no path parts.
pub fn validate_checkpoint_name(name: &str) -> Result<(), StoreError> {
    let ok = (1..=64).contains(&name.len())
        && name != "."
        && name != ".."
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if ok {
        Ok(())
    } else {
        Err(StoreError::BadCheckpointName {
            name: name.to_string(),
        })
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    let parent = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(|source| StoreError::Io {
        path: parent.to_path_buf(),
        source,
    })?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temp = parent.join(format!(".{name}.tmp.{}", std::process::id()));
    fs::write(&temp, bytes).map_err(|source| StoreError::Io {
        path: temp.clone(),
        source,
    })?;
    fs::rename(&temp, path).map_err(|source| StoreError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn append(path: &Path, text: &str) -> Result<(), StoreError> {
    let parent = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(|source| StoreError::Io {
        path: parent.to_path_buf(),
        source,
    })?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|source| StoreError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    file.write_all(text.as_bytes())
        .map_err(|source| StoreError::Io {
            path: path.to_path_buf(),
            source,
        })
}

fn dir_size(dir: &Path, exclude: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let mut total = 0;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path == exclude {
            continue;
        }
        match entry.metadata() {
            Ok(meta) if meta.is_dir() => total += dir_size(&path, exclude),
            Ok(meta) => total += meta.len(),
            Err(_) => {}
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use gbcli_core::Model;

    const ROM: &[u8] = include_bytes!("../../engine/tests/roms/rebuild_v1.gb");

    struct Fixture {
        root: PathBuf,
        bundle: Bundle,
    }

    impl Fixture {
        fn new(name: &str) -> Fixture {
            let root =
                std::env::temp_dir().join(format!("gbcli-store-{}-{}", std::process::id(), name));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).unwrap();
            fs::write(root.join("demo.gb"), ROM).unwrap();
            let bundle = Bundle::open_or_create(&root).unwrap();
            Fixture { root, bundle }
        }

        fn source(&self) -> RomSource {
            RomSource {
                rom: self.root.join("demo.gb"),
                sym: None,
                model: Model::DmgB,
                seed: 0,
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn creating_a_session_writes_metadata_and_holds_no_head() {
        let f = Fixture::new("create");
        let store = f.bundle.session("demo").unwrap();
        let _machine = store.create(&f.source()).unwrap();
        assert!(store.exists());
        let meta = store.read_meta().unwrap();
        assert_eq!(meta.name, "demo");
        assert_eq!(meta.rev, Rev(0));
        assert_eq!(meta.rom, "demo.gb");
        assert!(store.read_head().unwrap().is_none());
        assert_eq!(f.bundle.session_names().unwrap(), vec!["demo"]);
    }

    #[test]
    fn a_missing_rom_is_refused_before_anything_is_written() {
        let f = Fixture::new("missing-rom");
        let store = f.bundle.session("gone").unwrap();
        let mut source = f.source();
        source.rom = f.root.join("nope.gb");
        assert!(matches!(
            store.create(&source),
            Err(StoreError::RomUnreadable { .. })
        ));
        assert!(!store.exists());
    }

    #[test]
    fn the_head_round_trips_and_the_old_one_goes_to_history() {
        let f = Fixture::new("head");
        let store = f.bundle.session("demo").unwrap();
        let mut machine = store.create(&f.source()).unwrap();
        let first = machine.capture(SaveMeta::default());
        store.write_head(&first, Rev(1)).unwrap();
        assert_eq!(store.read_head().unwrap(), Some(first.clone()));

        // Advance and write rev 2; rev 1 becomes history.
        let run = RunOptions::new(Budget::frames(1));
        machine.run(&run).unwrap();
        let second = machine.capture(SaveMeta::default());
        store.write_head(&second, Rev(2)).unwrap();
        assert_eq!(store.read_head().unwrap(), Some(second));
        let back = store.checkpoint_or_history(Rev(2), "~1").unwrap();
        assert_eq!(back.header.ticks, first.header.ticks);
        assert!(matches!(
            store.checkpoint_or_history(Rev(2), "~9"),
            Err(StoreError::CheckpointMissing { .. })
        ));
    }

    #[test]
    fn a_boot_cache_entry_from_another_core_identity_is_not_reused() {
        let f = Fixture::new("boot-cache-identity");
        let store = f.bundle.session("demo").unwrap();
        let mut machine = store.create(&f.source()).unwrap();
        // A loadable entry at power-on, as another build of the core would have
        // written it: reused, it would leave the machine short of the handoff.
        let mut foreign = machine.capture(SaveMeta::default());
        foreign.header.core.commit = "0000000+patches.0000000000000000".into();
        store.write_boot_cache(&foreign).unwrap();

        let warnings = store
            .to_handoff(&mut machine, &LoadOptions::default())
            .unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(machine.boot().is_some());
        let cached = fs::read_dir(f.bundle.cache_dir()).unwrap().count();
        assert_eq!(cached, 2, "each identity keeps its own entry");
    }

    #[test]
    fn history_keeps_the_eight_newest_entries() {
        let f = Fixture::new("history");
        let store = f.bundle.session("demo").unwrap();
        let machine = store.create(&f.source()).unwrap();
        let cp = machine.capture(SaveMeta::default());
        for rev in 1..=12 {
            store.write_head(&cp, Rev(rev)).unwrap();
        }
        let names = store.history_names();
        assert_eq!(names.len(), 8, "{names:?}");
        assert!(names.contains(&"s11.gbck".to_string()));
        assert!(!names.contains(&"s3.gbck".to_string()));
    }

    #[test]
    fn named_checkpoints_are_listed_and_removal_needs_a_flag() {
        let f = Fixture::new("checkpoints");
        let store = f.bundle.session("demo").unwrap();
        let machine = store.create(&f.source()).unwrap();
        store
            .write_checkpoint("f30", &machine.capture(SaveMeta::default()))
            .unwrap();
        assert_eq!(store.checkpoint_names().unwrap(), vec!["f30"]);
        assert!(store.checkpoint("f30").is_ok());
        assert!(matches!(
            store.remove(false),
            Err(StoreError::CheckpointsPresent { .. })
        ));
        assert!(store.exists(), "a refused removal changes nothing");
        store.remove(true).unwrap();
        assert!(!store.exists());
    }

    #[test]
    fn pruning_removes_sessions_unused_for_longer_than_asked() {
        let f = Fixture::new("prune");
        let store = f.bundle.session("demo").unwrap();
        store.create(&f.source()).unwrap();
        let meta = store.dir().join("session.json");
        let old = std::time::SystemTime::now() - Duration::from_secs(10 * 24 * 60 * 60);
        fs::File::options()
            .write(true)
            .open(&meta)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(old))
            .unwrap();
        assert!(
            f.bundle
                .prune(Duration::from_secs(30 * 24 * 60 * 60))
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            f.bundle.prune(Duration::from_secs(24 * 60 * 60)).unwrap(),
            vec!["demo".to_string()]
        );
        assert!(f.bundle.session_names().unwrap().is_empty());
    }

    #[test]
    fn a_checkpoint_name_cannot_escape_its_directory() {
        for bad in ["", ".", "..", "a/b", "a\\b"] {
            assert!(validate_checkpoint_name(bad).is_err(), "{bad:?}");
        }
        assert!(validate_checkpoint_name("f30").is_ok());
    }

    #[test]
    fn the_lock_is_exclusive_and_released_on_drop() {
        let f = Fixture::new("lock");
        let store = f.bundle.session("demo").unwrap();
        store.create(&f.source()).unwrap();
        let held = store.lock(Duration::from_millis(1)).unwrap();
        let err = store.lock(Duration::from_millis(10)).unwrap_err();
        assert!(matches!(err, StoreError::Busy { .. }), "{err:?}");
        drop(held);
        assert!(store.lock(Duration::from_millis(10)).is_ok());
    }
}
