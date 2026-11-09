//! From rev-hw2's review of commit 2ac2a03, kept as regression coverage:
//! what the evaluator answers where the core knows only that the PPU mode is
//! 0 or 3 (`Line100Late` on `ppu_mode.gb`), checked expression by expression
//! against both truths.

use crate::common;

use common::machine;
use gbcli_core::{Budget, PpuModeKnown};
use gbcli_engine::cond::{Condition, EvalCtx};
use gbcli_engine::{Machine, RunOptions, Symbols};
use gbcli_protocol::{Location, TimeMark};

const ROM: &[u8] = include_bytes!("../../../sameboy/tests/roms/ppu_mode.gb");
const SYM: &str = include_str!("../../../sameboy/tests/roms/ppu_mode.sym");

fn in_the_window() -> Machine {
    let mut m = machine(ROM, Some(SYM));
    let addr = Symbols::parse(SYM)
        .unwrap()
        .get("Line100Late")
        .unwrap()
        .addr;
    let at = Location {
        addr,
        bank: None,
        symbol: None,
    };
    let id = m.add_breakpoint(at, None).unwrap();
    m.run(&RunOptions::new(Budget::frames(3))).expect("runs");
    m.delete(id);
    assert_eq!(m.core().registers().pc, addr);
    assert_eq!(m.core().ppu().mode, PpuModeKnown::HBlankOrDrawing);
    m
}

fn eval(m: &Machine, text: &str, allow_uninit: bool) -> Result<i64, String> {
    let c = Condition::compile_value(text, m.symbols()).expect(text);
    let ctx = EvalCtx {
        core: m.core(),
        symbols: m.symbols(),
        access: None,
        base: TimeMark { ticks: 0, frame: 0 },
        allow_uninit,
    };
    c.eval(&ctx, &mut Vec::new()).map_err(|e| format!("{e:?}"))
}

/// The value of `text` with `mode` replaced by `v`, on the same machine.
fn with_mode(m: &Machine, text: &str, v: i64) -> Option<i64> {
    eval(m, &text.replace("mode", &v.to_string()), false).ok()
}

/// Every answer must equal the answer under mode 0 AND under mode 3; where
/// those differ the evaluator must fail. Where they agree, failing is
/// "too strict" and recorded separately (not a correctness bug).
#[test]
fn every_answer_agrees_with_both_truths_and_every_unknowable_fails() {
    let m = in_the_window();
    let mut wrong = Vec::new();
    let mut strict = Vec::new();
    for text in [
        "mode == 1",
        "mode != 1",
        "mode == 2",
        "mode != 2",
        "mode == 4",
        "mode != 4",
        "mode == -1",
        "mode == 0x103",
        "1 == mode",
        "mode == 0",
        "mode != 0",
        "mode == 3",
        "mode != 3",
        "mode == 1 || mode == 0",
        "mode == 0 || mode == 1",
        "mode != 1 || mode == 0",
        "mode == 1 && mode == 0",
        "mode == 0 && mode == 1",
        "!(mode == 1)",
        "!(mode != 2)",
        "(mode == 1) == 0",
        "mode == (mode == 1)",
        "mode == mode",
        "mode != mode",
        "mode == ly",
        "mode == ly - 97",
        "mode == ly - 100",
        "mode == 0 || mode == 3",
        "mode != 0 && mode != 3",
        "mode < 4",
        "mode == 1 || ly == 100",
        "ly == 100 && mode != 1",
    ] {
        let got = eval(&m, text, false);
        let (z, t) = (with_mode(&m, text, 0), with_mode(&m, text, 3));
        let truth = if z == t { z } else { None };
        match (&got, truth) {
            (Ok(v), Some(want)) if *v != want => wrong.push(format!("{text}: {v} != {want}")),
            (Ok(v), None) => wrong.push(format!("{text}: answered {v}, unknowable")),
            (Err(_), Some(want)) => strict.push(format!("{text}: fails, answer is {want}")),
            _ => {}
        }
    }
    eprintln!("too strict (answerable but fails): {strict:#?}");
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// `mode == [unwritten]`: the settle path evaluates the other side first.
#[test]
fn a_never_written_read_beside_mode() {
    let m = in_the_window();
    eprintln!("mode == [$DFF0]: {:?}", eval(&m, "mode == [$DFF0]", false));
    eprintln!("[$DFF0] == mode: {:?}", eval(&m, "[$DFF0] == mode", false));
    eprintln!(
        "mode != [$DFF0] allow: {:?}",
        eval(&m, "mode != [$DFF0]", true)
    );
}

const VRAM_MODE3: &[u8] = include_bytes!("../../../gbcli/tests/traps/vram_mode3.gb");
const VRAM_MODE3_SYM: &str = include_str!("../../../gbcli/tests/traps/vram_mode3.sym");

fn ignored_count(w: &[gbcli_engine::cond::EvalWarning]) -> usize {
    w.iter()
        .filter(|w| matches!(w, gbcli_engine::cond::EvalWarning::WritesIgnored(_)))
        .count()
}

/// A failed run's drop is reported once, on its error, and never again on
/// the next run of the same machine (no carry-over, no double count); a
/// watch condition's failure carries it too.
#[test]
fn a_failed_runs_warnings_are_its_own_and_counted_once() {
    let demo = || {
        let mut m = machine(VRAM_MODE3, Some(VRAM_MODE3_SYM));
        m.core_mut().bus_write(0xFF40, 0x91).unwrap();
        m
    };
    let opts = RunOptions::new(Budget::frames(4));
    let mut m = demo();
    let err = m
        .run_until("pc == Idle && 1 / (ly - 20) == 99", &opts)
        .unwrap_err();
    assert_eq!(ignored_count(err.warnings()), 1, "{err:?}");
    let next = m.run(&RunOptions::new(Budget::frames(2))).expect("runs");
    assert_eq!(ignored_count(&next.warnings), 0, "{:?}", next.warnings);

    // A watch on HRAM/WRAM written each frame would do; a watch on the
    // dropped address itself fires at the drop, so its failing condition
    // carries the drop that happened in that very chunk.
    let mut m = demo();
    let at = Location {
        addr: 0x8000,
        bank: None,
        symbol: None,
    };
    m.add_watch(
        gbcli_protocol::WatchAccess::Write,
        at,
        Some("ly == 10 && 1 / (a - a) == 1"),
        false,
    )
    .unwrap();
    let err = m.run(&opts).unwrap_err();
    assert_eq!(ignored_count(err.warnings()), 1, "{err:?}");
}
