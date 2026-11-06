//! Bank-aware breakpoint and watch filters, the write veto, and never-written tracking.

mod common;

use common::*;
use gbcli_core::{
    Access, AddrFilter, BankSel, Budget, Core, CoreConfig, CoreError, Hit, Model, Region,
    RunRequest, Stop, StopReason, WriteTracking,
};
use gbcli_sameboy::SameBoy;

const BANKED: &[u8] = include_bytes!("roms/banked.gb");

fn filter(entries: &[(u16, BankSel)]) -> AddrFilter {
    let mut filter = AddrFilter::new();
    for &(addr, bank) in entries {
        filter.insert(addr, bank);
    }
    filter
}

fn hit(stop: &Stop) -> Hit {
    match stop.reason {
        StopReason::Candidate(hit) => hit,
        other => panic!("expected a filter hit, got {other:?}"),
    }
}

/// Runs with one kind of filter set.
fn run_with<'a>(core: &mut SameBoy, set: impl FnOnce(&mut RunRequest<'a>)) -> Stop {
    let mut req = RunRequest::new(Budget::frames(5));
    set(&mut req);
    core.run(&mut req).unwrap()
}

#[test]
fn a_breakpoint_on_a_banked_address_fires_only_in_that_bank() {
    let mut core = quick_dmg(BANKED);
    let exec = filter(&[(0x4000, BankSel::Bank(2))]);
    let first = run_with(&mut core, |r| r.exec = &exec);
    let h = hit(&first);
    assert_eq!((h.access, h.addr, h.bank), (Access::Exec, 0x4000, 2));
    assert_eq!(h.old, Some(0x3E)); // the opcode, not yet executed
    assert_eq!((first.pc, first.bank), (0x4000, 2));
    assert_eq!(core.registers().pc, 0x4000);
    assert_eq!(core.registers().a, 2); // set by the caller, before the routine's own `ld a, 2`

    // Resuming runs the routine, then bank 1's routine at the same address,
    // and stops only on the next pass through bank 2.
    let second = run_with(&mut core, |r| r.exec = &exec);
    assert_eq!(hit(&second).bank, 2);
    assert!(second.used.instructions > 10, "{:?}", second.used);
}

#[test]
fn an_any_bank_breakpoint_fires_in_whichever_bank_comes_first() {
    let mut core = quick_dmg(BANKED);
    let exec = filter(&[(0x4000, BankSel::Any)]);
    let stop = run_with(&mut core, |r| r.exec = &exec);
    assert_eq!(hit(&stop).bank, 1);
}

#[test]
fn a_write_watch_on_banked_cart_ram_fires_in_its_bank_with_old_and_new_values() {
    let mut core = quick_dmg(BANKED);
    let write = filter(&[(0xA000, BankSel::Bank(2))]);
    let stop = run_with(&mut core, |r| r.write = &write);
    let h = hit(&stop);
    assert_eq!((h.access, h.addr, h.bank), (Access::Write, 0xA000, 2));
    assert_eq!(h.new, 0x22);
    assert_eq!(h.old, Some(0xFF)); // cart RAM powers on as $FF in SameBoy
    assert_eq!(h.pc, 0x4007); // the writing instruction, in bank 2
    assert_eq!(stop.pc, 0x400A); // stopped after it completed
    assert!(!h.vetoed);
    assert_eq!(core.region(Region::Sram).unwrap().bytes[0x4000], 0x22);
}

#[test]
fn a_read_watch_fires_after_the_read() {
    let mut core = quick_dmg(BANKED);
    core.poke(Region::Wram, 0x200, &[0x5A]).unwrap();
    let read = filter(&[(0xC200, BankSel::Any)]);
    let stop = run_with(&mut core, |r| r.read = &read);
    let h = hit(&stop);
    assert_eq!(
        (h.access, h.addr, h.new, h.pc),
        (Access::Read, 0xC200, 0x5A, 0x4010)
    );
    assert_eq!(core.registers().a, 0x5A);
}

