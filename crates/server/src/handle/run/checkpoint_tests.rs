//! A run's named checkpoint is saved only when the whole request succeeds
//! (design §11), and a failed run still reports the key events it didn't
//! reach.

use super::*;
use crate::fixture::{Fixture, JOYPAD};
use crate::handle::test_support::{handler, request, run_frames};
use gbcli_protocol::{CheckpointArgs, Op, Response, Rev, parse_keys};

/// The frame the checkpoint `name` was saved at, read from its file.
fn saved_frame(f: &Fixture, name: &str) -> u64 {
    let store = f.bundle.session("demo").unwrap();
    store.checkpoint(name).expect("the checkpoint").header.frame
}

fn save(h: &mut Handler, name: &str) {
    let saved = h.handle(&request(Op::Checkpoint(CheckpointArgs {
        name: name.into(),
    })));
    assert!(saved.is_ok(), "{:?}", saved.error());
}

fn assert_not_saved(response: &Response, name: &str) {
    let error = response.error().expect("the request failed");
    assert!(
        error
            .message
            .contains(&format!("checkpoint `{name}` was not saved")),
        "{}",
        error.message
    );
}

#[test]
fn a_run_whose_read_fails_saves_no_checkpoint_and_says_so() {
    let f = Fixture::new("cp-read-fails");
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 1).is_ok());
    save(&mut h, "foo");
    let before = saved_frame(&f, "foo");
    let response = h.handle(&request(Op::Run(RunArgs {
        frames: Some(2),
        read: vec!["[$C003]".into()],
        checkpoint: Some("foo".into()),
        ..RunArgs::default()
    })));
    assert_eq!(
        response.error().map(|e| e.code),
        Some(ErrorCode::UninitializedRead)
    );
    assert_not_saved(&response, "foo");
    assert_eq!(saved_frame(&f, "foo"), before, "foo was replaced");
}

#[test]
fn a_run_that_misses_its_target_saves_no_checkpoint() {
    let f = Fixture::new("cp-unanswered");
    let mut h = handler(&f);
    let response = h.handle(&request(Op::Run(RunArgs {
        until: Some("[$C000] == $1234".into()),
        frames: Some(2),
        checkpoint: Some("hit".into()),
        ..RunArgs::default()
    })));
    assert_eq!(
        response.error().map(|e| e.code),
        Some(ErrorCode::BudgetExhausted)
    );
    assert_not_saved(&response, "hit");
    let store = f.bundle.session("demo").unwrap();
    assert!(store.checkpoint("hit").is_err(), "hit was saved");
}

#[test]
fn a_run_refused_before_it_moves_says_its_checkpoint_was_not_saved() {
    let f = Fixture::new("cp-refused");
    let response = handler(&f).handle(&request(Op::Run(RunArgs {
        frames: Some(1),
        to: Some("NoSuchLabel".into()),
        checkpoint: Some("there".into()),
        ..RunArgs::default()
    })));
    assert_not_saved(&response, "there");
}

#[test]
fn a_successful_run_saves_its_checkpoint_at_the_new_rev() {
    let f = Fixture::new("cp-ok");
    let mut h = handler(&f);
    let response = h.handle(&request(Op::Run(RunArgs {
        frames: Some(2),
        checkpoint: Some("two".into()),
        ..RunArgs::default()
    })));
    let made = response.outcome.expect("ran").checkpoint.expect("saved");
    assert_eq!(made.rev, Rev(1));
    let store = f.bundle.session("demo").unwrap();
    let header = store.checkpoint("two").unwrap().header;
    assert_eq!(header.rev, Some(Rev(1)), "the file records its rev");
}

#[test]
fn a_failed_run_still_returns_the_key_events_it_did_not_reach() {
    let f = Fixture::with_rom("cp-keys", JOYPAD);
    let mut h = handler(&f);
    let response = h.handle(&request(Op::Run(RunArgs {
        frames: Some(2),
        keys: parse_keys("A@0 START@9").unwrap(),
        read: vec!["[$D123]".into()],
        ..RunArgs::default()
    })));
    let error = response.outcome.expect_err("the read fails");
    assert_eq!(error.keys_unapplied, Some(parse_keys("START@9").unwrap()));
}
