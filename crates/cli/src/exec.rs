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
/// does. The handler's defaults are the target's session and actor, for the
/// request lines of `serve` and `batch`.
pub fn handler(target: &Target, env: &Env) -> Result<Handler, Box<ErrorBody>> {
    let start = target.dir.clone().unwrap_or_else(|| env.cwd.clone());
    let bundle = Bundle::open_or_create_in(&start, env.gbcli_dir.as_deref())
        .map_err(|e| Box::new(store(e)))?;
    let source = target.create.as_ref().map(|create| RomSource {
        rom: create.rom.clone(),
        sym: create.sym.clone(),
        model: create.model,
        seed: create.seed,
    });
    match (&target.session, &source) {
        (Some(name), source) => {
            bundle
                .open_session(name, source.as_ref())
                .map_err(|e| Box::new(store(e)))?;
        }
        (None, Some(source)) => {
            return Err(Box::new(ErrorBody::new(
                ErrorCode::NoSession,
                format!(
                    "{} names a ROM but no session; ephemeral runs without a session aren't built yet",
                    source.rom.display()
                ),
                "add --session NAME (or set GBCLI_SESSION) to create a session from it",
            )));
        }
        (None, None) => {}
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
/// parse, the same panic guard. The guard lives in [`serve`], so the request
/// goes through it as one line.
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
