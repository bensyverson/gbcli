//! A flash board as a property of the machine (leaf 4hM0UN): the header
//! names it, a restore refuses another board's state, a power cycle keeps
//! or imports the chip, and a mask-ROM machine says which board a program's
//! flash commands are for.

use gbcli_core::flash::{BusyTicks, FlashProfile, NoteKind};
use gbcli_core::{Budget, CoreConfig, Model, Region};
use gbcli_engine::{
    Backend, Battery, CheckpointError, Flash, LoadOptions, Machine, RunOptions, SaveMeta,
    flash_warnings,
};
use gbcli_protocol::ErrorCode;

/// Programs $42 into bank 12 with GB Studio's swapped-data sequence, then
/// counts it in wSaved ($C000) once it reads back (`roms/flash_save.asm`).
const FLASH_SAVE: &[u8] = include_bytes!("roms/flash_save.gb");
/// Bank 12's first byte in the chip.
const SAVED_BYTE: usize = 0x3_0000;
/// Through the boot ROM (about 2.5 s on DMG) and the save.
const BOOT_AND_SAVE: u64 = 200;

fn machine(flash: Option<&str>) -> Machine {
    let mut config = CoreConfig::new(Model::DmgB, FLASH_SAVE.to_vec());
    config.flash = flash.map(|name| FlashProfile::named(name).expect("named"));
    Machine::new(Backend::sameboy(), config, None).expect("machine")
}

fn run(m: &mut Machine, frames: u64) {
    m.run(&RunOptions::new(Budget::frames(frames)))
        .expect("run");
}

fn saved(m: &Machine) -> u8 {
    m.core().region(Region::Wram).unwrap().bytes[0]
}

fn chip_byte(m: &Machine) -> u8 {
    m.core().region(Region::Rom).unwrap().bytes[SAVED_BYTE]
}

fn meta() -> SaveMeta {
    SaveMeta {
        created_by: None,
        parent: None,
        rev: None,
    }
}

#[test]
fn the_header_names_the_board_and_mask_rom_names_none() {
    let mut flash = machine(Some("generic-wr-aaa-a9"));
    assert_eq!(
        flash.capture(meta()).header.flash.as_deref(),
        Some("generic-wr-aaa-a9")
    );
    let mut mask = machine(None);
    assert_eq!(mask.capture(meta()).header.flash, None);
}

#[test]
fn a_state_from_another_board_or_mask_rom_is_refused_as_a_bad_request() {
    let from_a9 = machine(Some("generic-wr-aaa-a9")).capture(meta());
    let from_mask = machine(None).capture(meta());
    let cases = [
        (machine(None), &from_a9),
        (machine(Some("generic-wr-aaa-aa")), &from_a9),
        (machine(Some("generic-wr-aaa-a9")), &from_mask),
    ];
    for (mut m, cp) in cases {
        let err = m.restore(cp, &LoadOptions::default()).unwrap_err();
        assert!(
            matches!(err, CheckpointError::FlashConflict { .. }),
            "{err:?}"
        );
        assert_eq!(err.code(), ErrorCode::BadRequest);
        assert!(
            err.remedy().contains("--flash") || err.remedy().contains("session"),
            "{}",
            err.remedy()
        );
    }
    let mut same = machine(Some("generic-wr-aaa-a9"));
    same.restore(&from_a9, &LoadOptions::default())
        .expect("same board");
}

#[test]
fn the_save_lands_and_a_power_cycle_keeps_it() {
    let mut m = machine(Some("generic-wr-aaa-a9"));
    run(&mut m, BOOT_AND_SAVE);
    assert_eq!(saved(&m), 1, "the program read back");
    assert_eq!(chip_byte(&m), 0x42);
    m.power_cycle(Battery::Keep, Flash::Keep, 0).unwrap();
    assert_eq!(chip_byte(&m), 0x42);
}

#[test]
fn a_power_cycle_imports_a_chip_image_and_reports_sectors_inside_the_image() {
    let mut source = machine(Some("generic-wr-aaa-a9"));
    run(&mut source, BOOT_AND_SAVE);
    let mut image = source.flash_image().expect("a flash cart");
    assert_eq!(image.len(), 0x20_0000, "the chip's size");
    image[0x100] ^= 0xFF; // inside the image: in bank 0's 64 KiB sector

    let mut m = machine(Some("generic-wr-aaa-a9"));
    let cycled = m
        .power_cycle(Battery::Keep, Flash::Image(&image), 0)
        .unwrap();
    assert_eq!(chip_byte(&m), 0x42, "the save sector came in");
    assert_eq!(cycled.flash_bytes, Some(image.len()));
    assert_eq!(cycled.flash_sectors_in_image, 1);
    let wrong = vec![0xFF; 0x1000];
    assert!(
        m.power_cycle(Battery::Keep, Flash::Image(&wrong), 0)
            .is_err()
    );
    assert_eq!(chip_byte(&m), 0x42, "a refused import changes nothing");
    assert!(machine(None).flash_image().is_none());
}

#[test]
fn a_power_cycle_carries_another_cores_flash_contents() {
    let mut source = machine(Some("generic-wr-aaa-a9"));
    run(&mut source, BOOT_AND_SAVE);
    let contents = source.core_mut().flash_contents().unwrap().unwrap();
    let mut m = machine(Some("generic-wr-aaa-a9"));
    m.power_cycle(Battery::Keep, Flash::Carried(&contents), 0)
        .unwrap();
    assert_eq!(chip_byte(&m), 0x42);
}

#[test]
fn a_mask_rom_machine_names_the_board_its_program_unlocks() {
    let mut m = machine(None);
    run(&mut m, BOOT_AND_SAVE);
    let notes = m.core_mut().take_flash_notes().unwrap();
    assert_eq!(
        notes.notes.iter().map(|n| n.kind).collect::<Vec<_>>(),
        [NoteKind::NoChip {
            board: "generic-wr-aaa-a9"
        }],
        "one note, for the first unlock cycle"
    );
    let warnings = flash_warnings(&notes);
    assert_eq!(warnings.len(), 1);
    assert!(
        warnings[0].contains("--flash generic-wr-aaa-a9"),
        "{}",
        warnings[0]
    );
}

#[test]
fn a_board_that_misses_the_commands_says_why() {
    let mut profile = FlashProfile::named("generic-wr-aaa-aa").unwrap();
    profile.busy = BusyTicks::INSTANT;
    let mut config = CoreConfig::new(Model::DmgB, FLASH_SAVE.to_vec());
    config.flash = Some(profile);
    let mut m = Machine::new(Backend::sameboy(), config, None).unwrap();
    run(&mut m, BOOT_AND_SAVE);
    assert_eq!(saved(&m), 0);
    let warnings = flash_warnings(&m.core_mut().take_flash_notes().unwrap());
    assert!(warnings[0].contains("D0 and D1"), "{warnings:?}");
}
