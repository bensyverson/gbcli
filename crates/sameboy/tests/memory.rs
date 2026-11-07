//! Regions, decoded registers, the three writes, registers and keys.

mod common;

use common::*;
use gbcli_core::{Budget, Core, CoreError, Feature, IoReg, Keys, Region, Registers, StepUnit};

#[test]
fn regions_have_their_dmg_sizes() {
    let core = quick_dmg(BUSY);
    let size = |r| core.region(r).unwrap().bytes.len();
    assert_eq!(size(Region::Rom), 0x8000);
    assert_eq!(size(Region::Wram), 0x2000);
    assert_eq!(size(Region::Vram), 0x2000);
    assert_eq!(size(Region::Sram), 0);
    assert_eq!(size(Region::Hram), 0x7F);
    assert_eq!(size(Region::Oam), 0xA0);
    assert_eq!(size(Region::Io), 0x80);
    assert_eq!(size(Region::Ie), 1);
    assert!(size(Region::BootRom) >= 0x100);
}

#[test]
fn a_region_shows_what_the_program_wrote_and_its_bank() {
    let mut core = quick_dmg(BUSY);
    run(&mut core, Budget::frames(5));
    let wram = core.region(Region::Wram).unwrap();
    assert!(
        (4..=5).contains(&wram.bytes[2]),
        "vblanks {}",
        wram.bytes[2]
    );
    assert_eq!(core.region(Region::Rom).unwrap().bank, 1);
    assert_eq!(core.region(Region::Rom).unwrap().bytes[0x150], 0xF3); // di
}

#[test]
fn cgb_palette_ram_is_refused_on_a_dmg() {
    let core = quick_dmg(BUSY);
    for region in [Region::BgPalette, Region::ObjPalette] {
        let err = core.region(region).unwrap_err();
        assert!(matches!(err, CoreError::InvalidState { .. }), "{err:?}");
    }
}

#[test]
fn decoded_registers_equal_what_the_cpu_reads() {
    let mut core = quick_dmg(BUSY);
    run(&mut core, Budget::frames(3));
    run(&mut core, Budget::instructions(777));
    for reg in [
        IoReg::JOYP,
        IoReg::DIV,
        IoReg::TIMA,
        IoReg::TMA,
        IoReg::TAC,
        IoReg::IF,
        IoReg::LCDC,
        IoReg::SCY,
        IoReg::SCX,
        IoReg::LYC,
        IoReg::BGP,
        IoReg::BANK,
    ] {
        let decoded = core.io_register(reg).unwrap();
        assert_eq!(
            decoded,
            core.bus_read(0xFF00 | u16::from(reg.0)).unwrap(),
            "{reg:?}"
        );
    }
}

#[test]
fn div_is_decoded_from_the_internal_counter_not_the_io_array() {
    let mut core = quick_dmg(BUSY);
    let first = core.io_register(IoReg::DIV).unwrap();
    run(&mut core, Budget::ticks(2 * 256 * 10)); // 10 DIV increments
    let later = core.io_register(IoReg::DIV).unwrap();
    assert_eq!(later.wrapping_sub(first), 10);
}

#[test]
fn nr52_is_unsupported_while_the_apu_has_unrun_cycles() {
    let core = quick_dmg(BUSY);
    let err = core.io_register(IoReg::NR52).unwrap_err();
    assert!(
        matches!(
            err,
            CoreError::Unsupported {
                feature: Feature::SideEffectFreeIo,
                ..
            }
        ),
        "{err:?}"
    );
}

#[test]
fn an_address_outside_ff00_ff7f_is_not_an_io_register() {
    let core = quick_dmg(BUSY);
    let err = core.io_register(IoReg(0x80)).unwrap_err();
    assert!(matches!(err, CoreError::OutOfRange { .. }), "{err:?}");
}

#[test]
fn ppu_status_reflects_lcdc_and_ly() {
    let mut core = quick_dmg(BUSY);
    run(&mut core, Budget::frames(2));
    let ppu = core.ppu();
    assert!(ppu.lcd_on);
    assert!(ppu.ly <= 153);
    assert_eq!(ppu.dot, None);
}

#[test]
fn a_bus_write_has_io_side_effects() {
    let mut core = quick_dmg(BUSY);
    run(&mut core, Budget::frames(2));
    core.bus_write(0xFF04, 0x55).unwrap(); // any write resets DIV
    assert_eq!(core.io_register(IoReg::DIV).unwrap(), 0);
    core.bus_write(0xFF40, 0x00).unwrap(); // LCD off resets LY
    assert!(!core.ppu().lcd_on);
    assert_eq!(core.ppu().ly, 0);
}

