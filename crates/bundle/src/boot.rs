//! Start states (design §9): the head, a named or history checkpoint, power-on,
//! or the boot handoff, which the bundle caches in `cache/boot-<key>.gbck`.

use std::sync::atomic::AtomicBool;

use gbcli_core::Budget;
use gbcli_engine::{Checkpoint, LoadOptions, Machine, RunOptions, SaveMeta, sha256};
use gbcli_protocol::{Location, SessionFile, StopReason, model_name};

use crate::error::StoreError;
use crate::project_store::Key;
use crate::store::SessionStore;

/// Frames the open boot ROM may take before the handoff. The real one takes
/// about 84; a ROM that never writes FF50 is refused with a remedy.
const BOOT_FRAMES: u64 = 600;

/// The address a normal boot hands over to.
const HANDOFF_PC: u16 = 0x0100;

/// Where a machine starts instead of the head: the protocol's own `--from`,
/// so the request and the store share one type.
pub use gbcli_protocol::Start;

impl SessionStore {
    /// A machine at the start state a request asks for: a named checkpoint or
    /// history entry when `at` is given, else `from` when given (skipping the
    /// head: how a session restarts on a rebuilt ROM), else the head, else
    /// the boot handoff. `interrupt` ends a boot run early, as the caller's
    /// wall guard.
    pub fn start_machine(
        &self,
        meta: &SessionFile,
        at: Option<&str>,
        from: Option<Start>,
        load: &LoadOptions,
        interrupt: Option<&AtomicBool>,
    ) -> Result<(Machine, Vec<String>), StoreError> {
        let mut machine = self.machine(meta)?;
        let mut warnings = Vec::new();
        if let Some(at) = at {
            let cp = self.checkpoint_or_history(meta.rev, at)?;
            warnings.extend(self.restore(meta, &mut machine, &cp, load)?);
        } else if from == Some(Start::PowerOn) {
            // Cold: the head and the boot cache are both skipped (design §9).
        } else if from == Some(Start::Boot) {
            warnings.extend(self.to_handoff(meta, &mut machine, load, interrupt)?);
        } else if let Some(head) = self.read_head(meta.rev)? {
            warnings.extend(self.restore(meta, &mut machine, &head, load)?);
        } else {
            warnings.extend(self.to_handoff(meta, &mut machine, load, interrupt)?);
        }
        Ok((machine, warnings))
    }

    /// Side B's machine for `equiv --at`: `meta` is B's own ROM, and `at` is
    /// the state saved on side A's, so a changed-ROM warning is worded for
    /// B, not as a rebuild of the session's file.
    pub fn start_side_b_at(
        &self,
        meta: &SessionFile,
        at: &str,
        load: &LoadOptions,
    ) -> Result<(Machine, Vec<String>), StoreError> {
        let mut machine = self.machine(meta)?;
        let cp = self.checkpoint_or_history(meta.rev, at)?;
        let warnings = machine
            .restore(&cp, load)
            .map_err(StoreError::Checkpoint)?
            .iter()
            .map(|w| StoreError::side_b_warning_text(w, &meta.rom))
            .collect();
        Ok((machine, warnings))
    }

    /// A machine at the head, or at power-on when there is none yet: what a
    /// request that inspects the session sees. Never runs the boot ROM.
    pub fn head_or_power_on(
        &self,
        meta: &SessionFile,
        load: &LoadOptions,
    ) -> Result<(Machine, Vec<String>), StoreError> {
        let mut machine = self.machine(meta)?;
        let warnings = match self.read_head(meta.rev)? {
            Some(head) => self.restore(meta, &mut machine, &head, load)?,
            None => Vec::new(),
        };
        Ok((machine, warnings))
    }

    /// Restores a checkpoint into a machine built from `meta`, turning load
    /// warnings into text that names `meta`'s ROM.
    pub fn restore(
        &self,
        meta: &SessionFile,
        machine: &mut Machine,
        cp: &Checkpoint,
        opts: &LoadOptions,
    ) -> Result<Vec<String>, StoreError> {
        let warnings = machine.restore(cp, opts).map_err(StoreError::Checkpoint)?;
        Ok(warnings
            .iter()
            .map(|w| StoreError::warning_text(w, &meta.rom))
            .collect())
    }

