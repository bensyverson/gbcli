//! gbcli's session bundle store (design §1): the project's `.gbcli/`
//! bundle, one session per name, crash-consistent heads with history, named
//! checkpoints, the event log and the gap ledger. It holds storage
//! decisions, not emulation ones; the engine owns the machine. Every
//! decision is written against [`ProjectStore`], a key-and-blob store rooted
//! at the project; [`FileStore`] is its filesystem backing.

mod boot;
mod bundle;
mod checkpoints;
mod created;
mod error;
mod events;
mod file_store;
mod history;
mod io;
mod listing;
mod lock;
mod log;
mod memory_store;
mod project_store;
mod prune;
mod session;
mod stamp;
mod store;
#[cfg(test)]
mod store_parity_tests;
#[cfg(test)]
mod test_fixture;

pub use boot::Start;
pub use bundle::{Bundle, next, validate_name};
pub use checkpoints::validate_checkpoint_name;
pub use created::{Opened, color_note};
pub use error::StoreError;
pub use events::EVENTS_ROTATE_AT;
pub use file_store::{FileStore, SystemClock};
pub use history::HISTORY_BACK;
pub use lock::{LOCK_WAIT, SessionLock};
pub use memory_store::{MemoryStore, RelativeRoot};
pub use project_store::{
    ChangeToken, Clock, Entry, Guard, Key, Kind, LockError, ProjectStore, PutAllError,
};
pub use prune::{PruneReport, parse_duration};
pub use session::RomSource;
pub use stamp::FilesStamp;
pub use store::SessionStore;
