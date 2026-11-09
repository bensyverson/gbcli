//! gbcli's RGBDS loop (design §12, the Build row and `doctor`): find the
//! pinned RGBDS on `PATH`, build a project by convention with every
//! diagnostic turned into a file, a line and a message, check the machine
//! with `doctor`, and scaffold a starter project with `new`.
//!
//! The results are the protocol's types, and every error carries the
//! protocol's [`ErrorBody`](gbcli_protocol::ErrorBody), so the CLI only
//! wires and prints.

mod assets;
mod diagnostics;
mod doctor;
mod font;
mod images;
mod music;
mod paths;
mod pixels;
mod project;
mod scaffold;
mod toolchain;

pub use assets::{AssetsError, AssetsInput, AssetsOptions, assets};
pub use diagnostics::parse as parse_diagnostics;
pub use doctor::{BOOT_ROM_SIZES, DoctorInput, GIT_LOCAL_ENV_VARS, doctor};
pub use font::font;
pub use music::{MUSIC_DIR, MUSIC_OUT, song_problem};
pub use pixels::pixels;
pub use project::{BuildError, BuildOptions, TOOL_TIMEOUT, build, default_rom, default_target_of};
pub use scaffold::{HARDWARE_INC, HUGEDRIVER, NewError, new_project};
pub use toolchain::{
    INSTALL_COMMAND, ImageTool, PinMatch, RGBDS_PIN, ToolFound, ToolProblem, Toolchain,
    ToolchainError, Version, install_remedy, probe, release_url,
};
