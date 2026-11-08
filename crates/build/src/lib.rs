//! gbcli's RGBDS loop (design §12, the Build row and `doctor`): find the
//! pinned RGBDS on `PATH`, build a project by convention with every
//! diagnostic turned into a file, a line and a message, check the machine
//! with `doctor`, and scaffold a starter project with `new`.
//!
//! The results are the protocol's types, and every error carries the
//! protocol's [`ErrorBody`](gbcli_protocol::ErrorBody), so the CLI only
//! wires and prints.

mod diagnostics;
mod doctor;
mod images;
mod music;
mod paths;
mod project;
mod scaffold;
mod toolchain;

pub use diagnostics::parse as parse_diagnostics;
pub use doctor::{BOOT_ROM_SIZES, DoctorInput, GIT_LOCAL_ENV_VARS, doctor};
pub use music::{MUSIC_DIR, MUSIC_OUT, song_problem};
pub use project::{BuildError, BuildOptions, TOOL_TIMEOUT, build};
pub use scaffold::{HARDWARE_INC, HUGEDRIVER, NewError, new_project};
pub use toolchain::{
    INSTALL_COMMAND, PinMatch, RGBDS_PIN, ToolFound, ToolProblem, Toolchain, ToolchainError,
    Version, install_remedy, probe, release_url,
};
