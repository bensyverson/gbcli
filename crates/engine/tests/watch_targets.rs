//! Watches and breakpoints match the byte an access reaches (design §2.1,
//! §11): WRAMX named as bank 00, a CPU write that OAM DMA's bus conflict
//! moves, and more watched reads in one step than a fixed buffer holds.

mod common;

use common::*;
use gbcli_core::{Access, Budget};
use gbcli_engine::{Machine, RunOptions, Since, mem};
use gbcli_protocol::{Location, StopReason, WatchAccess};

fn opts(instructions: u64) -> RunOptions<'static> {
    let mut opts = RunOptions::new(Budget::instructions(instructions));
    opts.since = Since::PowerOn;
    opts.allow_uninit = true;
    opts
}

fn at(addr: u16, bank: Option<u16>) -> Location {
    Location {
        addr,
        bank,
        symbol: None,
    }
}

fn stop(m: &mut Machine, instructions: u64) -> StopReason {
    m.run(&opts(instructions)).expect("run").stop.reason
}

#[test]
fn a_watch_on_wramx_bank_00_fires() {
    // `rgblink -w` links WRAMX as bank 00; the CPU reaches it with bank 1
    // mapped (SameBoy never maps WRAM bank 0 at $D000), as the evaluator
    // already reads it. ld hl,$D000; xor a; ld [hl],a; .l: inc [hl]; jr .l
    let rom = program(&[0x21, 0x00, 0xD0, 0xAF, 0x77, 0x34, 0x18, 0xFD]);
    let mut m = machine(&rom, Some("00:D000 wCnt\n"));
    let wcnt = gbcli_engine::addr::parse("wCnt", m.symbols()).expect("wCnt");
    assert_eq!(wcnt.bank, Some(0));
    let id = m
        .add_watch(WatchAccess::Write, wcnt, None, false)
        .expect("watch");
    match stop(&mut m, 500) {
        StopReason::Watchpoint { id: hit, addr, .. } => {
            assert_eq!((hit, addr), (id, 0xD000));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_breakpoint_on_wramx_bank_00_fires() {
    // ld a,$C9; ld [$D000],a (a `ret` there); .l: call $D000; jr .l
    let rom = program(&[0x3E, 0xC9, 0xEA, 0x00, 0xD0, 0xCD, 0x00, 0xD0, 0x18, 0xFB]);
    let mut m = machine(&rom, None);
    let id = m
        .add_breakpoint(at(0xD000, Some(0)), None)
        .expect("breakpoint");
    assert_eq!(stop(&mut m, 500), StopReason::Breakpoint { id });
    assert_eq!(m.core().registers().pc, 0xD000);
}

#[test]
fn a_write_cgb_oam_dma_moves_is_watched_where_it_lands() {
    // On CGB, while OAM DMA reads ROM, the routine's ld [$D123],a lands in
    // $C123 (SameBoy memory.c): the watch on $C123 sees it, the one on
    // $D123 doesn't, since that byte never changes.
    let (rom, call_at) = dma_conflict_rom(true);
    let mut m = cgb_machine(&rom, None);
    step_to(&mut m, call_at);
    let named = m
        .add_watch(WatchAccess::Write, at(0xD123, None), None, false)
        .expect("watch");
    let landed = m
        .add_watch(WatchAccess::Write, at(0xC123, None), None, false)
        .expect("watch");
    match stop(&mut m, 200) {
        StopReason::Watchpoint {
            id,
            addr,
            new,
            writer_pc,
            ..
        } => {
            assert_ne!(id, named);
            assert_eq!((id, addr, new, writer_pc), (landed, 0xC123, 0x77, 0xFF86));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_write_dmg_oam_dma_drops_leaves_its_byte_unwritten() {
    // On DMG, while OAM DMA reads ROM, a CPU write to WRAM goes to the DMA's
    // source address instead (SameBoy memory.c): $D123 is never written.
    let (rom, call_at) = dma_conflict_rom(false);
    let mut m = machine(&rom, None);
    step_to(&mut m, call_at);
    assert!(mem::never_written(m.core(), 0xD123));
    steps(&mut m, 120);
    assert_ne!(m.core().registers().pc, 0xFF86, "the routine returned");
    assert!(
        mem::never_written(m.core(), 0xD123),
        "a dropped write is not a write"
    );
}

#[test]
fn every_watched_read_of_a_cgb_gdma_is_seen() {
    // Fills $C000-$C00F with $10-$1F, then copies them to VRAM with one
    // 16-byte general-purpose HDMA, which reads every source byte in the
    // step of the instruction that starts it.
    #[rustfmt::skip]
    let code = [
        0x21, 0x00, 0xC0,       // ld hl, $C000
        0x3E, 0x10,             // ld a, $10
        0x22, 0x3C,             // .fill: ld [hl+], a; inc a
        0xFE, 0x20, 0x20, 0xFA, // cp $20; jr nz, .fill
        0x3E, 0xC0, 0xE0, 0x51, // HDMA1: source $C000
        0x3E, 0x00, 0xE0, 0x52,
        0x3E, 0x80, 0xE0, 0x53, // HDMA3: destination $8000
        0x3E, 0x00, 0xE0, 0x54,
        0x3E, 0x00, 0xE0, 0x55, // HDMA5: 16 bytes, general purpose
        0x18, 0xFE,             // jr @
    ];
    let mut m = cgb_machine(&program(&code), None);
    // Eight watches whose condition never holds, then one that always does:
    // the ninth read must still reach the engine.
    for addr in 0xC000..0xC008 {
        m.add_watch(WatchAccess::Read, at(addr, None), Some("new == $AA"), false)
            .expect("watch");
    }
    let last = m
        .add_watch(WatchAccess::Read, at(0xC008, None), None, false)
        .expect("watch");
    match stop(&mut m, 500) {
        StopReason::Watchpoint {
            id,
            access,
            addr,
            new,
            ..
        } => assert_eq!((id, access, addr, new), (last, Access::Read, 0xC008, 0x18)),
        other => panic!("{other:?}"),
    }
}
