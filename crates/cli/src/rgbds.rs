//! `doctor`, `new` and `build`: argv to `gbcli-build`, and a good build's
//! reload sent to the session as a `restore` request, through the same path
//! as every other verb, so the server's rebuilt-ROM check decides it.

use std::path::PathBuf;

use gbcli_build::{BuildOptions, DoctorInput, Toolchain};
use gbcli_bundle::Bundle;
use gbcli_protocol::{ErrorBody, ErrorCode, Op, Response, RestoreArgs, ResultBody};

use crate::argv::Invocation;
use crate::build::{Command, Target};
use crate::exec::{answer, handler, store};
use crate::usage::Usage;
use crate::{Env, Io, render};

/// The response id for a verb answered here rather than by the server.
const ID: &str = "cli";

/// A parsed `build`.
#[derive(Debug)]
pub struct BuildCommand {
    pub options: BuildOptions,
    /// The checkpoint to reload into `target.session`.
    pub restore: Option<String>,
    pub target: Target,
}

pub fn command(inv: &Invocation, env: &Env, target: Target) -> Result<Command, Usage> {
    let project = env
        .cwd
        .join(inv.positionals.first().map_or(".", String::as_str));
    let mut options = BuildOptions::new(project, env.cwd.clone());
    options.src = inv.all("src").map(PathBuf::from).collect();
    options.include = inv.all("include").map(PathBuf::from).collect();
    options.out = inv.one("out").map(PathBuf::from);
    let restore = inv.one("restore").map(str::to_string);
    if restore.is_none() {
        let reload = ["session", "allow-rom-change", "rev", "dir", "as"];
        if let Some(flag) = reload.into_iter().find(|f| inv.has(f)) {
            return Err(Usage::new(
                format!("--{flag} only applies when --restore reloads the build"),
                "add --restore NAME (a checkpoint of the session), or drop it",
            ));
        }
    } else if target.session.is_none() {
        return Err(Usage::new(
            "--restore needs the session to restore into",
            "add --session NAME (or set GBCLI_SESSION), e.g. \
             `gbcli build --session game --restore start`",
        ));
    }
    Ok(Command::Build(Box::new(BuildCommand {
        options,
        restore,
        target,
    })))
}

pub fn doctor(json: bool, env: &Env, io: Io) -> u8 {
    use sameboy_sys::boot_roms as b;
    let boot_roms: &[(&str, &[u8])] = &[
        ("dmg", b::DMG),
        ("mgb", b::MGB),
        ("sgb", b::SGB),
        ("sgb2", b::SGB2),
        ("cgb", b::CGB),
        ("cgb0", b::CGB0),
        ("cgb_fast", b::CGB_FAST),
        ("agb", b::AGB),
    ];
    let outcome = gbcli_build::doctor(&DoctorInput {
        path_var: env.path.as_deref(),
        project: &env.cwd,
        boot_roms,
    });
    let response = match outcome {
        Ok(made) => ok(ResultBody {
            doctor: Some(made),
            ..ResultBody::default()
        }),
        Err(body) => Response::failed(ID, None, *body),
    };
    render::response(&response, json, io.out, io.err)
}

pub fn new(dir: &std::path::Path, json: bool, env: &Env, io: Io) -> u8 {
    let response = match gbcli_build::new_project(dir, &env.cwd) {
        Ok(made) => ok(ResultBody {
            created: Some(made),
            ..ResultBody::default()
        }),
        Err(e) => Response::failed(ID, None, e.body()),
    };
    render::response(&response, json, io.out, io.err)
}

pub fn build(command: BuildCommand, env: &Env, io: Io) -> u8 {
    let json = command.target.json;
    let response = built(command, env);
    render::response(&response, json, io.out, io.err)
}

fn built(command: BuildCommand, env: &Env) -> Response {
    let tools = match Toolchain::discover(env.path.as_deref()) {
        Ok(tools) => tools,
        Err(e) => return Response::failed(ID, None, e.body()),
    };
    let warnings = tools.warnings();
    let made = match gbcli_build::build(&tools, &command.options) {
        Ok(made) => made,
        Err(e) => return Response::failed(ID, None, e.body()).with_warnings(warnings),
    };
    let mut result = ResultBody {
        build: Some(made),
        ..ResultBody::default()
    };
    let Some(name) = command.restore else {
        return ok(result).with_warnings(warnings);
    };
    let rom = result
        .build
        .as_ref()
        .and_then(|b| b.outputs.first())
        .map(|o| o.path.clone());
    let rom = rom.expect("a good build has a ROM");
    let reloaded = reload(&command.target, &name, &rom, env);
    let (rev, outcome, more) = match reloaded {
        Ok(response) => (response.rev, response.outcome, response.warnings),
        Err(body) => (None, Err(*body), Vec::new()),
    };
    let warnings = [warnings, more].concat();
    match outcome {
        Ok(restored) => {
            result.restored = restored.restored;
            Response::ok(ID, rev, result).with_warnings(warnings)
        }
        Err(mut body) => {
            // The build still happened: its report rides with the refusal.
            body.result = Some(Box::new(result));
            Response::failed(ID, rev, body).with_warnings(warnings)
        }
    }
}

/// Restores checkpoint `name` into the target's session, whose ROM must be
/// the one just built (`rom`, as the build printed it): restoring into
/// another ROM would load the old program, not the build.
fn reload(target: &Target, name: &str, rom: &str, env: &Env) -> Result<Response, Box<ErrorBody>> {
    let session = target.session.as_deref().expect("command() requires it");
    let (start, gbcli_dir) = match &target.dir {
        Some(dir) => (dir.clone(), None),
        None => (env.cwd.clone(), env.gbcli_dir.as_deref()),
    };
    let bundle = Bundle::open_in(&start, gbcli_dir).map_err(|e| Box::new(store(e)))?;
    let meta = bundle
        .session(session)
        .and_then(|s| s.read_meta())
        .map_err(|e| Box::new(store(e)))?;
    let same = |a: PathBuf, b: PathBuf| match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if !same(bundle.project().join(&meta.rom), env.cwd.join(rom)) {
        return Err(Box::new(ErrorBody::new(
            ErrorCode::BadRequest,
            format!(
                "session {session} runs {}, not the build's {rom}; restoring would load the \
                 old ROM",
                meta.rom
            ),
            format!(
                "start a session on the build with `gbcli run {rom} --session NAME`, then \
                 checkpoint and restore in that one"
            ),
        )));
    }
    let mut handler = handler(target, env)?;
    let request = target.request(Op::Restore(RestoreArgs { name: name.into() }));
    Ok(answer(&mut handler, &request))
}

fn ok(result: ResultBody) -> Response {
    Response::ok(ID, None, result)
}
