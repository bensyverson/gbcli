//! The ignored-write log: the program's CPU writes the PPU drops (VRAM in
//! mode 3, OAM in modes 2-3), counted exactly with the first of each kind,
//! through the mode-blocked-writes patch (leaf si5fnG).

use crate::common;

use common::*;
use gbcli_core::{Budget, Core, CoreConfig, IgnoredWriteKind, IgnoredWrites, Model, StepUnit};
use gbcli_sameboy::SameBoy;

/// `ld a,$91; ldh [rLCDC],a`: the quick boot ROM leaves the LCD off.
const LCD_ON: &[u8] = &[0x3E, 0x91, 0xE0, 0x40];
/// `.w: ldh a,[rLY]; cp 2; jr nz,.w`
const WAIT_LY2: &[u8] = &[0xF0, 0x44, 0xFE, 0x02, 0x20, 0xFA];
/// `.w: ldh a,[rLY]; cp 144; jr nz,.w`
const WAIT_VBLANK: &[u8] = &[0xF0, 0x44, 0xFE, 0x90, 0x20, 0xFA];
/// `.w: ldh a,[rSTAT]; and 3; cp 3; jr nz,.w`
const WAIT_MODE3: &[u8] = &[0xF0, 0x41, 0xE6, 0x03, 0xFE, 0x03, 0x20, 0xF8];
/// `.w: ldh a,[rSTAT]; and 3; cp 2; jr nz,.w`
const WAIT_MODE2: &[u8] = &[0xF0, 0x41, 0xE6, 0x03, 0xFE, 0x02, 0x20, 0xF8];
/// `.w: ldh a,[rSTAT]; and 3; jr nz,.w`
const WAIT_MODE0: &[u8] = &[0xF0, 0x41, 0xE6, 0x03, 0x20, 0xFA];
/// `ld hl,$9800`
const HL_MAP: &[u8] = &[0x21, 0x00, 0x98];
/// `ld a,$FF`
const A_FF: &[u8] = &[0x3E, 0xFF];
/// `ld [hl+],a`
const STORE: &[u8] = &[0x22];
/// `jr @`
const IDLE: &[u8] = &[0x18, 0xFE];

/// Code assembled at $0150, where busy.gb's entry jumps, with marks.
struct Program {
    code: Vec<u8>,
}

impl Program {
    fn new() -> Program {
        Program { code: Vec::new() }
    }

    fn then(mut self, bytes: &[u8]) -> Program {
        self.code.extend_from_slice(bytes);
        self
    }

    /// The address the next byte lands at.
    fn here(&self) -> u16 {
        0x150 + self.code.len() as u16
    }

    fn rom(&self) -> Vec<u8> {
        let mut rom = BUSY.to_vec();
        rom[0x150..0x150 + self.code.len()].copy_from_slice(&self.code);
        rom
    }
}

/// Three VRAM writes in line 2's mode 3, then idles. Returns the ROM and
/// the PC of the first write.
fn vram_in_mode3() -> (Vec<u8>, u16) {
    let p = Program::new()
        .then(LCD_ON)
        .then(HL_MAP)
        .then(WAIT_LY2)
        .then(WAIT_MODE3)
        .then(A_FF);
    let first = p.here();
    let p = p.then(STORE).then(STORE).then(STORE).then(IDLE);
    (p.rom(), first)
}

/// Three VRAM writes in line 2's HBlank and two in VBlank, then idles.
fn vram_in_blanks() -> Vec<u8> {
    Program::new()
        .then(LCD_ON)
        .then(HL_MAP)
        .then(WAIT_LY2)
        .then(WAIT_MODE0)
        .then(A_FF)
        .then(STORE)
        .then(STORE)
        .then(STORE)
        .then(WAIT_VBLANK)
        .then(A_FF)
        .then(STORE)
        .then(STORE)
        .then(IDLE)
        .rom()
}

/// An OAM write in line 2's mode 3 (dropped), one to $FEA0 there (not OAM),
/// one in HBlank (lands) and one in the next line's mode 2 (dropped).
/// Returns the ROM and the PCs of the two dropped writes.
fn oam_writes() -> (Vec<u8>, u16, u16) {
    let p = Program::new()
        .then(LCD_ON)
        .then(WAIT_LY2)
        .then(WAIT_MODE3)
        .then(A_FF);
    let first = p.here();
    let p = p
        .then(&[0xEA, 0x00, 0xFE]) // ld [$FE00],a
        .then(&[0xEA, 0xA0, 0xFE]) // ld [$FEA0],a
        .then(WAIT_MODE0)
        .then(A_FF)
        .then(&[0xEA, 0x01, 0xFE]) // ld [$FE01],a
        .then(WAIT_MODE2)
        .then(A_FF);
    let second = p.here();
    let p = p.then(&[0xEA, 0x02, 0xFE]).then(IDLE); // ld [$FE02],a
    (p.rom(), first, second)
}

