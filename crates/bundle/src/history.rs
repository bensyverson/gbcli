//! Heads and history (design §1). Every head is written once, as
//! `history/s<rev>.gbck`, before `session.json` names its revision; the head
//! is the entry `session.json`'s `rev` names. A process killed between the two
//! writes leaves an entry no metadata names, which the next save overwrites,
//! so real history is never lost.

use std::fs;
use std::path::PathBuf;

use gbcli_engine::Checkpoint;
use gbcli_protocol::{Rev, SessionFile, UndoWalk};

use crate::error::StoreError;
use crate::io::{atomic_write, ensure_dir};
use crate::store::SessionStore;

/// Heads kept behind the current one: `~1` through `~8`.
pub const HISTORY_BACK: u64 = 8;

impl SessionStore {
    fn history_path(&self, rev: Rev) -> PathBuf {
        self.history_dir().join(format!("{rev}.gbck"))
    }

    /// The head at `rev`; `None` at rev 0, which has none.
    pub fn read_head(&self, rev: Rev) -> Result<Option<Checkpoint>, StoreError> {
        if rev.0 == 0 {
            return Ok(None);
        }
        let path = self.history_path(rev);
        match fs::read(&path) {
            Ok(bytes) => Checkpoint::decode(&bytes)
                .map(Some)
                .map_err(StoreError::Checkpoint),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(StoreError::HeadMissing {
                name: self.name().to_string(),
                rev: rev.0,
            }),
            Err(source) => Err(StoreError::Io { path, source }),
        }
    }

    /// Writes the head for `rev` and drops entries more than
    /// [`HISTORY_BACK`] behind it, or behind the walk's origin while `walk`
    /// is an undo walk in progress, so the walk can reach every entry that
    /// was kept when it began. The caller then writes `session.json`.
    pub fn write_head(
        &self,
        cp: &Checkpoint,
        rev: Rev,
        walk: Option<&UndoWalk>,
    ) -> Result<(), StoreError> {
        ensure_dir(&self.history_dir())?;
        atomic_write(&self.history_path(rev), &cp.encode())?;
        self.prune_history(walk.map_or(rev, |w| w.origin.min(rev)));
        Ok(())
    }

    /// What the next undo restores (design §10): the entry before the one
    /// the last undo restored when the head is part of a walk, else the
    /// entry before the head. Refused past the oldest entry kept.
    pub fn undo_target(&self, meta: &SessionFile) -> Result<(Checkpoint, UndoWalk), StoreError> {
        let (from, origin) = match meta.undo {
            Some(walk) => (walk.restored, walk.origin),
            None => (meta.rev, meta.rev),
        };
        // `from` is the head, or the entry the walk restored last: either
        // way the oldest kept once nothing precedes it.
        let past = || StoreError::UndoPastOldest {
            name: self.name().to_string(),
            head: meta.rev,
            oldest: (from.0 >= 1).then_some(from),
        };
        let target = from
            .0
            .checked_sub(1)
            .filter(|&rev| rev >= 1)
            .ok_or_else(past)?;
        match self.read_head(Rev(target)) {
            Ok(Some(cp)) => Ok((
                cp,
                UndoWalk {
                    restored: Rev(target),
                    origin,
                },
            )),
            Ok(None) | Err(StoreError::HeadMissing { .. }) => Err(past()),
            Err(e) => Err(e),
        }
    }

    /// A checkpoint by name, or `~N` for the head N revisions back.
    pub fn checkpoint_or_history(&self, head_rev: Rev, at: &str) -> Result<Checkpoint, StoreError> {
        let Some(back) = at.strip_prefix('~') else {
            return self.checkpoint(at);
        };
        let bad = || StoreError::BadHistoryRef {
            text: at.to_string(),
        };
        if back.is_empty() || !back.bytes().all(|b| b.is_ascii_digit()) {
            return Err(bad());
        }
        let back: u64 = back.parse().map_err(|_| bad())?;
        let missing = || StoreError::CheckpointMissing {
            name: at.to_string(),
            known: self.history_refs(head_rev),
        };
        let target = head_rev.0.checked_sub(back).ok_or_else(missing)?;
        match self.read_head(Rev(target)) {
            Ok(Some(cp)) => Ok(cp),
            Ok(None) | Err(StoreError::HeadMissing { .. }) => Err(missing()),
            Err(e) => Err(e),
        }
    }

    /// Revisions with a stored entry, oldest first.
    pub(crate) fn history_revs(&self) -> Vec<u64> {
        let Ok(entries) = fs::read_dir(self.history_dir()) else {
            return Vec::new();
        };
        let mut revs: Vec<u64> = entries
            .filter_map(Result::ok)
            .filter_map(|e| {
                e.file_name()
                    .to_str()?
                    .strip_suffix(".gbck")?
                    .strip_prefix('s')?
                    .parse()
                    .ok()
            })
            .collect();
        revs.sort_unstable();
        revs
    }

    /// The `~N` forms that resolve from `head_rev`, nearest first.
    fn history_refs(&self, head_rev: Rev) -> Vec<String> {
        let mut back: Vec<u64> = self
            .history_revs()
            .into_iter()
            .filter(|&rev| rev >= 1 && rev <= head_rev.0)
            .map(|rev| head_rev.0 - rev)
            .collect();
        back.sort_unstable();
        back.iter().map(|n| format!("~{n}")).collect()
    }

