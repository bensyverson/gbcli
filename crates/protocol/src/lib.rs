//! gbcli's one set of request and response types, shared by the JSON Lines
//! server and the CLI client. Empty until stage 3; the shapes are
//! in `project/2026-11-06-design.md` §5 and §6.

mod checkpoint;

pub use checkpoint::{
    BreakpointSpec, CheckpointHeader, CoreId, DebuggerConfig, LiveSymbol, Location, Patch,
    RenderConfig, Sha256, SymbolRole, TimeMark, WatchAccess, WatchSpec, model_name,
};

mod stop;

pub use stop::{BudgetUse, FrameShown, StepKind, StopReason, StopRecord, StopTime, TimeBase};
