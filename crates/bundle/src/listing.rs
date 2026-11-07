//! What `--at` can name in a session (design §12 `checkpoints`): named
//! checkpoints, sorted by name, then history entries, newest first.

use std::fs;
use std::path::Path;

use gbcli_engine::{Checkpoint, sha256};
use gbcli_protocol::{CheckpointRow, EventLogLine, Op, Rev, SessionFile};

use crate::error::StoreError;
use crate::store::SessionStore;

impl SessionStore {
    /// Every named checkpoint and history entry behind the head, with the
    /// revision it was saved at, its frame since boot, and whether it was
    /// saved from the ROM file the session names now.
    pub fn checkpoint_rows(&self, meta: &SessionFile) -> Result<Vec<CheckpointRow>, StoreError> {
        let rom = fs::read(self.resolve(Path::new(&meta.rom)))
            .ok()
            .map(|bytes| sha256(&bytes));
        let row = |at: String, rev: Option<Rev>, cp: &Checkpoint| CheckpointRow {
            at,
            rev,
            frame: cp
                .header
                .boot
                .map(|boot| cp.header.frame as i64 - boot.frame as i64),
            rom_matches: rom == Some(cp.header.rom_sha256),
        };
        let events = self.events(None)?;
        let mut rows = Vec::new();
        for name in self.checkpoint_names()? {
            let cp = self.checkpoint(&name)?;
            rows.push(row(name.clone(), saved_at(&events, &name), &cp));
        }
        for rev in self.history_revs().into_iter().rev() {
            if rev >= meta.rev.0 {
                continue;
            }
            if let Some(cp) = self.read_head(Rev(rev))? {
                rows.push(row(format!("~{}", meta.rev.0 - rev), Some(Rev(rev)), &cp));
            }
        }
        Ok(rows)
    }
}

/// The head's revision when the checkpoint `name` was last written, from
/// the event log: a `checkpoint` op, or a run that saved it.
fn saved_at(events: &[EventLogLine], name: &str) -> Option<Rev> {
    events
        .iter()
        .rev()
        .find(|e| match &e.op {
            Op::Checkpoint(args) => args.name == name,
            Op::Run(args) => args.checkpoint.as_deref() == Some(name),
            _ => false,
        })
        .map(|e| e.rev_after)
}
