//! gbcli's session bundle store (design §1): the project's `.gbcli/`
//! directory, one session directory per name, atomic heads with history,
//! named checkpoints, the event log and the gap ledger. It holds storage
//! decisions, not emulation ones; the engine owns the machine.

mod bundle;
mod error;
mod log;
mod store;

pub use bundle::{Bundle, next, now_secs, parse_duration, validate_name};
pub use error::StoreError;
pub use store::{LOCK_WAIT, RomSource, SessionLock, SessionStore, Start, validate_checkpoint_name};