#[test]
fn a_poke_writes_backing_memory_without_side_effects() {
    let mut core = quick_dmg(BUSY);
    run(&mut core, Budget::frames(1));
    let div = core.io_register(IoReg::DIV).unwrap();
    core.poke(Region::Io, 0x04, &[0x00]).unwrap(); // raw byte; DIV lives elsewhere
    assert_eq!(core.io_register(IoReg::DIV).unwrap(), div);
    core.poke(Region::Wram, 0x100, &[1, 2, 3]).unwrap();
    assert_eq!(
        &core.region(Region::Wram).unwrap().bytes[0x100..0x103],
        [1, 2, 3]
    );
    assert_eq!(core.bus_read(0xC101).unwrap(), 2);
}

#[test]
fn a_poke_to_rom_is_refused_with_the_patch_remedy() {
    let mut core = quick_dmg(BUSY);
    match core.poke(Region::Rom, 0x200, &[0]) {
        Err(CoreError::InvalidState { reason }) => assert!(reason.contains("patch"), "{reason}"),
        other => panic!("{other:?}"),
    }
}

/// A poke at a CPU address the boot ROM overlays lands in the boot ROM, so
/// it must be refused as a ROM poke is: a save state doesn't carry it.
#[test]
fn a_poke_to_the_boot_rom_is_refused() {
    let mut core = quick_dmg(BUSY);
    let before = core.region(Region::BootRom).unwrap().bytes[0];
    let err = core.poke(Region::BootRom, 0, &[!before]).unwrap_err();
    assert!(matches!(err, CoreError::InvalidState { .. }), "{err:?}");
    assert_eq!(core.region(Region::BootRom).unwrap().bytes[0], before);
}

#[test]
fn a_poke_past_the_region_is_out_of_range() {
    let mut core = quick_dmg(BUSY);
    let err = core.poke(Region::Hram, 0x7E, &[1, 2]).unwrap_err();
    assert!(matches!(err, CoreError::OutOfRange { .. }), "{err:?}");
    assert_ne!(core.region(Region::Hram).unwrap().bytes[0x7E], 2);
}

#[test]
fn a_rom_patch_is_what_the_cpu_reads() {
    let mut core = quick_dmg(BUSY);
    core.patch_rom(0x7000, &[0xAB, 0xCD]).unwrap();
    assert_eq!(
        &core.region(Region::Rom).unwrap().bytes[0x7000..0x7002],
        [0xAB, 0xCD]
    );
    assert_eq!(core.bus_read(0x7001).unwrap(), 0xCD);
}

#[test]
fn a_rom_patch_to_the_header_is_refused() {
    let mut core = quick_dmg(BUSY);
    for offset in [0x100, 0x147, 0x14F, 0xFF] {
        let len = if offset == 0xFF { 2 } else { 1 };
        let err = core.patch_rom(offset, &vec![0; len]).unwrap_err();
        assert!(
            matches!(err, CoreError::InvalidState { .. }),
            "{offset:#x}: {err:?}"
        );
    }
    core.patch_rom(0x150, &[0xF3]).unwrap();
}

#[test]
fn a_rom_patch_past_the_end_is_out_of_range() {
    let mut core = quick_dmg(BUSY);
    let err = core.patch_rom(0x7FFF, &[0, 0]).unwrap_err();
    assert!(matches!(err, CoreError::OutOfRange { .. }), "{err:?}");
}

#[test]
fn registers_round_trip_including_ime_and_halt() {
    let mut core = quick_dmg(BUSY);
    let regs = Registers {
        a: 0x12,
        f: 0xB0,
        b: 0x34,
        c: 0x56,
        d: 0x78,
        e: 0x9A,
        h: 0xBC,
        l: 0xDE,
        sp: 0xDFF0,
        pc: 0x0150,
        ime: true,
        halted: true,
        double_speed: false,
    };
    core.set_registers(&regs);
    assert_eq!(core.registers(), regs);
}

#[test]
fn held_keys_show_in_joyp() {
    let mut core = quick_dmg(BUSY);
    core.bus_write(0xFF00, 0x10).unwrap(); // select the button row
    // SameBoy applies a key change on the next emulated cycle, not at once.
    core.set_keys(Keys::A | Keys::START);
    core.step(StepUnit::Instruction).unwrap();
    assert_eq!(core.io_register(IoReg::JOYP).unwrap() & 0x0F, 0b0110);
    core.set_keys(Keys::NONE);
    core.step(StepUnit::Instruction).unwrap();
    assert_eq!(core.io_register(IoReg::JOYP).unwrap() & 0x0F, 0b1111);
}
