//! `--until` predicate stops (design §8, §11), each checked against an
//! independent per-instruction oracle: the same predicate, evaluated after
//! every instruction, must reach the same PC, ticks and instruction count as
//! the cadence-derived stop. `banks` covers memory in switchable banks and
//! aliased cart RAM; `triggers` covers mixed triggers and watch interplay.

#[path = "../common/mod.rs"]
mod common;

mod banks;
mod triggers;

use common::*;
use gbcli_core::{Budget, BudgetKind, Region, Registers, StepUnit};
use gbcli_engine::cond::{Condition, EvalCtx};
use gbcli_engine::{DebugError, Machine, Outcome, RunOptions, Since};
use gbcli_protocol::{Cadence, Location, StopReason, TimeMark, WatchAccess};

/// These tests start from the quick boot, which `machine` steps without a run,
/// so no handoff was recorded: the time base is power-on.
const POWER_ON: TimeMark = TimeMark { ticks: 0, frame: 0 };

fn opts() -> RunOptions<'static> {
    let mut opts = RunOptions::new(Budget::frames(60));
    opts.since = Since::PowerOn;
    opts
}

/// `opts`, reading RAM before the program first writes it: these programs
/// start counters from whatever is there.
fn lenient() -> RunOptions<'static> {
    let mut opts = opts();
    opts.allow_uninit = true;
    opts
}

/// busy.gb with `code` at $0150, where its entry point jumps.
fn program(code: &[u8]) -> Vec<u8> {
    let mut rom = BUSY.to_vec();
    rom[0x150..0x150 + code.len()].copy_from_slice(code);
    rom
}

/// `rom` with a cartridge type and RAM size code in its header.
fn with_cart(mut rom: Vec<u8>, kind: u8, ram_size: u8) -> Vec<u8> {
    rom[0x147] = kind;
    rom[0x149] = ram_size;
    rom[0x14D] = rom[0x134..0x14D]
        .iter()
        .fold(0u8, |x, b| x.wrapping_sub(*b).wrapping_sub(1));
    rom
}

fn at(addr: u16) -> Location {
    Location {
        addr,
        bank: None,
        symbol: None,
    }
}

fn ctx(m: &Machine, allow_uninit: bool) -> EvalCtx<'_> {
    EvalCtx {
        core: m.core(),
        symbols: m.symbols(),
        access: None,
        base: POWER_ON,
        allow_uninit,
    }
}

/// Evaluates the predicate the way the engine does.
fn holds(m: &Machine, text: &str, allow_uninit: bool) -> bool {
    let condition = Condition::compile(text, m.symbols()).expect("the predicate compiles");
    condition
        .holds(&ctx(m, allow_uninit), &mut Vec::new())
        .expect("the predicate evaluates")
}

/// An independent oracle: one instruction at a time, evaluating before each,
/// until the predicate holds at an instruction boundary.
struct Boundary {
    pc: u16,
    ticks: u64,
    instructions: u64,
}

fn oracle(m: &mut Machine, text: &str, max: u64, allow_uninit: bool) -> Boundary {
    let condition = Condition::compile(text, m.symbols()).expect("the predicate compiles");
    for instructions in 0..=max {
        let mut warnings = Vec::new();
        if condition
            .holds(&ctx(m, allow_uninit), &mut warnings)
            .expect("the oracle evaluates")
        {
            return Boundary {
                pc: m.core().registers().pc,
                ticks: m.core().now().0,
                instructions,
            };
        }
        assert!(instructions < max, "the oracle never saw {text:?}");
        m.core_mut().step(StepUnit::Instruction).expect("step");
    }
    unreachable!()
}

fn until(m: &mut Machine, text: &str, opts: &RunOptions<'_>) -> Outcome {
    m.run_until(text, opts)
        .unwrap_or_else(|e| panic!("until {text:?}: {e}"))
}

/// Runs `text` on a clone first (the oracle) and on `m`, then checks that the
/// derived-cadence stop matches the oracle exactly.
fn check(
    m: &mut Machine,
    text: &str,
    opts: &RunOptions<'_>,
    max: u64,
    cadence: Cadence,
) -> Outcome {
    let mut truth = m.try_clone().expect("clone");
    let truth = oracle(&mut truth, text, max, opts.allow_uninit);
    let outcome = until(m, text, opts);
    assert_eq!(
        outcome.stop.reason,
        StopReason::Predicate {
            text: text.into(),
            cadence,
        },
        "{:?}",
        outcome.stop
    );
    assert_eq!(outcome.stop.pc, truth.pc, "{text:?}");
    assert_eq!(outcome.stop.time.ticks, truth.ticks, "{text:?}");
    assert_eq!(
        outcome.stop.budget.instructions, truth.instructions,
        "{text:?}"
    );
    assert!(
        holds(m, text, opts.allow_uninit),
        "the predicate holds at the stop"
    );
    outcome
}

