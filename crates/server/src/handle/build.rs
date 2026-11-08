//! `build`, `doctor` and `new` (design §12): the RGBDS loop as ops, answered
//! through `gbcli-build`. None needs a session except a build's `restore`,
//! which the server decides here: the session must exist before any tool
//! runs, its ROM must be the file just built, and the restore is an
//! ordinary `restore` request, logged and guarded by `rev` like any other.
//! A build that restores nothing refuses the fields only a restore uses
//! (`session`, `rev`, `allow_rom_change`), rather than ignore them.

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

/// Answers the ops that need no project bundle, for a client with no
/// `.gbcli/` yet: `doctor`, `new`, `opcode`, and `build` without `restore`.
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
            Op::Build(args) => match build(&self.host, request, args) {
                Ok(built) => built.response(id),
                Err(response) => *response,
            },
            Op::Doctor => doctor(&self.host, id),
            Op::New(args) => new(&self.host, id, args),
            Op::Opcode(args) => super::opcode::opcode(id, args),
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
    /// The RGBDS loop's ops and `opcode`, or `None` for any other.
    pub(super) fn toolchain_op(&mut self, request: &Request) -> Option<Response> {
        let id = request.id.clone();
        Some(match &request.op {
            Op::Build(args) => self.build(request, args),
            Op::Doctor => doctor(&self.host, id),
            Op::New(args) => new(&self.host, id, args),
            Op::Opcode(args) => super::opcode::opcode(id, args),
            _ => return None,
        })
    }

    /// Builds, then restores `args.restore` into the request's session
    /// when the build is good and the session runs the ROM it wrote.
    fn build(&mut self, request: &Request, args: &BuildArgs) -> Response {
        let id = request.id.clone();
        let Some(name) = &args.restore else {
            return match build(&self.host, request, args) {
                Ok(built) => built.response(id),
                Err(response) => *response,
            };
        };
        // The session to restore into is checked before any tool runs, so a
        // refusal leaves the ROM as it was.
        let session = match self.restore_target(request) {
            Ok(session) => session,
            Err(body) => return Response::failed(id, None, *body),
        };
        let built = match build(&self.host, request, args) {
            Ok(built) => built,
            Err(response) => return *response,
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

    /// The session a build's restore goes into: the request's, or the
    /// server's default, and one that exists.
    fn restore_target(&self, request: &Request) -> Result<String, Box<ErrorBody>> {
        let Some(session) = request
            .session
            .clone()
            .or_else(|| self.default_session.clone())
        else {
            return Err(Box::new(ErrorBody::new(
                ErrorCode::BadRequest,
                "restoring a build needs the session to restore into",
                "add --session NAME, or name the session in the request",
            )));
        };
        let store_error = |e| Box::new(convert::store(&e));
        let store = self.bundle.session(&session).map_err(store_error)?;
        store.read_meta().map_err(store_error)?;
        Ok(session)
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
/// response that says so. A request without `restore` that carries a field
/// only a restore uses is refused before any tool runs.
fn build(host: &Host, request: &Request, args: &BuildArgs) -> Result<Built, Box<Response>> {
    let id = request.id.as_str();
    if args.restore.is_none()
        && let Some(field) = restore_only_field(request)
    {
        let body = ErrorBody::new(
            ErrorCode::BadRequest,
            format!("{field} only applies when the build restores a checkpoint (`restore`)"),
            "add --restore NAME (a checkpoint of the session), or drop it",
        );
        return Err(Box::new(Response::failed(id, None, body)));
    }
    let tools = Toolchain::discover(host.path.as_deref(), &host.cwd)
        .map_err(|e| Box::new(Response::failed(id, None, e.body())))?;
    let warnings = tools.warnings();
    let project = host.cwd.join(args.dir.as_deref().unwrap_or("."));
    let mut options = BuildOptions::new(project, host.cwd.clone());
    options.src = args.src.iter().map(PathBuf::from).collect();
    options.include = args.include.iter().map(PathBuf::from).collect();
    options.out = args.out.as_ref().map(PathBuf::from);
    options.target = args.target;
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

/// The first field of `request` that only a build's restore uses, as the
/// wire and the CLI name it.
fn restore_only_field(request: &Request) -> Option<&'static str> {
    if request.session.is_some() {
        Some("`session` (--session)")
    } else if request.rev.is_some() {
        Some("`rev` (--rev)")
    } else if request.allow_rom_change {
        Some("`allow_rom_change` (--allow-rom-change)")
    } else {
        None
    }
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
