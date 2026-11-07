//! gbcli's one set of request and response types, shared by the JSON Lines
//! server and the CLI client. The shapes are in
//! `project/2026-11-06-design.md` §5, §6 and §10.

mod audio;
mod checkpoint;
mod equiv;
mod error;
mod hex;
mod keys;
mod listing;
mod opcode;
mod parse;
mod profile;
mod raster;
mod request;
mod response;
mod rev;
mod session;
mod shot;
mod song;
mod stop;
mod strict;
mod toolchain;
mod video;
mod views;

pub use audio::{ApuWriteLine, AudioArgs, AudioMade, WavArgs, WavChannels, WavMade, WavTake};
pub use checkpoint::{
    BreakpointSpec, CheckpointHeader, CoreId, DebuggerConfig, LiveSymbol, Location, Patch,
    RenderConfig, Sha256, SymbolRole, TimeMark, WatchAccess, WatchSpec, model_from_name,
    model_name,
};
pub use equiv::{
    ApuWriteMade, AudioCompareName, DifferenceMade, DivergenceMade, EquivArgs, EquivMade,
    EquivSide, RamRangeArg, SideMade,
};
pub use error::{ErrorBody, ErrorCode};
pub use keys::{Button, KEY_GRAMMAR, KeyAction, KeyError, KeyEvent, parse_keys};
pub use listing::{CheckpointRow, SymArgs, SymbolArea, SymbolRow};
pub use opcode::{OpcodeArgs, OpcodeRow};
pub use parse::{RequestError, parse_request};
pub use profile::{
    CpuSpeedName, CycleStats, FrameBudgetMade, FrameBudgetRow, MeasureArgs, MeasureMade,
    PercentStats, ProfileArgs, ProfileMade, Real, RoutineRow, RunMeasure,
};
pub use raster::{RasterLineRow, RasterMade, RasterWriteRow};
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
    EventLogLine, GapCount, GapLine, SESSION_NAME_RULE, SessionFile, SessionRow, StopSummary,
    UndoWalk, is_session_name,
};
pub use shot::{Crop, FilmstripArgs, FilmstripMade, ShotArgs, ShotFrame, ShotMade, ShotReadArgs};
pub use song::{SongFileFormat, SongMade, SongOutput, SongOutputKind, SongProblem};
pub use stop::{
    BudgetLimits, BudgetUse, Cadence, FrameShown, StepKind, StopReason, StopRecord, StopTime,
    TimeBase, vblank_name,
};
pub use toolchain::{
    BuildArgs, BuildMade, CheckMade, CheckName, CheckStatus, Diagnostic, DoctorMade, NewArgs,
    NewMade, OutputKind, OutputMade, Severity, SourceLine, Tool,
};
pub use video::{
    AddressingView, ImageMade, ImageOut, MapAreaView, MapSel, OamView, TileView, VideoData,
    ViewArgs, ViewKind, ViewMade,
};
pub use views::{FlagsView, PpuModeView, PpuView, RegistersView};