    /// Drops entries more than [`HISTORY_BACK`] behind `head`. Best effort: an
    /// entry left behind costs disk, not correctness.
    fn prune_history(&self, head: Rev) {
        let oldest = head.0.saturating_sub(HISTORY_BACK);
        for rev in self.history_revs() {
            if rev < oldest {
                let _ = fs::remove_file(self.history_path(Rev(rev)));
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use gbcli_core::Budget;
    use gbcli_engine::{Machine, RunOptions, SaveMeta};

    use super::*;
    use crate::test_fixture::Fixture;

    fn advance(machine: &mut Machine) -> Checkpoint {
        machine.run(&RunOptions::new(Budget::frames(1))).unwrap();
        machine.capture(SaveMeta::default())
    }

    /// A save as the server makes one: head, then metadata.
    fn save(store: &SessionStore, cp: &Checkpoint, rev: u64) {
        store.write_head(cp, Rev(rev), None).unwrap();
        let mut meta = store.read_meta().unwrap();
        meta.rev = Rev(rev);
        store.write_meta(&meta).unwrap();
    }

    #[test]
    fn the_head_round_trips_and_the_old_one_stays_reachable() {
        let f = Fixture::new("head");
        let store = f.demo();
        let mut machine = store.machine(&store.read_meta().unwrap()).unwrap();
        let first = advance(&mut machine);
        save(&store, &first, 1);
        assert_eq!(store.read_head(Rev(1)).unwrap(), Some(first.clone()));
        let second = advance(&mut machine);
        save(&store, &second, 2);
        assert_eq!(store.read_head(Rev(2)).unwrap(), Some(second.clone()));
        assert_eq!(store.checkpoint_or_history(Rev(2), "~0").unwrap(), second);
        let back = store.checkpoint_or_history(Rev(2), "~1").unwrap();
        assert_eq!(back.header.ticks, first.header.ticks);
        assert!(matches!(
            store.checkpoint_or_history(Rev(2), "~2"),
            Err(StoreError::CheckpointMissing { .. })
        ));
    }

    #[test]
    fn a_kill_between_the_head_and_metadata_writes_loses_no_history() {
        let f = Fixture::new("head-kill");
        let store = f.demo();
        let mut machine = store.machine(&store.read_meta().unwrap()).unwrap();
        let first = advance(&mut machine);
        save(&store, &first, 1);
        // Killed after the head write: session.json still says rev 1.
        let orphan = advance(&mut machine);
        store.write_head(&orphan, Rev(2), None).unwrap();
        assert_eq!(store.read_meta().unwrap().rev, Rev(1));
        // The next request starts from rev 1's head and saves rev 2.
        let next = advance(&mut machine);
        save(&store, &next, 2);
        let back = store.checkpoint_or_history(Rev(2), "~1").unwrap();
        assert_eq!(
            back.header.ticks, first.header.ticks,
            "rev 1 was overwritten"
        );
        let head = store.checkpoint_or_history(Rev(2), "~0").unwrap();
        assert_eq!(head.header.ticks, next.header.ticks);
    }

    #[test]
    fn the_head_before_a_kill_is_the_one_metadata_names() {
        let f = Fixture::new("head-kill-read");
        let store = f.demo();
        let mut machine = store.machine(&store.read_meta().unwrap()).unwrap();
        let first = advance(&mut machine);
        save(&store, &first, 1);
        store
            .write_head(&advance(&mut machine), Rev(2), None)
            .unwrap();
        let rev = store.read_meta().unwrap().rev;
        let head = store.read_head(rev).unwrap().unwrap();
        assert_eq!(head.header.ticks, first.header.ticks);
    }

    #[test]
    fn history_reaches_eight_back_and_lists_the_forms_in_order_past_that() {
        let f = Fixture::new("history");
        let store = f.demo();
        let mut machine = store.machine(&store.read_meta().unwrap()).unwrap();
        let mut ticks = Vec::new();
        for rev in 1..=12 {
            let cp = advance(&mut machine);
            ticks.push(cp.header.ticks);
            save(&store, &cp, rev);
        }
        let eight = store.checkpoint_or_history(Rev(12), "~8").unwrap();
        assert_eq!(eight.header.ticks, ticks[3], "~8 from rev 12 is rev 4");
        let err = store.checkpoint_or_history(Rev(12), "~9").unwrap_err();
        let StoreError::CheckpointMissing { known, .. } = err else {
            panic!("{err:?}");
        };
        let expected: Vec<String> = (0..=8).map(|n| format!("~{n}")).collect();
        assert_eq!(known, expected);
        assert_eq!(store.history_revs(), (4..=12).collect::<Vec<_>>());
    }

    #[test]
    fn a_history_reference_must_be_a_tilde_and_digits() {
        let f = Fixture::new("history-ref");
        let store = f.demo();
        for bad in ["~", "~x", "~-1", "~+1"] {
            assert!(
                matches!(
                    store.checkpoint_or_history(Rev(1), bad),
                    Err(StoreError::BadHistoryRef { .. })
                ),
                "{bad}"
            );
        }
    }
}
