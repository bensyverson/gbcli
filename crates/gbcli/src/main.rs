//! The gbcli executable: argv, the environment and stdio, handed to the
//! `gbcli-cli` library, which holds every decision.

use std::io;
use std::process::ExitCode;

fn main() -> ExitCode {
    let env = gbcli_cli::Env::from_process();
    let stdin = io::stdin();
    let stdout = io::stdout();
    let stderr = io::stderr();
    let code = gbcli_cli::run(
        std::env::args_os().skip(1).collect(),
        &env,
        gbcli_cli::Io {
            stdin: &mut stdin.lock(),
            out: &mut stdout.lock(),
            err: &mut stderr.lock(),
        },
    );
    ExitCode::from(code)
}
