//! `measure` under a key timeline (`roms/measure_keys.asm`): Work spins a
//! 100-pass loop while A is held, so its cost says whether the measurement
//! saw the key. Costs are summed from the opcode table: 76 T to the branch,
//! then a taken `jr nz` (12) with A released, or a not-taken one (8),
//! `ld c` (8), 99 taken passes of `dec c; jr nz` (16) and the last (12).

use crate::common::machine;

use gbcli_core::Budget;
use gbcli_engine::Symbols;
use gbcli_engine::profile::{MeasureSpec, Measurement};
use gbcli_protocol::{KeyEvent, Location, parse_keys};

const ROM: &[u8] = include_bytes!("../roms/measure_keys.gb");
const SYM: &str = include_str!("../roms/measure_keys.sym");

const RELEASED_T: u64 = 76 + 12;
const HELD_T: u64 = 76 + 8 + 8 + 99 * 16 + 12;

fn at(name: &str) -> Location {
    Location {
        addr: Symbols::parse(SYM).unwrap().get(name).unwrap().addr,
        bank: None,
        symbol: Some(name.into()),
    }
}

fn measure(iterations: u32, keys: &[KeyEvent]) -> Measurement {
    let mut m = machine(ROM, Some(SYM));
    let spec = MeasureSpec {
        from: at("Work"),
        to: at("Work.done"),
        iterations,
        budget: Budget::frames(60),
        interrupt: None,
        keys,
    };
    let measured = m.measure(&spec).expect("measured");
    assert_eq!(m.keys().0, 0, "the measurement ran on a copy");
    measured
}

fn t(m: &Measurement) -> Vec<u64> {
    m.samples.iter().map(|s| s.t(m.speed)).collect()
}

#[test]
fn the_table_costs_hold_with_no_keys() {
    let m = measure(3, &[]);
    assert_eq!(t(&m), vec![RELEASED_T; 3]);
}

#[test]
fn a_key_held_from_frame_0_is_held_for_every_iteration() {
    let m = measure(3, &parse_keys("A@0").unwrap());
    assert_eq!(t(&m), vec![HELD_T; 3]);
}

/// Work runs once a frame, so an event at frame 2 lands between two
/// iterations; one released before frame 4 is gone again by the end.
#[test]
fn later_events_apply_at_their_frame_offsets() {
    let m = measure(6, &parse_keys("A@2 -A@4").unwrap());
    let costs = t(&m);
    assert_eq!(costs.first(), Some(&RELEASED_T), "{costs:?}");
    assert!(costs.contains(&HELD_T), "{costs:?}");
    assert_eq!(costs.last(), Some(&RELEASED_T), "{costs:?}");
    assert!(
        m.frames >= 4,
        "the measurement reached frame 4: {}",
        m.frames
    );
}
