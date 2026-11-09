//! The session's breakpoints and watches as a caller sees them (C-5, C-6 of
//! the raycaster review): `show` lists them, `break` and `watch` echo their
//! condition, ids are never reused across requests, and the configure verbs
//! resolve against the ROM on disk, so a rebuild doesn't block them.

use gbcli_protocol::{
    Arming, BreakArgs, DeleteArgs, ErrorCode, Op, Response, RunArgs, Start, StopReason,
    WatchAccess, WatchArgs,
};

use super::Handler;
use super::test_support::{handler, request, run_frames};
use crate::fixture::{Fixture, V4, V4_SYM};

fn code(response: &Response) -> Option<ErrorCode> {
    response.outcome.as_ref().err().map(|e| e.code)
}

fn break_at(h: &mut Handler, at: &str, condition: Option<&str>) -> Response {
    h.handle(&request(Op::Break(BreakArgs {
        at: at.into(),
        condition: condition.map(String::from),
    })))
}

fn watch_writes(h: &mut Handler, at: &str, condition: Option<&str>) -> Response {
    h.handle(&request(Op::Watch(WatchArgs {
        access: WatchAccess::Write,
        at: at.into(),
        condition: condition.map(String::from),
        once: false,
    })))
}

fn delete(h: &mut Handler, id: u32) -> Response {
    h.handle(&request(Op::Delete(DeleteArgs { id })))
}

#[test]
fn break_and_watch_echo_their_condition() {
    let f = Fixture::with_sym("echo-condition");
    let mut h = handler(&f);
    let made = break_at(&mut h, "Work.done", Some("[wFrame] == 3"));
    let made = made.outcome.unwrap().breakpoint.unwrap();
    assert_eq!(made.condition.as_deref(), Some("[wFrame] == 3"));
    let plain = break_at(&mut h, "Work", None).outcome.unwrap();
    assert_eq!(plain.breakpoint.unwrap().condition, None);

    let made = watch_writes(&mut h, "wFrame", Some("new > 2"));
    let made = made.outcome.unwrap().watch.unwrap();
    assert_eq!(made.condition.as_deref(), Some("new > 2"));
}

#[test]
fn show_lists_every_breakpoint_and_watch_with_its_condition() {
    let f = Fixture::with_sym("show-debugger");
    let mut h = handler(&f);
    let none = h.handle(&request(Op::Show)).outcome.unwrap();
    let none = none.debugger.expect("show always carries the debugger");
    assert!(none.breakpoints.is_empty() && none.watches.is_empty());

    assert!(break_at(&mut h, "Work.done", Some("[wFrame] == 3")).is_ok());
    assert!(watch_writes(&mut h, "wFrame", None).is_ok());
    let shown = h.handle(&request(Op::Show)).outcome.unwrap();
    assert!(shown.registers.is_some(), "the rest of show stands");
    let debugger = shown.debugger.expect("debugger");
    let b = &debugger.breakpoints[0];
    assert_eq!(b.id, 1);
    assert_eq!(b.at.addr, 0x0170);
    assert_eq!(b.at.symbol.as_deref(), Some("Work.done"));
    assert_eq!(b.condition.as_deref(), Some("[wFrame] == 3"));
    assert_eq!(b.arming, Arming::Armed);
    let w = &debugger.watches[0];
    assert_eq!((w.id, w.at.addr, w.access), (2, 0xC000, WatchAccess::Write));
}

/// Each request on a fresh handler, so the counter must come from the
/// session file, not the warm machine.
#[test]
fn deleting_the_highest_id_then_setting_another_gives_a_new_id() {
    let f = Fixture::with_sym("ids-kept");
    assert!(break_at(&mut handler(&f), "Work", None).is_ok());
    let second = break_at(&mut handler(&f), "Work.done", None);
    assert_eq!(second.outcome.unwrap().breakpoint.unwrap().id, 2);
    assert!(delete(&mut handler(&f), 2).is_ok());
    let third = watch_writes(&mut handler(&f), "wFrame", None);
    assert_eq!(third.outcome.unwrap().watch.unwrap().id, 3);
}

