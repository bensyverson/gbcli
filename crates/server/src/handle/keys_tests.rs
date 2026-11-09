//! Key input on a run (design §11 step 3), checked by `joypad.gb`, which logs
//! the held buttons at every vblank. A run with `frames` stops at a vblank
//! before the ROM has logged it, so the next run's first log entry is that
//! vblank: frame 0 of the run, after the events at 0. Entry `start + k` is
//! the vblank that ends frame `k`, after the events at `k`.

use gbcli_protocol::{KeyEvent, Op, ReadArgs, Response, RunArgs, parse_keys};

use super::Handler;
use super::test_support::{handler, request, run_frames};
use crate::fixture::{Fixture, JOYPAD};

const A: i64 = 0x10;
const START: i64 = 0x80;
const RIGHT: i64 = 0x01;

fn read(h: &mut Handler, exprs: &[String]) -> Vec<i64> {
    let response = h.handle(&request(Op::Read(ReadArgs {
        reads: exprs.to_vec(),
        since: None,
        allow_uninit: false,
    })));
    let result = response.outcome.expect("reads");
    result
        .reads
        .unwrap()
        .iter()
        .map(|r| r.value.unwrap())
        .collect()
}

/// The next log index: where the run's first vblank will be logged.
fn log_index(h: &mut Handler) -> i64 {
    read(h, &["[$C000]".to_string()])[0]
}

/// `n` log entries from `from`.
fn log(h: &mut Handler, from: i64, n: i64) -> Vec<i64> {
    let exprs: Vec<String> = (from..from + n)
        .map(|i| format!("[${:04X}]", 0xC100 + (i & 0xFF)))
        .collect();
    read(h, &exprs)
}

fn run_keys(h: &mut Handler, script: &str, frames: u64) -> Response {
    h.handle(&request(Op::Run(RunArgs {
        frames: Some(frames),
        keys: parse_keys(script).unwrap(),
        ..RunArgs::default()
    })))
}

#[test]
fn a_run_presses_and_releases_on_the_stated_frames() {
    let f = Fixture::with_rom("keys-timeline", JOYPAD);
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 2).is_ok(), "the ROM is logging");
    let start = log_index(&mut h);

    let response = run_keys(&mut h, "A@0 -A@2 START@3 -START@4 RIGHT@1 -RIGHT@5", 6);
    assert!(response.is_ok(), "{:?}", response.error());
    assert_eq!(
        log(&mut h, start, 6),
        vec![A, A | RIGHT, RIGHT, START | RIGHT, RIGHT, 0]
    );
}

#[test]
fn held_keys_stay_held_in_the_head_across_runs() {
    let f = Fixture::with_rom("keys-held", JOYPAD);
    let mut h = handler(&f);
    assert!(run_keys(&mut h, "B@0", 1).is_ok());
    let start = log_index(&mut h);
    assert!(run_frames(&mut h, 2).is_ok());
    assert_eq!(log(&mut h, start, 2), vec![0x20, 0x20], "B is still held");

    // A cold load of the head holds it too.
    let mut cold = handler(&f);
    let start = log_index(&mut cold);
    assert!(run_frames(&mut cold, 1).is_ok());
    assert_eq!(log(&mut cold, start, 1), vec![0x20]);
}

#[test]
fn a_key_event_past_the_stop_is_reported_not_dropped() {
    let f = Fixture::with_rom("keys-past", JOYPAD);
    let mut h = handler(&f);
    let response = run_keys(&mut h, "A@0 -A@3 START@9", 3);
    assert!(response.is_ok(), "{:?}", response.error());
    let late: Vec<KeyEvent> = parse_keys("START@9").unwrap();
    // The result lists them; a warning listing them too buried the stop
    // under hundreds of events (leaf emtFvQ).
    assert!(response.warnings.is_empty(), "{:?}", response.warnings);
    let result = response.outcome.unwrap();
    assert_eq!(result.keys_unapplied, Some(late));
    assert_eq!(result.stop.unwrap().budget.frames, 3);
}

#[test]
fn every_event_applied_reports_nothing_unapplied() {
    let f = Fixture::with_rom("keys-all", JOYPAD);
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 1).is_ok());
    let start = log_index(&mut h);
    let response = run_keys(&mut h, "A@0 -A@3", 3);
    assert!(response.warnings.is_empty(), "{:?}", response.warnings);
    assert_eq!(response.outcome.unwrap().keys_unapplied, None);
    // The release at the stop's vblank was applied: the ROM logs it first
    // thing in the next run.
    assert!(run_frames(&mut h, 1).is_ok());
    assert_eq!(log(&mut h, start, 4), vec![A, A, A, 0]);
}
