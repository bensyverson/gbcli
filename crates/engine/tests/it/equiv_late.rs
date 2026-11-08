//! The equivalence check on a variable the program sets late
//! (`roms/equiv_late.inc`): wX is first written after the fourth vblank, and
//! the two builds keep it at different WRAM addresses. Power-on WRAM is
//! random, so before the write each side reads its own never-written byte,
//! and the check says so rather than reporting a plain divergence.

use crate::common;

use common::machine;
use gbcli_core::Budget;
use gbcli_engine::equiv::{Difference, EquivSpec, RamRange, Side, Verdict, compare};
use gbcli_engine::{Machine, RunOptions};
use gbcli_protocol::StepKind;

const A: &[u8] = include_bytes!("../roms/equiv_late_a.gb");
const A_SYM: &str = include_str!("../roms/equiv_late_a.sym");
const B: &[u8] = include_bytes!("../roms/equiv_late_b.gb");
const B_SYM: &str = include_str!("../roms/equiv_late_b.sym");

fn wx() -> Vec<RamRange> {
    vec![RamRange {
        at: "wX".into(),
        len: 1,
    }]
}

fn frames(m: &mut Machine, n: u64) {
    for _ in 0..n {
        m.step(StepKind::Frame, &RunOptions::new(Budget::frames(2)))
            .expect("frame");
    }
}

#[test]
fn a_moved_variable_read_before_the_program_writes_it_is_never_written_on_both_sides() {
    let (mut a, mut b) = (machine(A, Some(A_SYM)), machine(B, Some(B_SYM)));
    let ram = wx();
    let spec = EquivSpec {
        ram: &ram,
        ..EquivSpec::new(20)
    };
    let Verdict::Diverged(d) = compare(&mut a, &mut b, &spec).unwrap() else {
        panic!("the two random bytes should differ");
    };
    assert_eq!(d.frame, 1);
    let [Difference::Ram { never_written, .. }] = d.differences.as_slice() else {
        panic!("expected wX to differ: {:?}", d.differences);
    };
    assert_eq!(never_written, &vec![Side::A, Side::B]);
}

#[test]
fn started_after_the_program_writes_it_the_two_builds_are_the_same() {
    // wX is written after the fourth vblank, which the program sees at the
    // start of the fifth frame (a frame stop comes before the ROM's own
    // vblank code), so five frames in, both sides have written it.
    let (mut a, mut b) = (machine(A, Some(A_SYM)), machine(B, Some(B_SYM)));
    frames(&mut a, 5);
    frames(&mut b, 5);
    let ram = wx();
    let spec = EquivSpec {
        ram: &ram,
        ..EquivSpec::new(20)
    };
    match compare(&mut a, &mut b, &spec).unwrap() {
        Verdict::Same { frames, ended } => assert_eq!((frames, ended), (20, None)),
        Verdict::Diverged(d) => panic!("frame {}: {:?}", d.frame, d.differences),
    }
}
