//! One request, answered in-process by the server's [`Handler`], then printed.

use std::io::{Cursor, Write};

use gbcli_bundle::{Bundle, RomSource, StoreError};
use gbcli_protocol::{ErrorBody, ErrorCode, Op, Request, Response};
use gbcli_server::{Answer, Handler, Host, Tooling, serve};

use crate::build::Target;
use crate::{Env, Io, render};

pub fn request(target: &Target, op: Op, env: &Env, io: Io) -> u8 {
    let request = target.request(op);
    // `doctor`, `new` and a build that restores nothing need no `.gbcli/`,
    // so they go to the server's bundle-less side and never make one.
    let response = if request.op.needs_bundle() {
        // A build's restore target is the server's to check, before it
        // builds, so the CLI answers it as `serve` does.
        let opens = !matches!(request.op, Op::Build(_));
        let mut handler = match open_handler(target, env, opens) {
            Ok(handler) => handler,
            Err(body) => return render::error(io.err, &body),
        };
        answer(&mut handler, &request)
    } else {
        answer(&mut Tooling::new(host(env)), &request)
    };
    render::response(&response, target.json, io.out, io.err)
}

/// Where the server resolves paths and finds RGBDS: the CLI's own.
fn host(env: &Env) -> Host {
    Host {
        cwd: env.cwd.clone(),
        path: env.path.clone(),
    }
}

/// What `serve` and `batch` answer their lines with: the [`Handler`] when a
/// bundle was found (or made) at start, else the bundle-less [`Tooling`], so
/// a client in a fresh project still reaches `version`, `doctor`, `new` and
/// the rest of what needs no session. The choice is made once, at start: a
/// bundle that appears later (from `new`, say, which scaffolds another
/// directory and makes none) is not looked for again.
pub enum Responder {
    Bundled(Box<Handler>),
    Bare(Box<Tooling>),
}

impl Answer for Responder {
    fn answer(&mut self, request: &Request) -> Response {
        match self {
            Responder::Bundled(handler) => handler.answer(request),
            Responder::Bare(tooling) => tooling.answer(request),
        }
    }

    fn after_panic(&mut self) {
        match self {
            Responder::Bundled(handler) => handler.after_panic(),
            Responder::Bare(tooling) => tooling.after_panic(),
        }
    }
}

/// The project's bundle and a handler over it, with the target's session
/// opened first, and created when a ROM names a new one, as `gbcli serve`
/// does. Only an invocation that can create a session (a ROM and a session
/// name) creates `.gbcli/`; any other finds one or, here, falls back to the
/// bare side: the requests that need a bundle are refused one by one, with
/// `no_session` carrying the store's cause. Every other failure (a bad ROM, a missing named session in
/// a bundle that exists) still refuses the start. `--dir` beats
/// `GBCLI_DIR`: the flag is the more explicit. The handler's defaults are
/// the target's session and actor, for the request lines of `serve` and
/// `batch`.
pub fn responder(target: &Target, env: &Env) -> Result<Responder, Box<ErrorBody>> {
    match start_handler(target, env, true) {
        Ok(handler) => Ok(Responder::Bundled(Box::new(handler))),
        Err(Opening::NoBundle(why)) => Ok(Responder::Bare(Box::new(Tooling::without_bundle(
            host(env),
            named_session(*why, target),
        )))),
        Err(Opening::Refused(body)) => Err(body),
    }
}

/// `why` with the session the invocation named, so a refusal says which one
/// it could not open.
fn named_session(why: ErrorBody, target: &Target) -> ErrorBody {
    match &target.session {
        Some(name) => ErrorBody::new(
            why.code,
            format!("{} (for session {name:?})", why.message),
            why.remedy,
        ),
        None => why,
    }
}

/// Why a handler didn't start.
enum Opening {
    /// No `.gbcli/` was found, and this invocation can't create one; the
    /// store's refusal, for a caller that has no bare side to fall back on.
    NoBundle(Box<ErrorBody>),
    Refused(Box<ErrorBody>),
}

/// [`handler`], opening the target's session first only when `opens`.
fn open_handler(target: &Target, env: &Env, opens: bool) -> Result<Handler, Box<ErrorBody>> {
    start_handler(target, env, opens).map_err(|e| match e {
        Opening::Refused(body) | Opening::NoBundle(body) => body,
    })
}

fn start_handler(target: &Target, env: &Env, opens: bool) -> Result<Handler, Opening> {
    let source = target.create.as_ref().map(|create| RomSource {
        rom: create.rom.clone(),
        sym: create.sym.clone(),
        model: create.model,
        seed: create.seed,
        flash: create.flash.clone(),
    });
    if let (None, Some(source)) = (&target.session, &source) {
        return Err(Opening::Refused(Box::new(ErrorBody::new(
            ErrorCode::NoSession,
            format!(
                "{} names a ROM but no session; ephemeral runs without a session aren't built yet",
                source.rom.display()
            ),
            "add --session NAME (or set GBCLI_SESSION) to create a session from it",
        ))));
    }
    let (start, gbcli_dir) = match &target.dir {
        Some(dir) => (dir.clone(), None),
        None => (env.cwd.clone(), env.gbcli_dir.as_deref()),
    };
    let creates = target.session.is_some() && target.create.is_some();
    let bundle = if creates {
        Bundle::open_or_create_in(&start, gbcli_dir)
    } else {
        Bundle::open_in(&start, gbcli_dir)
    }
    .map_err(|e| {
        let no_bundle = matches!(e, StoreError::NoBundle { .. });
        let body = Box::new(store(e));
        if no_bundle {
            Opening::NoBundle(body)
        } else {
            Opening::Refused(body)
        }
    })?;
    if let Some(name) = target.session.as_ref().filter(|_| opens) {
        bundle
            .open_session(name, source.as_ref())
            .map_err(|e| Opening::Refused(Box::new(store(e))))?;
    }
    let mut handler = Handler::new(bundle, target.session.clone(), target.actor.clone());
    handler.set_host(host(env));
    Ok(handler)
}

pub fn store(e: StoreError) -> ErrorBody {
    ErrorBody::new(e.code(), e.to_string(), e.remedy())
}

/// Answers `request` exactly as `gbcli serve` answers its line: the same
/// parse, the same panic guard. It goes through as a serialized line on
/// purpose (not `gbcli_server::answer`): the strict parse then proves the
/// CLI never sends what `serve` would refuse, for microseconds of JSON.
pub fn answer<A: Answer>(handler: &mut A, request: &Request) -> Response {
    let line = serde_json::to_string(request).expect("a request always serializes");
    answer_line(handler, &line)
}

/// Answers one protocol line, as `gbcli serve` would.
pub fn answer_line<A: Answer>(handler: &mut A, line: &str) -> Response {
    let mut out = Vec::new();
    let input = Cursor::new(format!("{line}\n"));
    if let Err(e) = serve(input, &mut out, handler) {
        return Response::failed(
            String::new(),
            None,
            ErrorBody::plain(ErrorCode::Internal, format!("answering in-process: {e}")),
        );
    }
    match serde_json::from_slice(&out) {
        Ok(response) => response,
        Err(e) => {
            let mut text = String::from_utf8_lossy(&out).into_owned();
            text.truncate(200);
            Response::failed(
                String::new(),
                None,
                ErrorBody::plain(
                    ErrorCode::Internal,
                    format!("the in-process answer didn't parse ({e}): {text}"),
                ),
            )
        }
    }
}

/// Writes, ignoring a closed stream: the exit code still tells the story.
pub fn emit(w: &mut dyn Write, text: &str) {
    let _ = w.write_all(text.as_bytes());
}
