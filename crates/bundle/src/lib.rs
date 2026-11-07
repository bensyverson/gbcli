//! gbcli's session bundle store (design §1): the project's `.gbcli/`
//! directory, one session directory per name, crash-consistent heads with
//! history, named checkpoints, the event log and the gap ledger. It holds
//! storage decisions, not emulation ones; the engine owns the machine.

mod boot;
mod bundle;
mod checkpoints;
mod error;
mod history;
mod io;
mod lock;
mod log;
mod prune;
mod session;
mod store;
#[cfg(test)]
mod test_fixture;

pub use boot::Start;
pub use bundle::{Bundle, next, now_secs, validate_name};
pub use checkpoints::validate_checkpoint_name;
pub use error::StoreError;
pub use history::HISTORY_BACK;
pub use lock::{LOCK_WAIT, SessionLock};
pub use prune::{PruneReport, parse_duration};
pub use session::RomSource;
pub use store::SessionStore;
