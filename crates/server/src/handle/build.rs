//! `build`, `doctor` and `new` (design §12): the RGBDS loop as ops, answered
//! through `gbcli-build`. None needs a session except a build's `restore`,
//! which the server decides here: the session's ROM must be the file just
//! built, and the restore is an ordinary `restore` request, logged and
//! guarded by `rev` like any other.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use gbcli_build::{BuildOptions, DoctorInput, Toolchain};
use gbcli_protocol::{
    BuildArgs, ErrorBody, ErrorCode, NewArgs, Op, Request, Response, RestoreArgs, ResultBody,
};

use super::Handler;
use crate::convert;
use crate::serve::Answer;

/// The machine around the server: where relative paths start, and the
/// `PATH` RGBDS is found on.
#[derive(Clone, Debug)]
pub struct Host {
    pub cwd: PathBuf,
    pub path: Option<OsString>,
}

impl Host {
    /// This process's working directory and `PATH`.
    pub fn from_process() -> Host {
        Host {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            path: std::env::var_os("PATH"),
        }
    }
}

/// Answers the RGBDS loop's ops without a project bundle, for a client
/// with no `.gbcli/` yet: `doctor`, `new`, and `build` without `restore`.
/// Anything else needs a session, and so a [`Handler`].
pub struct Tooling {
    host: Host,
}

impl Tooling {
    pub fn new(host: Host) -> Tooling {
        Tooling { host }
    }
}

impl Answer for Tooling {
    fn answer(&mut self, request: &Request) -> Response {
        let id = request.id.clone();
        match &request.op {
            op if op.needs_bundle() => Response::failed(
                id,
                None,
                ErrorBody::new(
                    ErrorCode::NoSession,
                    format!(
                        "{} needs a session, and there is no project bundle here",
                        op_name(op)
                    ),
                    "start one with `gbcli run game.gb --session NAME`",
                ),
            ),
            Op::Build(args) => match build(&self.host, &id, args) {
                Ok(built) => built.response(id),
                Err(response) => *response,
            },
            Op::Doctor => doctor(&self.host, id),
            Op::New(args) => new(&self.host, id, args),
            op => unreachable!("{} needs a bundle", op.name()),
        }
    }

    fn after_panic(&mut self) {}
}

fn op_name(op: &Op) -> String {
    match op {
        Op::Build(_) => "a build's restore".into(),
        op => format!("`{}`", op.name()),
    }
}

impl Handler {
    /// The RGBDS loop's ops, or `None` for any other.
    pub(super) fn toolchain_op(&mut self, request: &Request) -> Option<Response> {
        let id = request.id.clone();
        Some(match &request.op {
            Op::Build(args) => self.build(request, args),
            Op::Doctor => doctor(&self.host, id),
            Op::New(args) => new(&self.host, id, args),
            _ => return None,
        })
    }

    /// Builds, then restores `args.restore` into the request's session
    /// when the build is good and the session runs the ROM it wrote.
    fn build(&mut self, request: &Request, args: &BuildArgs) -> Response {
        let id = request.id.clone();
        let built = match build(&self.host, &id, args) {
            Ok(built) => built,
            Err(response) => return *response,
        };
        let Some(name) = &args.restore else {
            return built.response(id);
        };
        let Built {
            mut result,
            warnings,
            rom,
        } = built;
        let refuse = |body: ErrorBody, result: ResultBody, warnings: Vec<String>| {
            let mut body = body;
            // The build still happened: its report rides with the refusal.
            body.result = Some(Box::new(result));
            Response::failed(id.clone(), None, body).with_warnings(warnings)
        };
        let Some(session) = request
            .session
            .clone()
            .or_else(|| self.default_session.clone())
        else {
            let body = ErrorBody::new(
                ErrorCode::BadRequest,
                "restoring a build needs the session to restore into",
                "add --session NAME, or name the session in the request",
            );
            return refuse(body, result, warnings);
        };
        if let Err(body) = self.runs_rom(&session, &rom.path, &rom.shown) {
            return refuse(*body, result, warnings);
        }
        let restore = Request {
            id: id.clone(),
            session: Some(session),
            actor: request.actor.clone(),
            rev: request.rev,
            allow_rom_change: request.allow_rom_change,
            op: Op::Restore(RestoreArgs { name: name.clone() }),
        };
        let restored = self.session_request(id.clone(), &restore);
        let warnings = [warnings, restored.warnings].concat();
        match restored.outcome {
            Ok(r) => {
                result.restored = r.restored;
                Response::ok(id, restored.rev, result).with_warnings(warnings)
            }
            Err(mut body) => {
                body.result = Some(Box::new(result));
                Response::failed(id, restored.rev, body).with_warnings(warnings)
            }
        }
    }

