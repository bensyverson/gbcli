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
const FLASH_SAVE: &[u8] = include_bytes!("../roms/flash_save.gb");
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
    let cycled = m
        .power_cycle(Battery::Keep, Flash::Carried(&contents), 0)
        .unwrap();
    assert_eq!(chip_byte(&m), 0x42);
    assert!(
        cycled.flash_sectors_dropped.is_empty(),
        "the save lies past the image: {:?}",
        cycled.flash_sectors_dropped
    );
}

/// A rebuild that grows the ROM into the sector the old build saved in: the
/// new build's code stays there, and the sector is named as dropped (the
/// 2026-11-07 ruling on the review of fc85d19).
#[test]
fn a_rebuild_grown_into_the_carried_save_keeps_its_code_and_names_the_sector() {
    let mut old = machine(Some("generic-wr-aaa-a9"));
    run(&mut old, BOOT_AND_SAVE);
    let contents = old.core_mut().flash_contents().unwrap().unwrap();

    let mut grown = FLASH_SAVE.to_vec();
    grown.resize(0x4_0000, 0xC9); // banks 8-15: the new build's code, all RET
    grown[0x148] = 0x03; // 256 KiB
    let mut config = CoreConfig::new(Model::DmgB, grown.clone());
    config.flash = Some(FlashProfile::named("generic-wr-aaa-a9").unwrap());
    let mut new = Machine::new(Backend::sameboy(), config, None).unwrap();
    let cycled = new
        .power_cycle(Battery::Keep, Flash::Carried(&contents), 0)
        .unwrap();
    let chip = &new.core().region(Region::Rom).unwrap().bytes;
    let differing = (SAVED_BYTE..grown.len())
        .filter(|&i| chip[i] != grown[i])
        .count();
    assert_eq!(differing, 0, "bytes of the new build's code replaced");
    let sector = std::ops::Range {
        start: 0x3_0000,
        end: 0x4_0000,
    };
    assert_eq!(cycled.flash_sectors_dropped, vec![sector]);
}

/// FLASH_SAVE grown to 256 KiB: banks 8-15, the sector the old build saved
/// in included, are the new build's code (all RET).
fn grown() -> Vec<u8> {
    let mut grown = FLASH_SAVE.to_vec();
    grown.resize(0x4_0000, 0xC9);
    grown[0x148] = 0x03; // 256 KiB
    grown
}

fn machine_over(rom: Vec<u8>, sym: Option<&str>) -> Machine {
    let mut config = CoreConfig::new(Model::DmgB, rom);
    config.flash = Some(FlashProfile::named("generic-wr-aaa-a9").unwrap());
    Machine::new(Backend::sameboy(), config, sym).unwrap()
}

/// How many bytes of `rom` from the save's sector on the chip no longer holds.
fn code_replaced(m: &Machine, rom: &[u8]) -> usize {
    let chip = &m.core().region(Region::Rom).unwrap().bytes;
    (SAVED_BYTE..rom.len())
        .filter(|&i| chip[i] != rom[i])
        .count()
}