fn logged(core: &mut SameBoy, budget: Budget) -> Vec<IgnoredWrites> {
    core.set_ignored_write_log(true).unwrap();
    run(core, budget);
    core.take_ignored_writes().unwrap()
}

#[test]
fn vram_writes_in_mode_3_are_counted_with_the_first() {
    let (rom, first) = vram_in_mode3();
    let mut core = quick_dmg(&rom);
    let found = logged(&mut core, Budget::frames(3));
    assert_eq!(found.len(), 1, "{found:?}");
    let w = found[0];
    assert_eq!(w.kind, IgnoredWriteKind::Vram);
    assert_eq!(w.count, 3);
    assert_eq!((w.pc, w.addr, w.line), (first, 0x9800, 2));
    assert!(core.take_ignored_writes().unwrap().is_empty(), "taken");
}

#[test]
fn vram_writes_in_hblank_and_vblank_are_not() {
    let mut core = quick_dmg(&vram_in_blanks());
    let found = logged(&mut core, Budget::frames(3));
    assert!(found.is_empty(), "{found:?}");
    // They landed: the log's silence is not a run that never wrote.
    for addr in 0x9800..0x9805 {
        assert_eq!(core.bus_read(addr).unwrap(), 0xFF, "{addr:#06x}");
    }
}

#[test]
fn oam_writes_in_modes_2_and_3_are_counted() {
    let (rom, first, _second) = oam_writes();
    let mut core = quick_dmg(&rom);
    let found = logged(&mut core, Budget::frames(3));
    assert_eq!(found.len(), 1, "{found:?}");
    let w = found[0];
    assert_eq!(w.kind, IgnoredWriteKind::Oam);
    assert_eq!(w.count, 2, "mode 3 and mode 2; $FEA0 isn't OAM");
    assert_eq!((w.pc, w.addr, w.line), (first, 0xFE00, 2));
}

#[test]
fn a_cgb_counts_vram_writes_in_mode_3_too() {
    let (rom, first) = vram_in_mode3();
    let mut core = quick_boot(CoreConfig::new(Model::CgbE, rom));
    let found = logged(&mut core, Budget::frames(3));
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!((found[0].count, found[0].pc), (3, first));
}

#[test]
fn nothing_is_counted_with_the_log_off() {
    let (rom, _) = vram_in_mode3();
    let mut core = quick_dmg(&rom);
    run(&mut core, Budget::frames(3));
    core.set_ignored_write_log(true).unwrap();
    assert!(core.take_ignored_writes().unwrap().is_empty());
}

#[test]
fn a_write_by_the_caller_is_not_the_programs() {
    let (rom, first) = vram_in_mode3();
    let mut core = quick_dmg(&rom);
    core.set_ignored_write_log(true).unwrap();
    while core.registers().pc != first {
        core.step(StepUnit::Instruction).unwrap();
    }
    assert!(core.take_ignored_writes().unwrap().is_empty());
    core.bus_write(0x9C00, 0x5A).unwrap();
    assert_ne!(
        core.bus_read(0x9C00).unwrap(),
        0x5A,
        "the write was dropped in mode 3, so the test means something"
    );
    assert!(core.take_ignored_writes().unwrap().is_empty());
    // A step over the program's write counts it.
    core.step(StepUnit::Instruction).unwrap();
    let found = core.take_ignored_writes().unwrap();
    assert_eq!((found.len(), found[0].count), (1, 1), "{found:?}");
}

#[test]
fn the_log_leaves_the_state_as_it_was() {
    let (rom, _) = vram_in_mode3();
    let save_after = |log: bool| {
        let mut core = quick_dmg(&rom);
        core.set_ignored_write_log(log).unwrap();
        run(&mut core, Budget::frames(3));
        core.save()
    };
    assert!(save_after(true) == save_after(false));
}

#[test]
fn a_load_drops_what_was_counted() {
    let (rom, _) = vram_in_mode3();
    let mut core = quick_dmg(&rom);
    let state = core.save();
    core.set_ignored_write_log(true).unwrap();
    run(&mut core, Budget::frames(3));
    core.load(&state).unwrap();
    assert!(core.take_ignored_writes().unwrap().is_empty());
}
