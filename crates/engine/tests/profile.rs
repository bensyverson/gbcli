//! The cycle profiler on hand-written routines (`roms/profile.asm`): every
//! expected cost is summed from the opcode table, never read off the
//! emulator.

mod common;

use common::{cgb_machine, machine, step_to};
use gbcli_core::{Budget, Registers};
use gbcli_engine::disasm::{NoSymbols, decode};
use gbcli_engine::profile::{CpuSpeed, Leg, MeasureError, MeasureSpec, Profile, RoutineCost};
use gbcli_engine::{Machine, Symbols};
use gbcli_protocol::Location;

const ROM: &[u8] = include_bytes!("roms/profile.gb");
const SYM: &str = include_str!("roms/profile.sym");

fn addr(name: &str) -> u16 {
    Symbols::parse(SYM).unwrap().get(name).unwrap().addr
}

fn at(name: &str) -> Location {
    Location {
        addr: addr(name),
        bank: None,
        symbol: Some(name.into()),
    }
}

/// The single-speed T-cycles the opcode table gives the instruction at
/// `addr`, taken when it is a conditional branch.
fn table_t(addr: u16) -> u64 {
    let bytes = &ROM[usize::from(addr)..];
    u64::from(decode(bytes, addr, &NoSymbols).unwrap().cycles.taken)
}

/// The instructions at these addresses, by the table.
fn sum(path: &[u16]) -> u64 {
    path.iter().map(|&a| table_t(a)).sum()
}

/// Instruction addresses of each routine's body, in execution order.
fn leaf() -> Vec<u16> {
    vec![addr("Leaf"), addr("Leaf") + 2] // ld a,5; ret
}

fn outer_own() -> Vec<u16> {
    let o = addr("Outer");
    vec![o, o + 3, o + 4] // call Leaf; nop; ret
}

fn trampoline() -> Vec<u16> {
    let t = addr("Trampoline");
    vec![t, t + 3, t + 4, addr("Target")] // ld hl; push hl; ret; ret
}

/// One pass of the main loop, from MainLoop back to it.
fn main_loop() -> Vec<u16> {
    let m = addr("MainLoop");
    let d = addr("Discard");
    let mut path = vec![m];
    path.extend(leaf());
    path.push(m + 3);
    path.push(outer_own()[0]);
    path.extend(leaf());
    path.extend(&outer_own()[1..]);
    path.extend([m + 6, d, d + 1, m + 9]); // call Discard; pop hl; jp .back; call Trampoline
    path.extend(trampoline());
    path.push(m + 12); // jr MainLoop
    path
}

fn at_main_loop() -> Machine {
    let mut m = machine(ROM, Some(SYM));
    step_to(&mut m, addr("MainLoop"));
    m
}

fn spec(from: &str, to: &str, iterations: u32) -> MeasureSpec {
    MeasureSpec {
        from: at(from),
        to: at(to),
        iterations,
        budget: Budget::instructions(10_000),
    }
}

#[test]
fn the_hand_computed_main_loop_is_288_t_by_the_table() {
    // Pins the expectation itself: 4 calls (24) + 2 leaves (8+16) + nop (4)
    // + ret (16) + pop (12) + jp (16) + ld hl (12) + push (16) + 2 rets (16)
    // + jr (12).
    assert_eq!(main_loop().len(), 18);
    assert_eq!(sum(&main_loop()), 288);
}

#[test]
fn measure_counts_the_main_loop_at_its_table_cost_every_iteration() {
    let m = at_main_loop();
    let result = m.measure(&spec("MainLoop", "MainLoop", 3)).unwrap();
    let t = sum(&main_loop());
    assert_eq!(result.n(), 3);
    assert_eq!(result.speed, CpuSpeed::Single);
    let stats = result.t();
    assert_eq!((stats.min, stats.max, stats.mean), (t, t, t as f64));
    assert_eq!(result.m().max, t / 4);
    let (lo, hi, mean) = result.frame_percent();
    let pct = t as f64 * 100.0 / 70_224.0;
    assert_eq!((lo, hi, mean), (pct, pct, pct));
    assert!(result.exceeds(t - 1));
    assert!(!result.exceeds(t));
}

#[test]
fn measure_runs_from_the_first_arrival_at_from_to_the_next_at_to() {
    // From Leaf's first instruction to Outer's: the leaf, then `call Outer`.
    let m = at_main_loop();
    let result = m.measure(&spec("Leaf", "Outer", 1)).unwrap();
    let expected = sum(&leaf()) + table_t(addr("MainLoop") + 3);
    assert_eq!(result.t().max, expected);
}

#[test]
fn measure_leaves_the_machine_unchanged() {
    let m = at_main_loop();
    let before = m.core().save();
    m.measure(&spec("MainLoop", "MainLoop", 2)).unwrap();
    assert_eq!(m.core().save(), before);
}

#[test]
fn measure_refuses_zero_iterations() {
    let m = at_main_loop();
    let err = m.measure(&spec("MainLoop", "MainLoop", 0)).unwrap_err();
    assert_eq!(err, MeasureError::NoIterations);
}

#[test]
fn measure_says_how_far_it_got_when_the_budget_runs_out() {
    let m = at_main_loop();
    let mut never = spec("MainLoop", "MainLoop", 1);
    never.to = Location {
        addr: 0x3000,
        bank: None,
        symbol: None,
    };
    never.budget = Budget::instructions(500);
    let err = m.measure(&never).unwrap_err();
    assert_eq!(
        err,
        MeasureError::Budget {
            completed: 0,
            leg: Leg::ToEnd
        }
    );
}