/// v1 after Main's writes, so wFrame is initialized (never-written tracking
/// cleared). From here Work's first pass is six instructions.
fn initialized_v1() -> Machine {
    let mut m = machine(V1, Some(V1_SYM));
    step_to(&mut m, 0x015E);
    m
}

#[test]
fn a_memory_only_predicate_stops_on_the_exact_write() {
    let mut m = initialized_v1();
    let outcome = check(&mut m, "u16[wFrame] >= 60", &opts(), 5_000, Cadence::Writes);
    assert_eq!(outcome.stop.pc, 0x016C);
    assert_eq!(outcome.stop.budget.instructions, 537);
    assert_eq!(
        m.core().region(Region::Wram).unwrap().bytes[0],
        60,
        "the stop is the instruction that made it true"
    );
    assert_eq!(m.core().registers().pc, 0x016C);
}

#[test]
fn a_wide_read_arms_both_bytes() {
    let mut m = initialized_v1();
    // $0100 changes only when the high byte is written, at $016F after the
    // low byte wrapped; arming the low byte alone would never fire.
    let outcome = check(
        &mut m,
        "u16[wFrame] >= $0100",
        &opts(),
        10_000,
        Cadence::Writes,
    );
    assert_eq!(outcome.stop.pc, 0x0170);
    let wram = m.core().region(Region::Wram).unwrap().bytes;
    assert_eq!(u16::from_le_bytes([wram[0], wram[1]]), 0x0100);
}

#[test]
fn a_predicate_that_never_holds_exhausts_the_budget() {
    let mut m = initialized_v1();
    let opts = RunOptions::new(Budget::instructions(2_000));
    let stop = until(&mut m, "u16[wFrame] >= 1000", &opts).stop;
    assert_eq!(
        stop.reason,
        StopReason::BudgetExhausted {
            kind: BudgetKind::Instructions
        }
    );
    assert_eq!(stop.budget.instructions, 2_000);
}

#[test]
fn a_pc_predicate_is_checked_every_instruction() {
    let mut m = machine(V1, Some(V1_SYM));
    let outcome = check(
        &mut m,
        "pc == $0163",
        &opts(),
        1_000,
        Cadence::EveryInstruction,
    );
    assert_eq!(outcome.stop.pc, 0x0163);
    assert_eq!(outcome.stop.budget.instructions, 9);
    assert_eq!(outcome.stop.symbol.as_deref(), Some("Work"));
}

#[test]
fn a_frame_predicate_stops_at_its_vblank() {
    let mut m = machine(V1, Some(V1_SYM));
    let outcome = check(&mut m, "frame >= 3", &opts(), 200_000, Cadence::Vblank);
    assert_eq!(outcome.stop.time.frame, 3);
}

#[test]
fn a_ly_predicate_stops_at_the_first_scanline() {
    let mut m = machine(BUSY, None);
    let outcome = check(&mut m, "ly == 100", &opts(), 500_000, Cadence::Scanline);
    assert_eq!(outcome.stop.time.ly, 100);
}

#[test]
fn a_hardware_register_predicate_is_checked_every_instruction() {
    let mut m = machine(BUSY, None);
    // DIV changes without a CPU write. rLY would test the same cadence but is
    // `Unsupported` at boundaries where SameBoy has batched mode-3 cycles, so
    // it cannot be evaluated exactly; DIV is always readable.
    let outcome = check(
        &mut m,
        "[rDIV] >= 4",
        &opts(),
        500_000,
        Cadence::EveryInstruction,
    );
    assert!(outcome.stop.time.ticks > 0);
}

#[test]
fn a_dynamic_address_is_checked_every_instruction() {
    let mut m = initialized_v1();
    let regs = Registers {
        h: 0xC0,
        l: 0x00,
        ..m.core().registers()
    };
    m.core_mut().set_registers(&regs);
    let outcome = check(
        &mut m,
        "[hl] == 60",
        &opts(),
        5_000,
        Cadence::EveryInstruction,
    );
    assert_eq!(outcome.stop.pc, 0x016C);
    assert_eq!(outcome.stop.budget.instructions, 537);
}