#[test]
fn an_echo_ram_write_is_reported_at_its_wram_address() {
    let mut core = quick_dmg(BANKED);
    let write = filter(&[(0xC124, BankSel::Any)]);
    let stop = run_with(&mut core, |r| r.write = &write);
    let h = hit(&stop);
    assert_eq!((h.addr, h.new, h.pc), (0xC124, 0x11, 0x400D));
}

#[test]
fn a_write_veto_leaves_memory_unchanged_and_reports_the_attempted_value() {
    let mut core = quick_dmg(BANKED);
    core.poke(Region::Wram, 0x123, &[0x77]).unwrap();
    let veto = filter(&[(0xC123, BankSel::Any)]);
    let stop = run_with(&mut core, |r| r.veto = &veto);
    let h = hit(&stop);
    assert_eq!(
        (h.access, h.addr, h.old, h.new),
        (Access::Write, 0xC123, Some(0x77), 0x11)
    );
    assert!(h.vetoed);
    assert_eq!(core.region(Region::Wram).unwrap().bytes[0x123], 0x77);
}

#[test]
fn filters_are_disarmed_after_the_run() {
    let mut core = quick_dmg(BANKED);
    let write = filter(&[(0xC123, BankSel::Any)]);
    run_with(&mut core, |r| r.write = &write);
    let stop = run(&mut core, Budget::frames(2));
    assert_eq!(
        stop.reason,
        StopReason::Budget(gbcli_core::BudgetKind::Frames)
    );
}

#[test]
fn cpu_writes_mark_bytes_written_bank_by_bank() {
    let mut core = quick_dmg(BANKED);
    assert!(core.written(Region::Wram).unwrap().never_written(0x123));
    run(&mut core, Budget::frames(2));
    let wram = core.written(Region::Wram).unwrap();
    assert_eq!(wram.len, 0x2000);
    assert!(wram.cpu_wrote(0x123) && wram.cpu_wrote(0x124)); // the second via echo RAM
    assert!(wram.never_written(0x200)); // only ever read
    let hram = core.written(Region::Hram).unwrap();
    assert!(hram.cpu_wrote(0x7C) && hram.cpu_wrote(0x7D)); // `call` pushes to $FFFC-D
    assert!(hram.never_written(0x00));
    let sram = core.written(Region::Sram).unwrap();
    assert_eq!(sram.len, 0x8000);
    assert!(sram.cpu_wrote(0x2000) && sram.cpu_wrote(0x4000)); // A000 in banks 1 and 2
    assert!(sram.never_written(0x0000) && sram.never_written(0x6000)); // banks 0 and 3
}

#[test]
fn pokes_and_bus_writes_are_told_apart() {
    let mut core = quick_dmg(BANKED);
    core.poke(Region::Wram, 0x300, &[1, 2]).unwrap();
    core.bus_write(0xC400, 9).unwrap();
    let wram = core.written(Region::Wram).unwrap();
    assert!(wram.was_poked(0x300) && wram.was_poked(0x301) && !wram.cpu_wrote(0x300));
    assert!(wram.cpu_wrote(0x400) && !wram.was_poked(0x400));
}

#[test]
fn written_bits_are_saved_and_loaded() {
    let mut core = quick_dmg(BANKED);
    run(&mut core, Budget::frames(2));
    let blob = core.save();
    let mut fresh = quick_dmg(BANKED);
    assert!(fresh.written(Region::Wram).unwrap().never_written(0x123));
    fresh.load(&blob).unwrap();
    assert!(fresh.written(Region::Wram).unwrap().cpu_wrote(0x123));
}

#[test]
fn vram_and_oam_are_not_tracked_because_the_hook_misses_dma() {
    let core = quick_dmg(BANKED);
    for region in [Region::Vram, Region::Oam] {
        let err = core.written(region).unwrap_err();
        assert!(matches!(err, CoreError::Unsupported { .. }), "{err:?}");
    }
}

#[test]
fn tracking_off_refuses_written_queries() {
    let mut config = CoreConfig::new(Model::DmgB, BANKED.to_vec());
    config.write_tracking = WriteTracking::Off;
    let core = SameBoy::new(config).unwrap();
    let err = core.written(Region::Wram).unwrap_err();
    assert!(matches!(err, CoreError::InvalidState { .. }), "{err:?}");
}
