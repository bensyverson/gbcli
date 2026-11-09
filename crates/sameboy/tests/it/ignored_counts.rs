//! The ignored-write log's count equals the bytes that didn't land, on DMG,
//! CGB single and double speed, and the built-in boot ROMs log nothing.
//! First written as review rev-hw's probes of leaf si5fnG.

use crate::common;

use common::*;
use gbcli_core::{Budget, Core, CoreConfig, IgnoredWriteKind, Model, Region};
use gbcli_sameboy::SameBoy;

const SPEED_SWITCH: &[u8] = &[0x3E, 0x01, 0xE0, 0x4D, 0x10, 0x00];
const LCD_ON: &[u8] = &[0x3E, 0x91, 0xE0, 0x40];
/// ld hl,$8000; ld bc,$0800; .l: ld a,$A5; ld [hl+],a; dec bc; ld a,b; or c; jr nz,.l
const FILL_VRAM: &[u8] = &[
    0x21, 0x00, 0x80, 0x01, 0x00, 0x08, 0x3E, 0xA5, 0x22, 0x0B, 0x78, 0xB1, 0x20, 0xF8,
];
/// ld hl,$FE00; ld b,160; .l: ld a,$A5; ld [hl+],a; dec b; jr nz,.l
const FILL_OAM: &[u8] = &[
    0x21, 0x00, 0xFE, 0x06, 0xA0, 0x3E, 0xA5, 0x22, 0x05, 0x20, 0xFA,
];
const WAIT_VBLANK: &[u8] = &[0xF0, 0x44, 0xFE, 0x90, 0x20, 0xFA];
/// xor a; ldh [rLCDC],a
const LCD_OFF: &[u8] = &[0xAF, 0xE0, 0x40];
const IDLE: &[u8] = &[0x18, 0xFE];

fn rom(parts: &[&[u8]], cgb: bool) -> Vec<u8> {
    let mut rom = BUSY.to_vec();
    let code: Vec<u8> = parts.concat();
    rom[0x150..0x150 + code.len()].copy_from_slice(&code);
    if cgb {
        rom[0x143] = 0x80;
        let mut sum: u8 = 0;
        for b in &rom[0x134..0x14D] {
            sum = sum.wrapping_sub(*b).wrapping_sub(1);
        }
        rom[0x14D] = sum;
    }
    rom
}

fn check(
    mut core: SameBoy,
    region: Region,
    len: usize,
    kind: IgnoredWriteKind,
    expect_speed: bool,
) {
    let size = core.region(region).unwrap().bytes.len();
    core.poke(region, 0, &vec![0u8; size]).unwrap();
    core.set_ignored_write_log(true).unwrap();
    run(&mut core, Budget::frames(10));
    assert_eq!(core.registers().double_speed, expect_speed, "speed");
    let found = core.take_ignored_writes().unwrap();
    let bytes = core.region(region).unwrap().bytes;
    let missing = bytes[..len].iter().filter(|b| **b != 0xA5).count() as u64;
    let counted = found
        .iter()
        .filter(|f| f.kind == kind)
        .map(|f| f.count)
        .sum::<u64>();
    eprintln!("{region:?} speed={expect_speed}: missing {missing}, counted {counted} {found:?}");
    assert!(missing > 0, "the probe must drop some writes");
    assert_eq!(missing, counted);
}

#[test]
fn dmg_vram_count_equals_bytes_not_landed() {
    let r = rom(&[LCD_ON, FILL_VRAM, WAIT_VBLANK, LCD_OFF, IDLE], false);
    check(
        quick_dmg(&r),
        Region::Vram,
        0x800,
        IgnoredWriteKind::Vram,
        false,
    );
}

#[test]
fn cgb_vram_count_equals_bytes_not_landed() {
    let r = rom(&[LCD_ON, FILL_VRAM, WAIT_VBLANK, LCD_OFF, IDLE], true);
    let core = quick_boot(CoreConfig::new(Model::CgbE, r));
    check(core, Region::Vram, 0x800, IgnoredWriteKind::Vram, false);
}

#[test]
fn cgb_double_speed_vram_count_equals_bytes_not_landed() {
    let r = rom(
        &[SPEED_SWITCH, LCD_ON, FILL_VRAM, WAIT_VBLANK, LCD_OFF, IDLE],
        true,
    );
    let core = quick_boot(CoreConfig::new(Model::CgbE, r));
    check(core, Region::Vram, 0x800, IgnoredWriteKind::Vram, true);
}

#[test]
fn cgb_oam_count_equals_bytes_not_landed() {
    let r = rom(&[LCD_ON, FILL_OAM, WAIT_VBLANK, LCD_OFF, IDLE], true);
    let core = quick_boot(CoreConfig::new(Model::CgbE, r));
    check(core, Region::Oam, 160, IgnoredWriteKind::Oam, false);
}

#[test]
fn cgb_double_speed_oam_count_equals_bytes_not_landed() {
    let r = rom(
        &[SPEED_SWITCH, LCD_ON, FILL_OAM, WAIT_VBLANK, LCD_OFF, IDLE],
        true,
    );
    let core = quick_boot(CoreConfig::new(Model::CgbE, r));
    check(core, Region::Oam, 160, IgnoredWriteKind::Oam, true);
}

#[test]
fn built_in_boot_roms_log_nothing() {
    for model in [Model::DmgB, Model::CgbE, Model::Sgb] {
        let r = rom(&[IDLE], false);
        let mut core = SameBoy::new(CoreConfig::new(model, r)).unwrap();
        core.set_ignored_write_log(true).unwrap();
        run(&mut core, Budget::frames(400));
        assert!(core.registers().pc >= 0x100, "{model:?} booted");
        let found = core.take_ignored_writes().unwrap();
        assert!(found.is_empty(), "{model:?}: {found:?}");
    }
}
