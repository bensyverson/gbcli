//! Saving a head: the one place a request moves a session's revision.

use gbcli_bundle::{SessionStore, next};
use gbcli_engine::{Machine, SaveMeta};
use gbcli_protocol::{SessionFile, StopRecord, UndoWalk};

use super::Failure;

/// What a head change records beside the state.
#[derive(Default)]
pub(super) struct HeadChange<'a> {
    pub actor: Option<String>,
    /// The checkpoint or history entry it came from.
    pub parent: Option<String>,
    pub stop: Option<&'a StopRecord>,
    /// Set only by `undo`; every other head change ends the walk (design §10).
    pub walk: Option<UndoWalk>,
}

/// Saves `machine` as the next head, with the debugger configuration it ends
/// with (a `--once` watch that fired is gone).
pub(super) fn save_head(
    store: &SessionStore,
    meta: &mut SessionFile,
    machine: &Machine,
    change: HeadChange,
) -> Result<(), Failure> {
    let cp = machine.capture(SaveMeta {
        created_by: change.actor,
        parent: change.parent,
    });
    let rev = next(meta.rev);
    store
        .write_head(&cp, rev, change.walk.as_ref())
        .map_err(|e| Failure::from_store(&e))?;
    meta.rev = rev;
    meta.last_stop = change.stop.cloned();
    meta.undo = change.walk;
    meta.debugger = machine.debugger_config().clone();
    store
        .write_meta(meta)
        .map_err(|e| Failure::from_store(&e))?;
    Ok(())
}