    /// Runs the boot ROM to the handoff of a machine built from `meta`: from
    /// the cache when it exists, else cold, writing the cache afterwards
    /// (design §9).
    pub fn to_handoff(
        &self,
        meta: &SessionFile,
        machine: &mut Machine,
        load: &LoadOptions,
        interrupt: Option<&AtomicBool>,
    ) -> Result<Vec<String>, StoreError> {
        let probe = machine.capture(SaveMeta::default());
        let rom_key = machine.boot_key();
        let key = self.boot_cache_key(&probe, rom_key);
        // The entry is keyed on the bytes that decide the handoff, so the ROM
        // that wrote it may differ elsewhere; its state is this ROM's
        // handoff all the same, so it is restored as this ROM's.
        let cached = self.files().get(&key).ok().and_then(|bytes| {
            let mut cp = Checkpoint::decode(&bytes).ok()?;
            cp.header.rom_sha256 = machine.rom_sha256();
            Some(cp)
        });
        if let Some(cp) = cached
            && let Ok(warnings) = self.restore(meta, machine, &cp, load)
        {
            return Ok(warnings);
        }
        let _ = self.files().remove(&key);
        let mut run = RunOptions::new(Budget::frames(BOOT_FRAMES));
        run.interrupt = interrupt;
        let outcome = machine
            .run_to(
                Location {
                    addr: HANDOFF_PC,
                    bank: Some(0),
                    symbol: None,
                },
                &run,
            )
            .map_err(StoreError::BootFailed)?;
        if outcome.stop.reason == StopReason::UserBreak {
            return Err(StoreError::BootInterrupted);
        }
        if machine.boot().is_none() {
            return Err(StoreError::NoHandoff);
        }
        let cp = machine.capture(SaveMeta::default());
        self.write_boot_cache(&cp, rom_key)?;
        Ok(Vec::new())
    }

    /// Keyed on the ROM's `rom_key` ([`Machine::boot_key`]: the bytes that
    /// decide the handoff, or the whole ROM), model, boot ROM, seed, flash
    /// board and core identity: an entry another build of the core wrote is
    /// never reused, as its state may have another layout. Symbols aren't in
    /// the key; the entry carries none.
    fn boot_cache_key(&self, cp: &Checkpoint, rom_key: gbcli_protocol::Sha256) -> Key {
        let h = &cp.header;
        let core = sha256(format!("{}\0{}", h.core.backend, h.core.commit).as_bytes());
        let key = format!(
            "{}-{}-{}-{}-{}",
            &rom_key.to_string()[..16],
            model_name(h.model),
            &h.boot_rom_sha256.to_string()[..16],
            h.seed,
            &core.to_string()[..16]
        );
        // A flash board's name ends the key; mask ROM's keys are unchanged.
        let board = h.flash.as_ref().map_or(String::new(), |b| format!("-{b}"));
        self.bundle()
            .boot_cache_key()
            .join(&format!("boot-{key}{board}.gbck"))
    }

    #[cfg(test)]
    fn boot_cache_path(&self, cp: &Checkpoint) -> std::path::PathBuf {
        self.files()
            .locate(&self.boot_cache_key(cp, cp.header.rom_sha256))
    }

