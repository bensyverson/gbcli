//! `run --expect`: conditions that must hold at the stop. V1 writes wSpeed
//! (0) at startup and never writes wNever.

use super::*;
use crate::fixture::Fixture;
use crate::handle::test_support::{handler, request};
use gbcli_protocol::{ExpectValue, Op, Response, Rev};

fn run_expecting(h: &mut Handler, expect: &[&str], args: RunArgs) -> Response {
    h.handle(&request(Op::Run(RunArgs {
        frames: Some(2),
        expect: expect.iter().map(|e| e.to_string()).collect(),
        ..args
    })))
}

fn verdicts(rows: &[ExpectValue]) -> Vec<(&str, Option<i64>, bool)> {
    rows.iter()
        .map(|e| (e.expr.as_str(), e.value, e.ok))
        .collect()
}

#[test]
fn expectations_that_hold_succeed_with_a_row_each_beside_the_reads() {
    let f = Fixture::with_sym("expect-holds");
    let response = run_expecting(
        &mut handler(&f),
        &["[wSpeed] == 0", "u16[wFrame]"],
        RunArgs {
            read: vec!["[wSpeed]".into()],
            ..RunArgs::default()
        },
    );
    let result = response.outcome.expect("every expectation held");
    let rows = result.expect.expect("expect rows");
    assert_eq!(rows[0].expr, "[wSpeed] == 0");
    assert_eq!((rows[0].value, rows[0].ok), (Some(1), true));
    assert!(rows[1].ok && rows[1].value.unwrap() > 0, "{rows:?}");
    assert_eq!(result.reads.expect("reads").len(), 1);
}

#[test]
fn a_false_expectation_is_predicate_false_with_every_row_and_the_run_saved() {
    let f = Fixture::with_sym("expect-false");
    let mut h = handler(&f);
    let response = run_expecting(
        &mut h,
        &["[wSpeed] == 0", "[wSpeed] == 1"],
        RunArgs::default(),
    );
    assert_eq!(response.rev, Some(Rev(1)), "the run moved the head");
    let error = response.outcome.expect_err("one expectation is false");
    assert_eq!(error.code, ErrorCode::PredicateFalse);
    assert_eq!(error.exit, 1);
    assert!(
        error.message.contains("`[wSpeed] == 1`"),
        "{}",
        error.message
    );
    assert!(error.message.contains("0"), "{}", error.message);
    let result = error.result.expect("the run's result rides on the error");
    assert_eq!(
        verdicts(&result.expect.expect("expect rows")),
        vec![
            ("[wSpeed] == 0", Some(1), true),
            ("[wSpeed] == 1", Some(0), false)
        ]
    );
    let stop = result.stop.expect("the stop is in the result");
    assert_eq!(stop.budget.frames, 2);
    // The next run starts from where this one stopped.
    let next = run_expecting(&mut h, &[], RunArgs::default());
    assert_eq!(next.outcome.expect("ran").stop.unwrap().time.frame, 4);
}

#[test]
fn a_false_expectation_saves_no_checkpoint() {
    let f = Fixture::with_sym("expect-checkpoint");
    let response = run_expecting(
        &mut handler(&f),
        &["[wSpeed] == 1"],
        RunArgs {
            checkpoint: Some("after".into()),
            ..RunArgs::default()
        },
    );
    let error = response.outcome.expect_err("false");
    assert!(
        error.message.contains("`after` was not saved"),
        "{}",
        error.message
    );
    let store = f.bundle.session("demo").unwrap();
    assert!(store.checkpoint("after").is_err(), "no checkpoint");
}

#[test]
fn a_never_written_byte_fails_the_expectation_with_its_own_code_unless_allowed() {
    let f = Fixture::with_sym("expect-uninit");
    let mut h = handler(&f);
    let error = run_expecting(&mut h, &["[wNever] == 0"], RunArgs::default())
        .outcome
        .expect_err("can't be evaluated");
    assert_eq!(error.code, ErrorCode::UninitializedRead);
    let rows = error.result.expect("result").expect.expect("rows");
    assert!(!rows[0].ok && rows[0].value.is_none());
    assert_eq!(
        rows[0].error.as_ref().map(|e| e.code),
        Some(ErrorCode::UninitializedRead)
    );

    // As --read: --allow-uninit reads the power-on value, with a warning.
    let allowed = run_expecting(
        &mut h,
        &["[wNever] == [wNever]"],
        RunArgs {
            allow_uninit: true,
            ..RunArgs::default()
        },
    );
    assert!(!allowed.warnings.is_empty());
    assert!(allowed.outcome.expect("allowed").expect.unwrap()[0].ok);
}