    /// Whether `session` runs the ROM at `rom`: restoring into any other
    /// would load the old program, not the build.
    fn runs_rom(&self, session: &str, rom: &Path, shown: &str) -> Result<(), Box<ErrorBody>> {
        let store_error = |e| Box::new(convert::store(&e));
        let store = self.bundle.session(session).map_err(store_error)?;
        let meta = store.read_meta().map_err(store_error)?;
        let same = match (
            self.bundle.project().join(&meta.rom).canonicalize(),
            rom.canonicalize(),
        ) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        };
        if same {
            return Ok(());
        }
        Err(Box::new(ErrorBody::new(
            ErrorCode::BadRequest,
            format!(
                "session {session} runs {}, not the build's {shown}; restoring would load the \
                 old ROM",
                meta.rom
            ),
            format!(
                "start a session on the build with `gbcli run {shown} --session NAME`, then \
                 checkpoint and restore in that one"
            ),
        )))
    }
}

/// A good build: its result, its warnings, and the ROM it wrote.
struct Built {
    result: ResultBody,
    warnings: Vec<String>,
    rom: Rom,
}

/// The ROM a build wrote: where it is, and as the response shows it.
struct Rom {
    path: PathBuf,
    shown: String,
}

impl Built {
    fn response(self, id: String) -> Response {
        Response::ok(id, None, self.result).with_warnings(self.warnings)
    }
}

/// Builds the project `args` names, as `host` sees it; a failure is the
/// response that says so.
fn build(host: &Host, id: &str, args: &BuildArgs) -> Result<Built, Box<Response>> {
    let tools = Toolchain::discover(host.path.as_deref(), &host.cwd)
        .map_err(|e| Box::new(Response::failed(id, None, e.body())))?;
    let warnings = tools.warnings();
    let project = host.cwd.join(args.dir.as_deref().unwrap_or("."));
    let mut options = BuildOptions::new(project, host.cwd.clone());
    options.src = args.src.iter().map(PathBuf::from).collect();
    options.include = args.include.iter().map(PathBuf::from).collect();
    options.out = args.out.as_ref().map(PathBuf::from);
    let made = gbcli_build::build(&tools, &options).map_err(|e| {
        Box::new(Response::failed(id, None, e.body()).with_warnings(warnings.clone()))
    })?;
    let rom = made.outputs.first().expect("a good build wrote a ROM");
    let rom = Rom {
        path: host.cwd.join(&rom.path),
        shown: rom.path.clone(),
    };
    Ok(Built {
        result: ResultBody {
            build: Some(made),
            ..ResultBody::default()
        },
        warnings,
        rom,
    })
}

fn doctor(host: &Host, id: String) -> Response {
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
        path_var: host.path.as_deref(),
        cwd: &host.cwd,
        project: &host.cwd,
        boot_roms,
    });
    match outcome {
        Ok(made) => Response::ok(
            id,
            None,
            ResultBody {
                doctor: Some(made),
                ..ResultBody::default()
            },
        ),
        Err(body) => Response::failed(id, None, *body),
    }
}

fn new(host: &Host, id: String, args: &NewArgs) -> Response {
    match gbcli_build::new_project(&host.cwd.join(&args.dir), &host.cwd) {
        Ok(made) => Response::ok(
            id,
            None,
            ResultBody {
                created: Some(made),
                ..ResultBody::default()
            },
        ),
        Err(e) => Response::failed(id, None, e.body()),
    }
}
