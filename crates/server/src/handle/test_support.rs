//! Helpers the handler's test modules share.

use gbcli_protocol::{Op, Request, Response, RunArgs};

use super::Handler;
use crate::fixture::Fixture;

pub(super) fn handler(f: &Fixture) -> Handler {
    Handler::new(f.bundle.clone(), Some("demo".into()), Some("tester".into()))
}

pub(super) fn request(op: Op) -> Request {
    Request {
        id: "1".into(),
        session: None,
        actor: None,
        rev: None,
        allow_rom_change: false,
        op,
    }
}

pub(super) fn run_frames(handler: &mut Handler, frames: u64) -> Response {
    handler.handle(&request(Op::Run(RunArgs {
        frames: Some(frames),
        ..RunArgs::default()
    })))
}