/// A bare RAM symbol is its address, so as an expectation it always held
/// (review of 634c8d0); refused as `--until` refuses one compared.
#[test]
fn a_bare_ram_symbol_expected_is_refused_suggesting_a_read() {
    let f = Fixture::with_sym("expect-bare-symbol");
    let mut h = handler(&f);
    let response = run_expecting(&mut h, &["wSpeed"], RunArgs::default());
    let error = response.outcome.expect_err("wSpeed is an address");
    assert_eq!(error.code, ErrorCode::BadExpression, "{}", error.message);
    assert!(error.message.contains("address"), "{}", error.message);
    assert!(
        error.remedy.contains("[wSpeed]") && error.remedy.contains("u16[wSpeed]"),
        "{}",
        error.remedy
    );
    assert!(
        error.stop.is_none() && error.result.is_none(),
        "nothing ran"
    );
    // Read, it is an expectation.
    let ran = run_expecting(&mut h, &["[wSpeed] == 0"], RunArgs::default());
    assert_eq!(ran.outcome.expect("ran").stop.unwrap().time.frame, 2);
}

/// A false expectation fails after the run is saved, so it carries the stop
/// (and the key events past it) as every other post-run failure does.
#[test]
fn a_false_expectation_carries_the_stop_and_the_unapplied_keys() {
    let f = Fixture::with_sym("expect-false-stop");
    let response = run_expecting(
        &mut handler(&f),
        &["[wSpeed] == 1"],
        RunArgs {
            keys: gbcli_protocol::parse_keys("A@9").unwrap(),
            ..RunArgs::default()
        },
    );
    let error = response.outcome.expect_err("false");
    let stop = error.stop.as_ref().expect("the stop on the error");
    assert_eq!(Some(stop), error.result.as_ref().unwrap().stop.as_ref());
    assert_eq!(
        error.keys_unapplied,
        Some(gbcli_protocol::parse_keys("A@9").unwrap())
    );
}

#[test]
fn a_bad_expectation_is_refused_before_the_machine_moves() {
    let f = Fixture::with_sym("expect-bad");
    let mut h = handler(&f);
    for (expect, code) in [
        ("[wSpeed] ==", ErrorCode::BadExpression),
        ("oam[0]", ErrorCode::BadRequest),
    ] {
        let response = run_expecting(&mut h, &[expect], RunArgs::default());
        let error = response.outcome.expect_err(expect);
        assert_eq!(error.code, code, "{expect}: {}", error.message);
        assert!(error.stop.is_none() && error.result.is_none(), "{expect}");
    }
    // Nothing ran, and a good expectation still does.
    let ran = run_expecting(&mut h, &["[wSpeed] == 0"], RunArgs::default());
    assert_eq!(ran.outcome.expect("ran").stop.unwrap().time.frame, 2);
}

/// A RAM symbol as an operand of && || ! is its address too, so it always
/// held (lDEWkI): refused in --expect and --until alike, before anything runs.
#[test]
fn a_bare_ram_symbol_in_a_logical_operator_is_refused() {
    let f = Fixture::with_sym("expect-bare-logical");
    let mut h = handler(&f);
    for expect in ["wSpeed && [wSpeed] == 0", "!wSpeed"] {
        let response = run_expecting(&mut h, &[expect], RunArgs::default());
        let error = response.outcome.expect_err(expect);
        assert_eq!(error.code, ErrorCode::BadExpression, "{}", error.message);
        assert!(error.remedy.contains("[wSpeed]"), "{}", error.remedy);
    }
    let response = h.handle(&request(Op::Run(RunArgs {
        frames: Some(2),
        until: Some("pc == 0 || wSpeed".into()),
        ..RunArgs::default()
    })));
    let error = response.outcome.expect_err("--until");
    assert_eq!(error.code, ErrorCode::BadExpression, "{}", error.message);
    assert!(error.remedy.contains("[wSpeed]"), "{}", error.remedy);
    // Read, the same operand is an expectation.
    let ran = run_expecting(&mut h, &["[wSpeed] == 0 || ![wSpeed]"], RunArgs::default());
    assert!(ran.outcome.is_ok());
}
