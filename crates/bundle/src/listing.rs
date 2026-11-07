//! What `--at` can name in a session (design §12 `checkpoints`): named
//! checkpoints, sorted by name, then history entries, newest first.

use std::fs;
use std::path::Path;

use gbcli_engine::{Checkpoint, sha256};
use gbcli_protocol::{CheckpointRow, Rev, SessionFile};

use crate::error::StoreError;
use crate::store::SessionStore;

impl SessionStore {
    /// Every named checkpoint and history entry behind the head, with the
    /// revision it was saved at, its frame since boot, and whether it was
    /// saved from the ROM file the session names now. An entry that doesn't
    /// load is a row carrying its error, so one bad file hides nothing else.
    pub fn checkpoint_rows(&self, meta: &SessionFile) -> Result<Vec<CheckpointRow>, StoreError> {
        let rom = fs::read(self.resolve(Path::new(&meta.rom)))
            .ok()
            .map(|bytes| sha256(&bytes));
        let row = |at: String, rev: Option<Rev>, cp: Result<Checkpoint, StoreError>| match cp {
            Ok(cp) => CheckpointRow {
                at,
                rev: rev.or(cp.header.rev),
                frame: cp
                    .header
                    .boot
                    .map(|boot| cp.header.frame as i64 - boot.frame as i64),
                rom_matches: Some(rom == Some(cp.header.rom_sha256)),
                error: None,
            },
            Err(e) => CheckpointRow {
                at,
                rev,
                frame: None,
                rom_matches: None,
                error: Some(e.to_string()),
            },
        };
        let mut rows = Vec::new();
        for name in self.checkpoint_names()? {
            let cp = self.checkpoint(&name);
            rows.push(row(name, None, cp));
        }
        for rev in self.history_revs().into_iter().rev() {
            if rev >= meta.rev.0 {
                continue;
            }
            let at = format!("~{}", meta.rev.0 - rev);
            match self.read_head(Rev(rev)) {
                Ok(None) => {}
                Ok(Some(cp)) => rows.push(row(at, Some(Rev(rev)), Ok(cp))),
                Err(e) => rows.push(row(at, Some(Rev(rev)), Err(e))),
            }
        }
        Ok(rows)
    }
}