    /// Writes a boot cache entry under `rom_key`. Sessions over one ROM share
    /// it whatever their `.sym`, so the entry carries no symbol fields; a
    /// machine restored from it captures its own.
    fn write_boot_cache(
        &self,
        cp: &Checkpoint,
        rom_key: gbcli_protocol::Sha256,
    ) -> Result<(), StoreError> {
        let dir = self.bundle().boot_cache_key();
        self.files()
            .make_dirs(&dir)
            .map_err(self.bundle().io_at(&dir))?;
        let mut entry = cp.clone();
        entry.header.sym_sha256 = None;
        entry.header.symbols_live.clear();
        let key = self.boot_cache_key(&entry, rom_key);
        self.files()
            .put(&key, &entry.encode())
            .map_err(self.bundle().io_at(&key))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use gbcli_core::Model;

    use super::*;
    use crate::test_fixture::{Fixture, SYM};

    fn boot_entries(f: &Fixture) -> Vec<PathBuf> {
        let mut entries: Vec<PathBuf> = fs::read_dir(f.bundle.cache_dir())
            .map(|d| d.map(|e| e.unwrap().path()).collect())
            .unwrap_or_default();
        entries.sort();
        entries
    }

    fn handoff(store: &SessionStore) -> Machine {
        let meta = store.read_meta().unwrap();
        let mut machine = store.machine(&meta).unwrap();
        let warnings = store
            .to_handoff(&meta, &mut machine, &LoadOptions::default(), None)
            .unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(machine.boot().is_some());
        machine
    }

    #[test]
    fn a_boot_cache_entry_from_another_core_identity_is_not_reused() {
        let f = Fixture::new("boot-cache-identity");
        let store = f.demo();
        let mut machine = store.machine(&store.read_meta().unwrap()).unwrap();
        // A loadable entry at power-on, as another build of the core would have
        // written it: reused, it would leave the machine short of the handoff.
        let mut foreign = machine.capture(SaveMeta::default());
        foreign.header.core.commit = "0000000+patches.0000000000000000".into();
        store
            .write_boot_cache(&foreign, machine.boot_key())
            .unwrap();
        handoff(&store);
        assert_eq!(
            boot_entries(&f).len(),
            2,
            "each identity keeps its own entry"
        );
    }

    #[test]
    fn the_boot_cache_key_names_the_model_and_seed_and_changes_with_them() {
        let f = Fixture::new("boot-cache-key");
        let store = f.demo();
        let mut machine = store.machine(&store.read_meta().unwrap()).unwrap();
        let cp = machine.capture(SaveMeta::default());
        let path = store.boot_cache_path(&cp);
        let name = path.file_name().unwrap().to_str().unwrap();
        assert!(
            name.starts_with("boot-") && name.ends_with(".gbck"),
            "{name}"
        );
        assert!(name.contains("-dmg-b-"), "{name}");
        assert_eq!(store.boot_cache_path(&cp), path, "the key is stable");
        let mut seeded = cp.clone();
        seeded.header.seed = 7;
        assert_ne!(store.boot_cache_path(&seeded), path);
        let mut other_model = cp.clone();
        other_model.header.model = Model::CgbE;
        assert_ne!(store.boot_cache_path(&other_model), path);
        let mut flash = cp.clone();
        flash.header.flash = Some("generic-wr-aaa-a9".into());
        let flash_path = store.boot_cache_path(&flash);
        assert_ne!(flash_path, path, "a flash cart's handoff is another entry");
        assert!(
            flash_path
                .to_str()
                .unwrap()
                .ends_with("-generic-wr-aaa-a9.gbck"),
            "{flash_path:?}"
        );
    }

    #[test]
    fn a_second_handoff_reuses_the_cache_entry() {
        let f = Fixture::new("boot-cache-reuse");
        let store = f.demo();
        let first = handoff(&store).capture(SaveMeta::default());
        let entries = boot_entries(&f);
        assert_eq!(entries.len(), 1);
        // Mark the entry: a reuse reads it, a cold boot would rewrite it.
        let mut marked = Checkpoint::decode(&fs::read(&entries[0]).unwrap()).unwrap();
        marked.header.created_by = Some("cache".into());
        fs::write(&entries[0], marked.encode()).unwrap();
        let second = handoff(&store).capture(SaveMeta::default());
        assert_eq!(second.header.ticks, first.header.ticks);
        let after = Checkpoint::decode(&fs::read(&entries[0]).unwrap()).unwrap();
        assert_eq!(after.header.created_by.as_deref(), Some("cache"));
    }

    /// The fixture's ROM with `at` flipped, as `name` in the fixture's
    /// project, and a session `name` over it.
    fn variant(f: &Fixture, name: &str, at: &[usize]) -> SessionStore {
        let mut rom = crate::test_fixture::ROM.to_vec();
        for &i in at {
            rom[i] ^= 0xFF;
        }
        let path = f.root.join(format!("{name}.gb"));
        fs::write(&path, rom).unwrap();
        let store = f.bundle.session(name).unwrap();
        let mut source = f.source();
        source.rom = path;
        store.create(&source).unwrap();
        store
    }

    /// Marks the one cache entry, so a reuse leaves the mark and a cold
    /// boot's rewrite drops it.
    fn mark(entry: &PathBuf) {
        let mut marked = Checkpoint::decode(&fs::read(entry).unwrap()).unwrap();
        marked.header.created_by = Some("cache".into());
        fs::write(entry, marked.encode()).unwrap();
    }

    fn marked(entry: &PathBuf) -> bool {
        let cp = Checkpoint::decode(&fs::read(entry).unwrap()).unwrap();
        cp.header.created_by.as_deref() == Some("cache")
    }

    /// Past the header ($0150 on) and the global checksum ($014E): bytes no
    /// boot reads.
    const UNREAD: [usize; 3] = [0x14E, 0x14F, 0x1234];

    #[test]
    fn a_rom_that_differs_only_where_the_boot_never_reads_reuses_the_entry() {
        let f = Fixture::new("boot-cache-unread");
        handoff(&f.demo());
        let entries = boot_entries(&f);
        assert_eq!(entries.len(), 1);
        mark(&entries[0]);
        let reused = handoff(&variant(&f, "rebuilt", &UNREAD)).capture(SaveMeta::default());
        assert_eq!(boot_entries(&f), entries, "no entry of its own");
        assert!(marked(&entries[0]), "the entry was reused, not rewritten");

        // The reused handoff is the state a cold boot of that ROM reaches,
        // to the byte, its header naming its own ROM.
        let cold = Fixture::new("boot-cache-unread-cold");
        let cold_cp = handoff(&variant(&cold, "rebuilt", &UNREAD)).capture(SaveMeta::default());
        assert_eq!(reused.header, cold_cp.header);
        assert!(reused.blob == cold_cp.blob, "the core states differ");
    }

    #[test]
    fn a_rom_that_differs_in_a_byte_the_boot_reads_gets_its_own_entry() {
        let f = Fixture::new("boot-cache-read");
        handoff(&f.demo());
        // The logo's first byte and the title's.
        for (name, at) in [("logo", 0x104), ("title", 0x134)] {
            handoff(&variant(&f, name, &[at]));
        }
        assert_eq!(boot_entries(&f).len(), 3);
    }

    #[test]
    fn a_named_boot_cache_holds_the_entries_and_projects_share_it() {
        let shared = std::env::temp_dir().join(format!(
            "gbcli-store-{}-boot-cache-shared",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&shared);
        let entries = || -> Vec<PathBuf> {
            let mut e: Vec<PathBuf> = fs::read_dir(&shared)
                .map(|d| d.map(|e| e.unwrap().path()).collect())
                .unwrap_or_default();
            e.sort();
            e
        };
        let a = Fixture::new("boot-cache-shared-a");
        let bundle = a.bundle.clone().with_boot_cache(Some(&shared));
        let store = bundle.session("demo").unwrap();
        store.create(&a.source()).unwrap();
        handoff(&store);
        assert_eq!(entries().len(), 1, "written to the named cache");
        assert!(boot_entries(&a).is_empty(), "not to the bundle's own");
        mark(&entries()[0]);

        let b = Fixture::new("boot-cache-shared-b");
        let rebuilt = variant(&b, "rebuilt", &UNREAD);
        let store = b
            .bundle
            .clone()
            .with_boot_cache(Some(&shared))
            .session("rebuilt")
            .unwrap();
        handoff(&store);
        assert_eq!(entries().len(), 1);
        assert!(marked(&entries()[0]), "another project reused it");
        assert!(boot_entries(&b).is_empty());
        drop(rebuilt);
        let _ = fs::remove_dir_all(&shared);
    }

    #[test]
    fn a_corrupt_cache_entry_falls_back_to_a_cold_boot_and_is_replaced() {
        let f = Fixture::new("boot-cache-corrupt");
        let store = f.demo();
        let first = handoff(&store).capture(SaveMeta::default());
        let entries = boot_entries(&f);
        fs::write(&entries[0], b"not a checkpoint").unwrap();
        let again = handoff(&store).capture(SaveMeta::default());
        assert_eq!(again.header.ticks, first.header.ticks);
        assert!(Checkpoint::decode(&fs::read(&entries[0]).unwrap()).is_ok());
    }

    #[test]
    fn the_boot_cache_carries_no_symbols_so_sessions_with_another_sym_share_it() {
        let f = Fixture::new("boot-cache-sym");
        fs::write(f.root.join("demo.sym"), SYM).unwrap();
        let with_sym = f.demo();
        assert!(with_sym.read_meta().unwrap().sym.is_some());
        handoff(&with_sym);
        let entries = boot_entries(&f);
        let cached = Checkpoint::decode(&fs::read(&entries[0]).unwrap()).unwrap();
        assert_eq!(cached.header.sym_sha256, None);
        assert!(cached.header.symbols_live.is_empty());

        // A session over the same ROM with other symbols reuses the entry and
        // captures its own.
        let other = f.root.join("other.sym");
        fs::write(&other, "00:0150 Elsewhere\n").unwrap();
        let store = f.bundle.session("other").unwrap();
        let mut source = f.source();
        source.sym = Some(other);
        store.create(&source).unwrap();
        let cp = handoff(&store).capture(SaveMeta::default());
        assert_eq!(boot_entries(&f).len(), 1);
        assert_ne!(cp.header.sym_sha256, None);
        assert_ne!(cp.header.sym_sha256, cached.header.sym_sha256);
    }
}
