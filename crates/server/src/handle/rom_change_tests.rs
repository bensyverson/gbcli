//! `allow_rom_change` on the request (design §6, ruling of leaf fT85Ss):
//! every op that loads a stored state honors it, so every `rom_changed`
//! remedy's "pass --allow-rom-change" is true.

use gbcli_protocol::{ErrorCode, Op, Request, Response, RestoreArgs};

use super::test_support::{handler, request, run_frames};
use crate::fixture::{Fixture, V2, V2_SYM};

fn forced(op: Op) -> Request {
    let mut r = request(op);
    r.allow_rom_change = true;
    r
}

fn code(response: &Response) -> Option<ErrorCode> {
    response.outcome.as_ref().err().map(|e| e.code)
}

/// A session two heads in, then rebuilt so live symbols move.
fn rebuilt(name: &str) -> Fixture {
    let f = Fixture::with_sym(name);
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 1).is_ok());
    assert!(run_frames(&mut h, 1).is_ok());
    f.rebuild(V2, V2_SYM);
    f
}

#[test]
fn undo_crosses_a_rebuild_only_with_the_flag() {
    let f = rebuilt("undo-cross");
    let refused = handler(&f).handle(&request(Op::Undo));
    assert_eq!(code(&refused), Some(ErrorCode::RomChanged), "{refused:?}");
    let undone = handler(&f).handle(&forced(Op::Undo));
    assert!(undone.is_ok(), "{:?}", undone.error());
    assert!(!undone.warnings.is_empty(), "a forced load warns");
}

#[test]
fn show_loads_the_head_over_a_rebuild_with_the_flag_and_warns() {
    let f = rebuilt("show-cross");
    let refused = handler(&f).handle(&request(Op::Show));
    assert_eq!(code(&refused), Some(ErrorCode::RomChanged), "{refused:?}");
    let shown = handler(&f).handle(&forced(Op::Show));
    assert!(shown.is_ok(), "{:?}", shown.error());
    assert!(!shown.warnings.is_empty(), "a forced load warns");
}

#[test]
fn restore_and_run_take_the_flag_from_the_request() {
    let f = Fixture::with_sym("restore-cross");
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 1).is_ok());
    let made = h.handle(&request(Op::Checkpoint(gbcli_protocol::CheckpointArgs {
        name: "a".into(),
    })));
    assert!(made.is_ok());
    f.rebuild(V2, V2_SYM);
    let restore = || Op::Restore(RestoreArgs { name: "a".into() });
    let refused = handler(&f).handle(&request(restore()));
    assert_eq!(code(&refused), Some(ErrorCode::RomChanged), "{refused:?}");
    assert!(handler(&f).handle(&forced(restore())).is_ok());
}

#[test]
fn the_event_line_records_a_forced_load() {
    let f = rebuilt("log-cross");
    assert!(handler(&f).handle(&forced(Op::Show)).is_ok());
    assert!(
        handler(&f)
            .handle(&request(Op::Events(Default::default())))
            .is_ok()
    );
    let rows = f.bundle.session("demo").unwrap().events(None).unwrap();
    let shown = rows.iter().find(|r| r.op == Op::Show).expect("show logged");
    assert!(shown.allow_rom_change, "{shown:?}");
    let events = rows.last().unwrap();
    assert!(!events.allow_rom_change);
}
