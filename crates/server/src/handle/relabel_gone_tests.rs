//! A breakpoint or watch set by a symbol that a rebuild removed stays in the
//! session, disarmed: it stops nothing, at its old address or anywhere, and
//! every request warns that it is disarmed and how to delete it. A later
//! build that has the symbol again re-arms it where that build puts it (Ben's
//! ruling, 2026-11-09). One set by an address is never disarmed.

use gbcli_protocol::{BreakArgs, DeleteArgs, Op, Response, StopReason, WatchAccess, WatchArgs};

use super::Handler;
use super::test_support::{handler, request, run_frames};
use crate::fixture::{Fixture, V1, V1_SYM, V2, V2_SYM, V3, V3_SYM};

fn break_at(h: &mut Handler, at: &str) {
    let made = h.handle(&request(Op::Break(BreakArgs {
        at: at.into(),
        condition: None,
    })));
    assert!(made.is_ok(), "{:?}", made.error());
}

fn watch_write(h: &mut Handler, at: &str) {
    let made = h.handle(&request(Op::Watch(WatchArgs {
        access: WatchAccess::Write,
        at: at.into(),
        condition: None,
        once: false,
    })));
    assert!(made.is_ok(), "{:?}", made.error());
}

fn stop_of(response: &Response) -> Option<(StopReason, u16)> {
    let stop = response.outcome.as_ref().ok()?.stop.as_ref()?;
    Some((stop.reason.clone(), stop.pc))
}

/// V1's symbols with one more label: the same ROM, a build whose `.sym`
/// names `line` (`00:0170 Gone`) too.
fn v1_sym_with(line: &str) -> Vec<u8> {
    let mut sym = V1_SYM.to_vec();
    sym.extend_from_slice(format!("{line}\n").as_bytes());
    sym
}

/// A head parked at Main ($0150) by breakpoint #1, which no later run hits
/// again.
fn parked_at_main(h: &mut Handler) {
    break_at(h, "$0150");
    let ran = run_frames(h, 10);
    assert_eq!(
        stop_of(&ran).map(|s| s.1),
        Some(0x0150),
        "{:?}",
        ran.error()
    );
}

fn says_disarmed(response: &Response, entry: &str, symbol: &str) -> bool {
    response.warnings.iter().any(|w| {
        w.starts_with(entry)
            && w.contains(symbol)
            && w.contains("disarmed")
            && w.contains("gbcli delete 2")
    })
}

/// `Extra` (V3, $0171) is gone in V2, where $0171 is `inc [hl]` inside Work,
/// run on every pass: a breakpoint still armed there would fire on code the
/// label never named.
#[test]
fn a_breakpoint_whose_label_is_gone_does_not_fire_at_its_stale_address() {
    let f = Fixture::with_rom_sym("gone-stale", V3, V3_SYM);
    let mut h = handler(&f);
    parked_at_main(&mut h);
    break_at(&mut h, "Extra");
    f.rebuild(V2, V2_SYM);

    let ran = run_frames(&mut handler(&f), 30);
    assert!(ran.is_ok(), "{:?}", ran.error());
    assert!(
        says_disarmed(&ran, "breakpoint #2", "Extra"),
        "{:?}",
        ran.warnings
    );
    let stop = stop_of(&ran);
    assert!(
        !matches!(stop, Some((StopReason::Breakpoint { id: 2 }, _))),
        "breakpoint #2 (Extra, gone in V2) fired at a stale address: {stop:?}"
    );
    let meta = f.bundle.session("demo").unwrap().read_meta().unwrap();
    assert_eq!(meta.debugger.breakpoints.len(), 2, "kept, not deleted");
}