#[test]
fn measure_refuses_a_span_across_a_speed_switch() {
    let mut m = cgb_machine(ROM, Some(SYM));
    let regs = Registers {
        pc: addr("SpeedSwitch"),
        ..m.core().registers()
    };
    m.core_mut().set_registers(&regs);
    let err = m
        .measure(&spec("SpeedSwitch.before", "SpeedSwitch.after", 1))
        .unwrap_err();
    assert_eq!(err, MeasureError::SpeedSwitch { iteration: 1 });
}

fn routine<'a>(profile: &'a Profile, name: &str) -> &'a RoutineCost {
    profile
        .routines
        .iter()
        .find(|r| r.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no {name} in {:#?}", profile.routines))
}

#[test]
fn the_profile_charges_each_routine_its_table_cost() {
    let mut m = at_main_loop();
    let profile = m.profile(Budget::instructions(18)).unwrap();
    assert_eq!(m.core().registers().pc, addr("MainLoop"), "one pass");
    assert_eq!(profile.total_t, sum(&main_loop()));

    let leaf_t = sum(&leaf());
    let leaf = routine(&profile, "Leaf");
    assert_eq!(
        (leaf.calls, leaf.exclusive_t, leaf.inclusive_t),
        (2, 2 * leaf_t, 2 * leaf_t)
    );

    let outer = routine(&profile, "Outer");
    let own = sum(&outer_own());
    assert_eq!(
        (outer.calls, outer.exclusive_t, outer.inclusive_t),
        (1, own, own + leaf_t)
    );

    // Discard pops its own return address, so it ends there: `jp` is the
    // caller's.
    let discard = routine(&profile, "Discard");
    let popped = table_t(addr("Discard"));
    assert_eq!(
        (discard.calls, discard.exclusive_t, discard.inclusive_t),
        (1, popped, popped)
    );

    // Trampoline's own `ret` is a jump to Target, whose `ret` returns.
    let tramp = routine(&profile, "Trampoline");
    let t = sum(&trampoline());
    assert_eq!(
        (tramp.calls, tramp.exclusive_t, tramp.inclusive_t),
        (1, t, t)
    );

    let root = routine(&profile, "MainLoop");
    assert_eq!(root.calls, 0, "the routine the profile started in");
    assert_eq!(root.inclusive_t, profile.total_t);
    let exclusive: u64 = profile.routines.iter().map(|r| r.exclusive_t).sum();
    assert_eq!(exclusive, profile.total_t, "every cycle is charged once");

    assert_eq!(profile.shadow.unwound, 1, "Discard");
    assert_eq!(profile.shadow.ret_jumps, 1, "Trampoline");
    assert_eq!(profile.shadow.open, 0);
}

#[test]
fn a_profile_cut_short_counts_the_open_calls_so_far() {
    // Stops inside Outer's call to Leaf, before Leaf's `ret`.
    let mut m = at_main_loop();
    let profile = m.profile(Budget::instructions(6)).unwrap();
    assert_eq!(profile.shadow.open, 2);
    let outer = routine(&profile, "Outer");
    assert_eq!(
        outer.inclusive_t,
        table_t(addr("Outer")) + table_t(addr("Leaf"))
    );
}

/// At HaltLoop with the stack set up: waits for each vblank in HALT.
fn halting() -> Machine {
    let mut m = at_main_loop();
    let regs = Registers {
        pc: addr("HaltLoop"),
        ..m.core().registers()
    };
    m.core_mut().set_registers(&regs);
    m
}

/// An interrupt dispatch, which the opcode table doesn't list: 5 M-cycles,
/// plus the M-cycle SameBoy spends leaving HALT in the same step (`GB_cpu_run`
/// in sm83_cpu.c advances 4 T in its halted branch, then dispatches). The
/// profile charges the whole step to the handler.
const DISPATCH_T: u64 = 20 + 4;

#[test]
fn an_interrupt_handler_is_a_routine_entered_at_its_vector() {
    // Ends halfway through a frame, in HALT, long after the last handler.
    let mut m = halting();
    let half = gbcli_core::Ticks::PER_FRAME / 2;
    let profile = m.profile(Budget::ticks(3 * 2 * half + half)).unwrap();
    assert!(m.core().registers().halted);
    let handler = routine(&profile, "VBlankVector");
    let vblanks = handler.calls;
    assert!(vblanks >= 2, "{vblanks}");
    let v = addr("VBlank");
    let body = table_t(addr("VBlankVector")) + sum(&[v, v + 3, v + 4]); // jp; ld hl; inc [hl]; reti
    assert_eq!(handler.inclusive_t, vblanks * (DISPATCH_T + body));
    assert_eq!(profile.shadow, Default::default(), "every RETI matched");
    assert!(profile.idle_t > 0);
}

#[test]
fn a_frame_that_halts_is_not_a_lag_frame_and_a_busy_one_is() {
    let mut m = halting();
    let frames = m.frame_budget(3).unwrap();
    assert_eq!(frames.len(), 3);
    for f in &frames[1..] {
        assert!(!f.lag, "{f:?}");
        assert!(f.idle_t > f.active_t, "{f:?}");
        // A whole frame, give or take the step the vblank fell in.
        assert!((f.active_t + f.idle_t).abs_diff(70_224) <= 24, "{f:?}");
    }
    let mut busy = at_main_loop();
    let frames = busy.frame_budget(2).unwrap();
    assert!(frames.iter().all(|f| f.lag && f.idle_t == 0), "{frames:?}");
}