#[test]
fn a_mixed_frame_and_memory_predicate_still_stops_on_the_write() {
    let mut m = initialized_v1();
    // The cadence is Vblank because `frame` is read, but the memory read can
    // become true mid-frame, so its stores are still armed.
    let outcome = check(
        &mut m,
        "frame >= 0 && u16[wFrame] >= 60",
        &opts(),
        5_000,
        Cadence::Vblank,
    );
    assert_eq!(outcome.stop.pc, 0x016C);
    assert_eq!(outcome.stop.budget.instructions, 537);
}

#[test]
fn a_region_form_read_stops_on_its_write() {
    let mut m = initialized_v1();
    let outcome = check(&mut m, "[wram:0] == 60", &opts(), 5_000, Cadence::Writes);
    assert_eq!(outcome.stop.pc, 0x016C);
    assert_eq!(outcome.stop.budget.instructions, 537);
}

#[test]
fn an_uninitialized_read_fails_with_a_remedy() {
    let mut m = machine(V1, Some(V1_SYM));
    let err = m.run_until("[wNever] == [wNever]", &opts()).unwrap_err();
    let DebugError::Until { error, stop } = &err else {
        panic!("{err:?}")
    };
    assert!(
        error.remedy().contains("--allow-uninit"),
        "{}",
        error.remedy()
    );
    assert!(err.to_string().starts_with("until: "), "{err}");
    assert_eq!(stop.budget.instructions, 0);
    assert_eq!(stop.pc, 0x0100);

    let mut allowed = opts();
    allowed.allow_uninit = true;
    let Outcome { stop, warnings } = m.run_until("[wNever] == [wNever]", &allowed).unwrap();
    assert!(matches!(stop.reason, StopReason::Predicate { .. }));
    assert_eq!(stop.budget.instructions, 0);
    assert!(!warnings.is_empty());
}

#[test]
fn a_repeated_uninitialized_read_warns_once() {
    let mut m = machine(V1, Some(V1_SYM));
    let mut allowed = RunOptions::new(Budget::instructions(20));
    allowed.since = Since::PowerOn;
    allowed.allow_uninit = true;
    let Outcome { stop, warnings } = m.run_until("[wNever] == 1 && pc == pc", &allowed).unwrap();
    assert!(matches!(stop.reason, StopReason::BudgetExhausted { .. }));
    assert_eq!(
        warnings.len(),
        1,
        "an every-instruction read would repeat it"
    );
}

#[test]
fn a_predicate_already_true_stops_before_the_first_instruction() {
    let mut m = machine(V1, Some(V1_SYM));
    let ticks = m.core().now().0;
    let outcome = until(&mut m, "pc == $0100", &opts());
    assert_eq!(
        outcome.stop.reason,
        StopReason::Predicate {
            text: "pc == $0100".into(),
            cadence: Cadence::EveryInstruction,
        }
    );
    assert_eq!(outcome.stop.pc, 0x0100);
    assert_eq!(outcome.stop.budget.instructions, 0);
    assert_eq!(m.core().now().0, ticks, "nothing ran");
}

#[test]
fn a_breakpoint_wins_at_the_instruction_a_predicate_becomes_true() {
    let mut m = machine(V1, Some(V1_SYM));
    let id = m.add_breakpoint(at(0x0163), None).unwrap();
    let stop = until(&mut m, "pc == $0163", &opts()).stop;
    assert_eq!(stop.reason, StopReason::Breakpoint { id });
    assert_eq!(stop.pc, 0x0163);
    assert_eq!(stop.budget.instructions, 9);

    // Without the breakpoint the same predicate stops at the same boundary.
    let mut plain = machine(V1, Some(V1_SYM));
    let outcome = check(
        &mut plain,
        "pc == $0163",
        &opts(),
        1_000,
        Cadence::EveryInstruction,
    );
    assert_eq!(outcome.stop.pc, stop.pc);
    assert_eq!(outcome.stop.budget.instructions, stop.budget.instructions);
}

#[test]
fn a_watch_wins_at_the_instruction_a_predicate_becomes_true() {
    let mut m = initialized_v1();
    let id = m
        .add_watch(WatchAccess::Write, at(0xC000), Some("new == 60"), false)
        .unwrap();
    let stop = until(&mut m, "u16[wFrame] >= 60", &opts()).stop;
    assert_eq!(
        stop.reason,
        StopReason::Watchpoint {
            id,
            access: gbcli_core::Access::Write,
            addr: 0xC000,
            old: Some(59),
            new: 60,
            writer_pc: 0x016B,
        }
    );
    assert_eq!(stop.pc, 0x016C);
}
