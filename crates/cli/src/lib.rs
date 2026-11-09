//! gbcli's command line (design §12): argv → a protocol request, answered
//! in-process by the server's handler, the path `gbcli serve` takes; then
//! rows, or the protocol's result object with `--json`. Help, the primer,
//! recipes and the schema live here too, so all of it is testable without
//! the binary, which only wires argv, the environment and stdio.

mod argv;
mod batch;
mod build;
mod exec;
mod help;
mod render;
mod rgbds;
mod serve_cmd;
mod song;
mod usage;
mod values;
mod verbs;

use std::ffi::OsString;
use std::io::{BufRead, Write};
use std::path::PathBuf;

use build::Command;

/// What the process environment contributes.
#[derive(Clone, Debug, Default)]
pub struct Env {
    /// `GBCLI_SESSION`: the session when `--session` isn't given.
    pub session: Option<String>,
    /// `GBCLI_AS`: who asks, when `--as` isn't given.
    pub actor: Option<String>,
    /// `GBCLI_DIR`: the bundle directory, overriding the search.
    pub gbcli_dir: Option<PathBuf>,
    /// The working directory: relative paths and the `.gbcli/` search start
    /// here. `--dir PATH` moves it to PATH for the invocation.
    pub cwd: PathBuf,
    /// `PATH`: where `build` and `doctor` look for RGBDS.
    pub path: Option<OsString>,
}

impl Env {
    pub fn from_process() -> Env {
        let var = |name| std::env::var(name).ok().filter(|v| !v.is_empty());
        Env {
            session: var("GBCLI_SESSION"),
            actor: var("GBCLI_AS"),
            gbcli_dir: std::env::var_os("GBCLI_DIR")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            path: std::env::var_os("PATH"),
        }
    }

    /// This environment as seen from `dir` (relative to the working
    /// directory): what `--dir` makes of it.
    fn anchored(&self, dir: &str) -> Env {
        Env {
            cwd: self.cwd.join(dir),
            ..self.clone()
        }
    }
}

/// The process's streams.
pub struct Io<'a> {
    pub stdin: &'a mut dyn BufRead,
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
}

/// Runs one invocation (`args` without the program name) and returns the
/// exit code (cli-design ▸ Exit codes).
pub fn run(args: Vec<OsString>, env: &Env, io: Io) -> u8 {
    let parsed = argv::parse(args);
    // `--dir` runs the whole invocation from PATH: the `.gbcli/` search and
    // every relative path it is given, the server's included.
    let anchored = match &parsed {
        Ok(argv::Parsed::Verb(inv)) => inv.one("dir").map(|dir| env.anchored(dir)),
        _ => None,
    };
    let env = anchored.as_ref().unwrap_or(env);
    let command = parsed.and_then(|parsed| match parsed {
        argv::Parsed::Primer => Ok(Command::Primer),
        argv::Parsed::VerbHelp(spec) => Ok(Command::VerbHelp(spec)),
        argv::Parsed::Verb(inv) => build::build(inv, env),
    });
    let command = match command {
        Ok(command) => command,
        Err(usage) => return render::error(io.err, &usage.body()),
    };
    let printed = match command {
        Command::Primer => io.out.write_all(help::primer().as_bytes()),
        Command::VerbHelp(spec) => io.out.write_all(help::verb(spec).as_bytes()),
        Command::Topic(topic) => io.out.write_all(topic.text().as_bytes()),
        Command::Recipes => io.out.write_all(help::RECIPES.as_bytes()),
        Command::Schema => io.out.write_all(help::SCHEMA.as_bytes()),
        Command::Serve(target) => return serve_cmd::serve(&target, env, io),
        Command::Batch { target, file } => return batch::batch(&target, &file, env, io),
        Command::Request { target, op } => return exec::request(&target, *op, env, io),
        Command::Song(song) => return song::song(*song, io),
    };
    match printed {
        Ok(()) => 0,
        // A closed pipe or full disk on stdout: nothing more to say there.
        Err(_) => 5,
    }
}
