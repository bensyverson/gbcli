//! gbcli's one set of request and response types, shared by the JSON Lines
//! server and the CLI client. The shapes are in
//! `project/2026-11-06-design.md` §5, §6 and §10.

mod assets;
mod audio;
mod battery;
mod checkpoint;
mod equiv;
mod error;
mod font;
mod font_author;
mod hex;
mod images;
mod keys;
mod listing;
mod opcode;
mod palettes;
mod parse;
mod pixels;
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
mod version;
mod video;
mod views;

pub use assets::{AssetManifest, AssetsArgs, AssetsMade, ManifestFont, ManifestImage};
pub use audio::{
    ApuWriteLine, AudioArgs, AudioChannel, AudioMade, AudioSummary, RegisterWrites, WavArgs,
    WavChannels, WavMade, WavTake, WriterWrites,
};
pub use battery::{
    BatteryFrom, FlashFileMade, FlashFrom, FlashNoteKind, FlashNoteRow, PowerCycleArgs,
    PowerCycleMade, SavArgs, SavClockState, SavMade,
};
pub use checkpoint::{
    Arming, BreakpointSpec, CheckpointHeader, CoreId, DebuggerConfig, LiveSymbol, Location, Patch,
    RenderConfig, Sha256, SymbolRole, TimeMark, WatchAccess, WatchSpec, model_from_name,
    model_name,
};
pub use equiv::{
    ApuWriteMade, AudioCompareName, ComparedMade, DifferenceMade, DivergenceMade, EquivArgs,
    EquivMade, EquivSide, RamRangeArg, SideMade, SideName,
};
pub use error::{ErrorBody, ErrorCode};
pub use font::{
    FontArgs, FontAsset, FontGlyph, FontMade, FontPreview, FontProblem, FontSample, GlyphOrigin,
};
pub use font_author::{
    FontDiffArgs, FontDiffMade, FontEditArgs, FontExportArgs, FontFile, FontFileKind, FontForm,
    FontImportArgs, FontNewArgs, FontOverlap, FontSaveArgs, FontWritten, GlyphChange,
    GlyphChangeKind,
};
pub use images::{
    Flip, GbcImage, GbcImagesMade, ImageArt, ImageAsset, ImageKind, ImageLimit, ImageProblem,
    ImagesMade, KnownPalette, NotPaletteSlot, ObjectSize, PaletteSlot, ShadeChoice, ShadeRule,
    TileAt, TileEntry,
};
pub use keys::{Button, KEY_GRAMMAR, KeyAction, KeyError, KeyEvent, parse_keys};
pub use listing::{CheckpointRow, SymArgs, SymbolArea, SymbolRow};
pub use opcode::{OpcodeArgs, OpcodeRow};
pub use palettes::{
    FaultAt, GbcPalette, NamedPalette, NamedPalettes, NotRgb555, PaletteFault, Rgb555,
};
pub use parse::{RequestError, parse_request};
pub use pixels::{
    PixelsArgs, PixelsForm, PixelsMade, PixelsOutput, PixelsOutputKind, PixelsProblem,
};
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
    BreakpointMade, CheckpointMade, Deleted, ExpectValue, OamEntryRead, ReadFailed, ReadValue,
    Response, Restored, ResultBody, WatchMade, WriteMade,
};
pub use rev::Rev;
pub use session::{
    CgbFlag, EventLogLine, GapCount, GapLine, MachineView, SESSION_NAME_RULE, SessionFile,
    SessionRow, StopSummary, UndoWalk, is_session_name,
};
pub use shot::{Crop, FilmstripArgs, FilmstripMade, ShotArgs, ShotFrame, ShotMade, ShotReadArgs};
pub use song::{SongFileFormat, SongMade, SongOutput, SongOutputKind, SongProblem};
pub use stop::{
    BudgetLimits, BudgetUse, Cadence, FrameShown, StepKind, StopReason, StopRecord, StopTime,
    TimeBase, vblank_name,
};
pub use toolchain::{
    BankOverflow, BuildArgs, BuildMade, BuildTarget, CheckMade, CheckName, CheckStatus, Diagnostic,
    DoctorMade, NewArgs, NewMade, OutputKind, OutputMade, PlacedSection, Severity, SongBuilt,
    SourceLine, Tool,
};
pub use version::{PROTOCOL_VERSION, VersionMade};
pub use video::{
    AddressingView, CgbColorView, CgbPaletteView, DmgPaletteView, ImageMade, ImageOut, MapAreaView,
    MapSel, MemoryMade, OamFrameView, OamLogMade, OamView, TileView, VideoData, ViewArgs, ViewKind,
    ViewMade,
};
pub use views::{FlagsView, PpuModeView, PpuView, RegistersView};
