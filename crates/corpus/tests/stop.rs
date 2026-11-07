//! The driver in STOP: by default a STOP nothing can wake ends the case at
//! once; while an input is coming (docboy's scheduled keys) the core passes
//! the time itself, so a cartridge RTC counts it (leaf 34Nx7n).

use gbcli_core::{CoreConfig, InStop, Model};
use gbcli_corpus::drive::{Driver, Event, TICKS_PER_FRAME};
use gbcli_sameboy::SameBoy;

/// Executes STOP with neither joypad row selected (see its .asm).
const STOP: &[u8] = include_bytes!("../../sameboy/tests/roms/stop.gb");

fn driver() -> Driver {
    let core = SameBoy::new(CoreConfig::new(Model::DmgB, STOP.to_vec())).expect("core");
    Driver::boot(Box::new(core), STOP).expect("boot")
}

#[test]
fn a_stop_nothing_can_wake_ends_the_case() {
    let mut d = driver();
    assert_eq!(d.run_for(10 * TICKS_PER_FRAME).unwrap(), Event::Stopped);
    assert!(d.elapsed() < TICKS_PER_FRAME);
}

#[test]
fn passing_time_in_stop_runs_the_core_to_the_deadline() {
    let mut d = driver();
    d.in_stop = InStop::PassTime;
    let until = 10 * TICKS_PER_FRAME;
    assert_eq!(d.run_for(until).unwrap(), Event::Budget);
    assert!(d.elapsed() >= until);
    // The core's own clock, with nothing skipped beside it.
    assert_eq!(d.elapsed(), d.core.now().0 - d.start);
}
