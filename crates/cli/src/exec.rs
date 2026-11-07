//! One request, answered in-process by the server's [`Handler`], then printed.

use std::io::{Cursor, Write};

use gbcli_bundle::{Bundle, RomSource, StoreError};
use gbcli_protocol::{ErrorBody, ErrorCode, Op, Request, Response};
use gbcli_server::{Handler, serve};

use crate::build::Target;
use crate::{Env, Io, render};

pub fn request(target: &Target, op: Op, env: &Env, io: Io) -> u8 {
    let mut handler = match handler(target, env) {
        Ok(handler) => handler,
        Err(body) => return render::error(io.err, &body),
    };
    let response = answer(&mut handler, &target.request(op));
    render::response(&response, target.json, io.out, io.err)
}

/// The project's bundle and a handler over it, with the target's session
/// opened first, and created when a ROM names a new one, as `gbcli serve`
/// does. Only an invocation that can create a session (a ROM and a session
/// name) creates `.gbcli/`; any other finds one or fails naming where it
/// looked. `--dir` beats `GBCLI_DIR`: the flag is the more explicit. The
/// handler's defaults are the target's session and actor, for the request
/// lines of `serve` and `batch`.
pub fn handler(target: &Target, env: &Env) -> Result<Handler, Box<ErrorBody>> {
    let source = target.create.as_ref().map(|create| RomSource {
        rom: create.rom.clone(),
        sym: create.sym.clone(),
        model: create.model,
        seed: create.seed,
    });
    if let (None, Some(source)) = (&target.session, &source) {
        return Err(Box::new(ErrorBody::new(
            ErrorCode::NoSession,
            format!(
                "{} names a ROM but no session; ephemeral runs without a session aren't built yet",
                source.rom.display()
            ),
            "add --session NAME (or set GBCLI_SESSION) to create a session from it",
        )));
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
    .map_err(|e| Box::new(store(e)))?;
    if let Some(name) = &target.session {
        bundle
            .open_session(name, source.as_ref())
            .map_err(|e| Box::new(store(e)))?;
    }
    Ok(Handler::new(
        bundle,
        target.session.clone(),
        target.actor.clone(),
    ))
}

pub fn store(e: StoreError) -> ErrorBody {
    ErrorBody::new(e.code(), e.to_string(), e.remedy())
}

/// Answers `request` exactly as `gbcli serve` answers its line: the same
/// parse, the same panic guard. It goes through as a serialized line on
/// purpose (not `gbcli_server::answer`): the strict parse then proves the
/// CLI never sends what `serve` would refuse, for microseconds of JSON.
pub fn answer(handler: &mut Handler, request: &Request) -> Response {
    let line = serde_json::to_string(request).expect("a request always serializes");
    answer_line(handler, &line)
}

/// Answers one protocol line, as `gbcli serve` would.
pub fn answer_line(handler: &mut Handler, line: &str) -> Response {
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
