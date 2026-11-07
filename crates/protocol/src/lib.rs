//! gbcli's one set of request and response types, shared by the JSON Lines
//! server and the CLI client. The shapes are in
//! `project/2026-11-06-design.md` §5, §6 and §10.

mod checkpoint;
mod error;
mod hex;
mod parse;
mod request;
mod response;
mod rev;
mod session;
mod stop;
mod strict;
mod views;

pub use checkpoint::{
    BreakpointSpec, CheckpointHeader, CoreId, DebuggerConfig, LiveSymbol, Location, Patch,
    RenderConfig, Sha256, SymbolRole, TimeMark, WatchAccess, WatchSpec, model_from_name,
    model_name,
};
pub use error::{ErrorBody, ErrorCode};
pub use parse::{RequestError, parse_request};
pub use request::{
    BreakArgs, CheckpointArgs, DeleteArgs, EventsArgs, Op, PruneArgs, ReadArgs, Request,
    RestoreArgs, RmArgs, RunArgs, Start, StepArgs, WatchArgs, WriteArg, WriteArgs, WriteKind,
};
pub use response::{
    BreakpointMade, CheckpointMade, Deleted, ReadValue, Response, Restored, ResultBody, WatchMade,
    WriteMade,
};
pub use rev::Rev;
pub use session::{
    EventLogLine, GapCount, GapLine, SessionFile, SessionRow, StopSummary, UndoWalk,
};
pub use stop::{
    BudgetLimits, BudgetUse, Cadence, FrameShown, StepKind, StopReason, StopRecord, StopTime,
    TimeBase,
};
pub use views::{FlagsView, PpuModeView, PpuView, RegistersView};