/// A session file written before the id counter existed has no `next_id`:
/// it loads, and its next id is one past the highest it holds.
#[test]
fn a_session_file_from_before_the_id_counter_loads_and_continues_past_its_ids() {
    let f = Fixture::with_sym("ids-old");
    assert!(break_at(&mut handler(&f), "Work", None).is_ok());
    assert!(break_at(&mut handler(&f), "Work.done", None).is_ok());
    let store = f.bundle.session("demo").unwrap();
    let path = store.dir().join("session.json");
    let mut json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let removed = json["debugger"].as_object_mut().unwrap().remove("next_id");
    assert!(
        removed.is_some(),
        "the counter is in the session file: {json}"
    );
    std::fs::write(&path, serde_json::to_string_pretty(&json).unwrap()).unwrap();

    let shown = handler(&f).handle(&request(Op::Show));
    assert!(shown.is_ok(), "{:?}", shown.error());
    let third = break_at(&mut handler(&f), "Main", None);
    assert_eq!(third.outcome.unwrap().breakpoint.unwrap().id, 3);
}

/// v1 run for a frame, then rebuilt as v4, whose RAM symbols all moved up a
/// byte: the head can't load, but the configure verbs don't need it.
fn rebuilt(name: &str) -> Fixture {
    let f = Fixture::with_sym(name);
    let mut h = handler(&f);
    assert!(break_at(&mut h, "Work", Some("[wFrame] == 200")).is_ok());
    assert!(run_frames(&mut h, 1).is_ok());
    f.rebuild(V4, V4_SYM);
    f
}

#[test]
fn after_a_rebuild_break_watch_and_delete_work_without_from_boot() {
    let f = rebuilt("config-rebuilt");
    let refused = run_frames(&mut handler(&f), 1);
    assert_eq!(code(&refused), Some(ErrorCode::RomChanged), "{refused:?}");

    let made = watch_writes(&mut handler(&f), "wNever", None);
    let made = made.outcome.expect("watch after a rebuild").watch.unwrap();
    assert_eq!(made.at.addr, 0xC004, "resolved in the ROM on disk");
    let made = break_at(&mut handler(&f), "Work.done", Some("[wSpeed] == 1"));
    assert!(made.is_ok(), "{:?}", made.error());
    let deleted = delete(&mut handler(&f), 1);
    assert!(deleted.is_ok(), "{:?}", deleted.error());

    // The restart still keeps what was set, at the new build's addresses.
    let restarted = handler(&f).handle(&request(Op::Run(RunArgs {
        from: Some(Start::Boot),
        frames: Some(1),
        ..RunArgs::default()
    })));
    assert!(restarted.is_ok(), "{:?}", restarted.error());
    let meta = f.bundle.session("demo").unwrap().read_meta().unwrap();
    let ids: Vec<u32> = meta.debugger.breakpoints.iter().map(|b| b.id).collect();
    assert_eq!(ids, [3]);
    assert_eq!(meta.debugger.watches[0].at.addr, 0xC004);
}

#[test]
fn a_rom_changed_refusal_carries_every_moved_symbol() {
    let f = rebuilt("moved-listed");
    let refused = run_frames(&mut handler(&f), 1);
    let error = refused.outcome.expect_err("rom_changed");
    assert_eq!(error.code, ErrorCode::RomChanged);
    let names: Vec<&str> = error.moved.iter().map(|m| m.name.as_str()).collect();
    assert!(names.contains(&"wFrame"), "{names:?}");
}

/// A configure verb builds a machine over the ROM on disk, not the head; it
/// mustn't leave that machine warm as if it were the head.
#[test]
fn a_configure_verb_leaves_the_warm_head_as_it_was() {
    let f = Fixture::with_sym("config-warm");
    let mut h = handler(&f);
    let first = run_frames(&mut h, 5).outcome.unwrap().stop.unwrap();
    // $0001 never runs, so this breakpoint stops nothing.
    assert!(break_at(&mut h, "$0001", None).is_ok());
    let shown = h.handle(&request(Op::Show)).outcome.unwrap();
    assert_eq!(shown.registers.unwrap().pc, first.pc, "show is the head");
    let next = run_frames(&mut h, 1).outcome.unwrap().stop.unwrap();
    assert!(
        matches!(next.reason, StopReason::BudgetExhausted { .. }),
        "{next:?}"
    );
    assert_eq!(next.time.frame, first.time.frame + 1, "{next:?}");
}