fn dropped(warnings: &[gbcli_engine::LoadWarning]) -> Vec<std::ops::Range<u32>> {
    warnings
        .iter()
        .find_map(|w| match w {
            gbcli_engine::LoadWarning::FlashSectorsDropped { sectors, .. } => Some(sectors.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

const FLASH_SAVE_SYM: &str = include_str!("../roms/flash_save.sym");

/// A compatible rebuild (no live symbol moved) grown into the sector the old
/// build saved in: the restore every op makes of the head keeps the new
/// build's code there and names the sector (the 2026-11-07 ruling on the
/// review of 6c5b029, the power cycle's rule).
#[test]
fn a_compatible_restore_over_a_grown_build_keeps_its_code_and_names_the_sector() {
    let mut old = machine_over(FLASH_SAVE.to_vec(), Some(FLASH_SAVE_SYM));
    run(&mut old, BOOT_AND_SAVE);
    let saved = old.capture(meta());
    let mut new = machine_over(grown(), Some(FLASH_SAVE_SYM));
    let warnings = new.restore(&saved, &LoadOptions::default()).unwrap();
    assert_eq!(code_replaced(&new, &grown()), 0, "{warnings:?}");
    assert_eq!(dropped(&warnings), vec![0x3_0000..0x4_0000], "{warnings:?}");
}

/// The same crossing forced with `--allow-rom-change`.
#[test]
fn a_forced_restore_over_a_grown_build_keeps_its_code() {
    let mut old = machine(Some("generic-wr-aaa-a9"));
    run(&mut old, BOOT_AND_SAVE);
    let saved = old.capture(meta());
    let mut new = machine_over(grown(), None);
    let forced = LoadOptions {
        allow_rom_change: true,
        ..LoadOptions::default()
    };
    let warnings = new.restore(&saved, &forced).unwrap();
    assert_eq!(code_replaced(&new, &grown()), 0, "{warnings:?}");
    assert_eq!(dropped(&warnings), vec![0x3_0000..0x4_0000], "{warnings:?}");
}

/// A rebuild that stays clear of the save: the restore keeps it, and says
/// nothing about flash.
#[test]
fn a_restore_across_a_rebuild_keeps_the_save_past_the_image() {
    let mut old = machine_over(FLASH_SAVE.to_vec(), Some(FLASH_SAVE_SYM));
    run(&mut old, BOOT_AND_SAVE);
    let saved = old.capture(meta());
    let mut rebuilt = FLASH_SAVE.to_vec();
    rebuilt[0x1_C001] = 0x77; // in the last bank, past the code
    let mut new = machine_over(rebuilt, Some(FLASH_SAVE_SYM));
    let warnings = new.restore(&saved, &LoadOptions::default()).unwrap();
    assert_eq!(chip_byte(&new), 0x42, "{warnings:?}");
    assert_eq!(dropped(&warnings), Vec::new(), "{warnings:?}");
}

/// FLASH_SAVE `rgbfix -p 0xFF`-padded to 256 KiB, so the save's sector
/// (banks 12-15) lies in the image, all $FF; and that build with a byte
/// changed elsewhere, as a rebuild that touches only code.
fn padded_and_rebuilt() -> (Vec<u8>, Vec<u8>) {
    let mut padded = FLASH_SAVE.to_vec();
    padded.resize(0x4_0000, 0xFF);
    padded[0x148] = 0x03; // 256 KiB
    let mut rebuilt = padded.clone();
    rebuilt[0x1_C001] = 0x77; // in bank 7, far from the save
    (padded, rebuilt)
}

/// A save in the image's $FF padding is not the new build's code: a
/// compatible rebuild elsewhere keeps it (the integrator's refinement of the
/// 2026-11-07 ruling, leaf ahKOkk).
#[test]
fn a_restore_across_a_rebuild_keeps_a_save_in_the_images_padding() {
    let (padded, rebuilt) = padded_and_rebuilt();
    let mut old = machine_over(padded, Some(FLASH_SAVE_SYM));
    run(&mut old, BOOT_AND_SAVE);
    assert_eq!(chip_byte(&old), 0x42, "the old build saved");
    let saved = old.capture(meta());
    let mut new = machine_over(rebuilt, Some(FLASH_SAVE_SYM));
    let warnings = new.restore(&saved, &LoadOptions::default()).unwrap();
    assert_eq!(chip_byte(&new), 0x42, "{warnings:?}");
    assert_eq!(dropped(&warnings), Vec::new(), "{warnings:?}");
}

/// The same crossing by a power cycle.
#[test]
fn a_power_cycle_across_a_rebuild_keeps_a_save_in_the_images_padding() {
    let (padded, rebuilt) = padded_and_rebuilt();
    let mut old = machine_over(padded, None);
    run(&mut old, BOOT_AND_SAVE);
    let contents = old.core_mut().flash_contents().unwrap().unwrap();
    let mut new = machine_over(rebuilt, None);
    let cycled = new
        .power_cycle(Battery::Keep, Flash::Carried(&contents), 0)
        .unwrap();
    assert_eq!(chip_byte(&new), 0x42);
    assert_eq!(cycled.flash_sectors_dropped, Vec::new());
}

#[test]
fn a_flash_report_holds_each_note_as_a_warning_and_a_row_once() {
    let mut m = machine(Some("generic-wr-aaa-aa"));
    run(&mut m, BOOT_AND_SAVE);
    let report = m.take_flash_report().unwrap();
    let rows = report.rows.expect("rows");
    assert_eq!(rows.len(), report.warnings.len());
    assert!(
        report.warnings[0].contains("D0 and D1"),
        "{:?}",
        report.warnings
    );
    assert_eq!(rows[0].text, report.warnings[0]);
    let again = m.take_flash_report().unwrap();
    assert_eq!((again.warnings.len(), again.rows), (0, None), "taken once");
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
