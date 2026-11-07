//! gbcli's logic, as a library the server and CLI adapters wrap. The engine
//! never prints; adapters format. See `project/2026-11-06-design.md`.

pub mod addr;
pub mod disasm;
pub mod inspect;

mod checkpoint;
mod checkpoint_error;
pub mod cond;
mod debug;
pub mod equiv;
mod machine;
pub mod mem;
pub mod profile;
mod romcheck;
mod session;
mod sha256;
mod sym;

pub use checkpoint::{Checkpoint, LoadOptions, MAGIC, SaveMeta, VERSION};
pub use checkpoint_error::{CheckpointError, LoadWarning, Moved, MovedRole, SymbolAt, SymbolSide};
pub use debug::{
    CallFrame, DebugError, FrameSink, Outcome, RunOptions, Since, core_code, eval_code, expr_code,
    unapplied_keys,
};
pub use machine::{Backend, Machine, SetupError};
pub use session::{MachineId, Session};
pub use sha256::sha256;
pub use sym::{Area, MappedSymbols, SymError, Symbol, Symbols};
