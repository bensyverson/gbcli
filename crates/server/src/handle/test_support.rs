//! Helpers the handler's test modules share.

use gbcli_protocol::{Op, Request, Response, RunArgs};

use super::Handler;
use crate::Watchdog;
use crate::fixture::Fixture;

/// A handler as the CLI builds one: the native watchdog, the host in the
/// project.
pub(super) fn handler(f: &Fixture) -> Handler {
    handler_on(f, "demo")
}

/// [`handler`], with `session` the default.
pub(super) fn handler_on(f: &Fixture, session: &str) -> Handler {
    let watchdog = Watchdog::start().expect("the wall guard's thread");
    Handler::new(f.host(), f.bundle.clone(), Box::new(watchdog))
        .with_defaults(Some(session.into()), Some("tester".into()))
}

pub(super) fn request(op: Op) -> Request {
    Request {
        id: "1".into(),
        session: None,
        actor: None,
        rev: None,
        allow_rom_change: false,
        wall_guard_s: None,
        op,
    }
}

pub(super) fn run_frames(handler: &mut Handler, frames: u64) -> Response {
    handler.handle(&request(Op::Run(RunArgs {
        frames: Some(frames),
        ..RunArgs::default()
    })))
}
