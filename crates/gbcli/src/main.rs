//! The gbcli executable: a thin adapter over the engine. Only a placeholder
//! until the CLI and server leaves land.

// Linked now so build-time measurements cover the real dependency graph.
use gbcli_engine as _;
use gbcli_protocol as _;

use std::process::ExitCode;

fn main() -> ExitCode {
    eprintln!(
        "gbcli {} (SameBoy {}): no commands yet",
        env!("CARGO_PKG_VERSION"),
        &sameboy_sys::SAMEBOY_COMMIT[..7],
    );
    ExitCode::FAILURE
}
