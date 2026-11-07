//! Never-written memory in a predicate means "not yet" (design §8 ruling),
//! and the errors a predicate can still raise.

use super::*;
use gbcli_engine::cond::{EvalWarning, StopTrigger};

#[test]
fn a_predicate_on_unwritten_memory_waits_for_the_write_from_boot() {
    // At $0100 wFrame has never been written: Main writes it, then Work
    // counts it up. The predicate waits through both.
    let mut m = machine(V1, Some(V1_SYM));
    let outcome = check(&mut m, "u16[wFrame] >= 60", &opts(), 5_000, Cadence::Writes);
    assert_eq!(outcome.stop.pc, 0x016C);
    assert!(outcome.warnings.is_empty(), "{:?}", outcome.warnings);
}

#[test]
fn a_budget_end_with_an_unwritten_read_warns_naming_the_byte() {
    let mut m = initialized_v1();
    // The whole predicate waits, though its second part holds.
    let text = "[wNever] == 0 || u16[wFrame] >= 3";
    let Outcome { stop, warnings } =
        until(&mut m, text, &RunOptions::new(Budget::instructions(500)));
    assert_eq!(
        stop.reason,
        StopReason::BudgetExhausted {
            kind: BudgetKind::Instructions
        }
    );
    let [EvalWarning::NotYet { trigger, read }] = &warnings[..] else {
        panic!("{warnings:?}")
    };
    assert_eq!(*trigger, StopTrigger::Until);
    assert_eq!(read.symbol.as_deref(), Some("wNever"));
    assert_eq!(read.addr, Some(0xC003));
    let text = warnings[0].to_string();
    assert!(
        text.contains("--until") && text.contains("wNever") && text.contains("--allow-uninit"),
        "{text}"
    );
}

#[test]
fn allow_uninit_reads_the_power_on_value() {
    let mut m = machine(V1, Some(V1_SYM));
    let Outcome { stop, warnings } = until(&mut m, "[wNever] == [wNever]", &lenient());
    assert!(matches!(stop.reason, StopReason::Predicate { .. }));
    assert_eq!(stop.budget.instructions, 0);
    assert!(matches!(&warnings[..], [EvalWarning::UninitializedRead(_)]));
}

#[test]
fn a_predicate_that_fails_mid_run_stops_with_its_true_reason() {
    let mut m = initialized_v1();
    // Short-circuited until wFrame reaches 60; then it divides by zero.
    let text = "[wFrame] == 60 && 1 / ([wFrame] - 60) == 0";
    let err = m.run_until(text, &opts()).unwrap_err();
    let DebugError::Until { error, stop } = &err else {
        panic!("{err:?}")
    };
    assert_eq!(*error, gbcli_engine::cond::EvalError::DivideByZero);
    assert_eq!(
        stop.reason,
        StopReason::PredicateError {
            text: text.into(),
            cadence: Cadence::Writes,
        }
    );
    assert_eq!(stop.pc, 0x016C);
    assert_eq!(stop.budget.instructions, 537);
    assert_eq!(
        m.core().registers().pc,
        0x016C,
        "the record is where the machine is"
    );
}

#[test]
fn a_predicate_that_fails_before_the_first_instruction_runs_nothing() {
    let mut m = machine(V1, Some(V1_SYM));
    let ticks = m.core().now().0;
    let err = m.run_until("new == 1", &opts()).unwrap_err();
    let DebugError::Until { stop, .. } = &err else {
        panic!("{err:?}")
    };
    assert!(err.to_string().starts_with("until: "), "{err}");
    assert!(
        matches!(stop.reason, StopReason::PredicateError { .. }),
        "{stop:?}"
    );
    assert_eq!(stop.budget.instructions, 0);
    assert_eq!(m.core().now().0, ticks);
}

#[test]
fn a_predicate_that_does_not_compile_runs_nothing() {
    let mut m = machine(V1, Some(V1_SYM));
    let ticks = m.core().now().0;
    for text in ["1 +", "[wNope] == 1"] {
        let err = m.run_until(text, &opts()).unwrap_err();
        assert!(
            matches!(err, DebugError::UntilSyntax { .. }),
            "{text}: {err:?}"
        );
        assert!(err.to_string().starts_with("until: "), "{err}");
    }
    assert_eq!(m.core().now().0, ticks);
}