#[test]
fn a_disarmed_breakpoint_warns_on_every_request_until_deleted() {
    let f = Fixture::with_rom_sym("gone-warns", V3, V3_SYM);
    let mut h = handler(&f);
    parked_at_main(&mut h);
    break_at(&mut h, "Extra");
    f.rebuild(V2, V2_SYM);

    let mut h = handler(&f);
    for _ in 0..2 {
        let ran = run_frames(&mut h, 1);
        assert!(says_disarmed(&ran, "breakpoint #2", "Extra"), "{ran:?}");
    }
    let deleted = h.handle(&request(Op::Delete(DeleteArgs { id: 2 })));
    assert!(deleted.is_ok(), "{:?}", deleted.error());
    let ran = run_frames(&mut h, 1);
    assert!(!ran.warnings.iter().any(|w| w.contains("#2")), "{ran:?}");
}

#[test]
fn a_disarmed_breakpoint_rearms_where_its_label_comes_back() {
    let f = Fixture::with_rom_sym("gone-rearm", V1, &v1_sym_with("00:0170 Gone"));
    let mut h = handler(&f);
    parked_at_main(&mut h);
    break_at(&mut h, "Gone");
    // The same code, without the label: $0170 (Work.done) runs every pass.
    f.rebuild(V1, V1_SYM);
    let ran = run_frames(&mut handler(&f), 10);
    assert!(says_disarmed(&ran, "breakpoint #2", "Gone"), "{ran:?}");
    let stop = stop_of(&ran);
    assert!(
        !matches!(stop, Some((StopReason::Breakpoint { id: 2 }, _))),
        "disarmed, yet fired: {stop:?}"
    );

    // A later build names Gone again, at Work ($0163).
    f.rebuild(V1, &v1_sym_with("00:0163 Gone"));
    let ran = run_frames(&mut handler(&f), 10);
    assert!(
        ran.warnings
            .iter()
            .any(|w| w.starts_with("breakpoint #2") && w.contains("armed again")),
        "{:?}",
        ran.warnings
    );
    assert_eq!(
        stop_of(&ran),
        Some((StopReason::Breakpoint { id: 2 }, 0x0163)),
        "{:?}",
        ran.error()
    );
    let meta = f.bundle.session("demo").unwrap().read_meta().unwrap();
    assert_eq!(meta.debugger.breakpoints[1].at.addr, 0x0163);
}

#[test]
fn a_watch_whose_label_is_gone_is_disarmed_and_rearms_when_it_comes_back() {
    // wGone names wFrame's low byte, which Work writes on every pass.
    let f = Fixture::with_rom_sym("gone-watch", V1, &v1_sym_with("00:c000 wGone"));
    let mut h = handler(&f);
    parked_at_main(&mut h);
    watch_write(&mut h, "wGone");
    f.rebuild(V1, V1_SYM);

    let ran = run_frames(&mut handler(&f), 10);
    assert!(says_disarmed(&ran, "watch #2", "wGone"), "{ran:?}");
    let stop = stop_of(&ran);
    assert!(
        !matches!(stop, Some((StopReason::Watchpoint { id: 2, .. }, _))),
        "disarmed, yet fired: {stop:?}"
    );

    f.rebuild(V1, &v1_sym_with("00:c000 wGone"));
    let ran = run_frames(&mut handler(&f), 10);
    let stop = stop_of(&ran);
    assert!(
        matches!(
            stop,
            Some((
                StopReason::Watchpoint {
                    id: 2,
                    addr: 0xC000,
                    ..
                },
                _
            ))
        ),
        "{stop:?} {:?}",
        ran.error()
    );
}

#[test]
fn deleting_an_unknown_id_lists_a_disarmed_entry_as_disarmed() {
    let f = Fixture::with_rom_sym("gone-remedy", V3, V3_SYM);
    let mut h = handler(&f);
    parked_at_main(&mut h);
    break_at(&mut h, "Extra");
    f.rebuild(V2, V2_SYM);

    let refused = handler(&f).handle(&request(Op::Delete(DeleteArgs { id: 9 })));
    let remedy = &refused.error().expect("no #9").remedy;
    assert!(remedy.contains("#2 break Extra (disarmed)"), "{remedy}");
    assert!(
        remedy.contains("#1 break $0150,"),
        "an armed one says nothing: {remedy}"
    );
}
