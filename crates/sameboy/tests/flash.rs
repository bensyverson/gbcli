//! A flash-save cart on SameBoy (`gbcli_flash`): command writes reach both
//! the chip and the MBC, the chip's array is the ROM the CPU reads, and its
//! state rides in the blob and across a power cycle.

mod common;

use common::cart::*;
use common::quick_boot;
use gbcli_core::flash::{BusyTicks, FlashProfile, NoteKind};
use gbcli_core::{Core, CoreConfig, CoreError, Model, Region};
use gbcli_sameboy::SameBoy;

/// The bank GB Studio saves from.
const SAVE_BANK: u8 = 12;

fn board(name: &str, busy: BusyTicks) -> FlashProfile {
    FlashProfile {
        busy,
        ..FlashProfile::named(name).expect("named profile")
    }
}

/// An MBC5 cart of 8 banks (128 KiB, as GB Studio's demo) on `profile`.
fn flash_core(profile: FlashProfile) -> SameBoy {
    let mut config = CoreConfig::new(Model::DmgB, cart(MBC5_RAM_BATTERY, 8, RAM_32K));
    config.flash = Some(profile);
    quick_boot(config)
}

/// gbvm's byte program, then the byte, through the bus.
fn program(core: &mut SameBoy, addr: u16, value: u8) {
    for (a, v) in [
        (0x0AAA, 0xA9),
        (0x0555, 0x56),
        (0x0AAA, 0xA0),
        (addr, value),
    ] {
        poke_bus(core, a, v);
    }
}

fn select_save_bank(core: &mut SameBoy) {
    poke_bus(core, 0x2000, SAVE_BANK);
}

#[test]
fn the_chip_is_the_boards_size_with_the_rom_at_its_start() {
    let core = flash_core(board("generic-wr-aaa-a9", BusyTicks::INSTANT));
    let rom = core.region(Region::Rom).unwrap().bytes;
    assert_eq!(rom.len(), 0x20_0000);
    assert_eq!(&rom[0x4000..0x4002], &[1, 0], "bank 1's tag");
    assert!(
        rom[0x2_0000..].iter().all(|&b| b == 0xFF),
        "erased past the image"
    );
}

#[test]
fn a_rom_larger_than_the_chip_is_refused() {
    let mut profile = board("generic-wr-aaa-a9", BusyTicks::INSTANT);
    profile.size = 0x1_0000;
    let mut config = CoreConfig::new(Model::DmgB, cart(MBC5_RAM_BATTERY, 8, RAM_32K));
    config.flash = Some(profile);
    let err = SameBoy::new(config).err().expect("refused");
    assert!(err.to_string().contains("0x10000"), "{err}");
}

#[test]
fn gbstudios_program_lands_in_the_save_bank_past_the_image() {
    let mut core = flash_core(board("generic-wr-aaa-a9", BusyTicks::INSTANT));
    select_save_bank(&mut core);
    assert_eq!(core.region(Region::Rom).unwrap().bank, u16::from(SAVE_BANK));
    program(&mut core, 0x4010, 0x5A);
    assert_eq!(core.bus_read(0x4010).unwrap(), 0x5A);
    let rom = core.region(Region::Rom).unwrap().bytes;
    assert_eq!(rom[0x3_0010], 0x5A);
    assert_eq!(
        rom[0x1_0010], 4,
        "bank 4, which a 128 KiB chip would alias, is intact"
    );
}

#[test]
fn command_writes_also_reach_the_mbc() {
    let mut core = flash_core(board("generic-wr-aaa-a9", BusyTicks::INSTANT));
    poke_bus(&mut core, 0x0000, 0x0A); // RAM on
    poke_bus(&mut core, 0xA000, 0x77);
    assert_eq!(core.bus_read(0xA000).unwrap(), 0x77);
    poke_bus(&mut core, 0x0AAA, 0xA9); // also an MBC RAM-enable write: off
    // Disabled cart RAM reads open bus (SameBoy: the last byte on the bus).
    assert_ne!(
        core.bus_read(0xA000).unwrap(),
        0x77,
        "RAM disabled by the unlock cycle"
    );
}

#[test]
fn a_busy_chip_answers_reads_with_status() {
    let mut core = flash_core(board("generic-wr-aaa-a9", BusyTicks::TYPICAL));
    select_save_bank(&mut core);
    program(&mut core, 0x4000, 0x80);
    let first = core.bus_read(0x4000).unwrap();
    let second = core.bus_read(0x4000).unwrap();
    assert_eq!(first & 0x80, 0, "DQ7 inverted");
    assert_ne!(first & 0x40, second & 0x40, "DQ6 toggles");
}

#[test]
fn a_checkpoint_restores_the_array_and_the_busy_state() {
    let profile = board("generic-wr-aaa-a9", BusyTicks::TYPICAL);
    let mut core = flash_core(profile.clone());
    select_save_bank(&mut core);
    program(&mut core, 0x4000, 0x80);
    let blob = core.save();
    let after: Vec<u8> = (0..3).map(|_| core.bus_read(0x4000).unwrap()).collect();

    let mut other = flash_core(profile);
    other.load(&blob).expect("load");
    let again: Vec<u8> = (0..3).map(|_| other.bus_read(0x4000).unwrap()).collect();
    assert_eq!(again, after);
    assert_eq!(other.region(Region::Rom).unwrap().bytes[0x3_0000], 0x80);
}

#[test]
fn a_load_undoes_flash_writes_made_after_the_save() {
    let mut core = flash_core(board("generic-wr-aaa-a9", BusyTicks::INSTANT));
    let blob = core.save();
    select_save_bank(&mut core);
    program(&mut core, 0x4000, 0x12);
    core.load(&blob).expect("load");
    assert_eq!(core.region(Region::Rom).unwrap().bytes[0x3_0000], 0xFF);
}

#[test]
fn flash_and_mask_rom_states_dont_mix() {
    let mut flash = flash_core(board("generic-wr-aaa-a9", BusyTicks::INSTANT));
    let mut mask = common::cart::core(cart(MBC5_RAM_BATTERY, 8, RAM_32K));
    let (flash_blob, mask_blob) = (flash.save(), mask.save());
    assert!(matches!(
        mask.load(&flash_blob),
        Err(CoreError::InvalidState { .. })
    ));
    assert!(matches!(
        flash.load(&mask_blob),
        Err(CoreError::InvalidState { .. })
    ));
    let mut straight = flash_core(board("generic-wr-aaa-aa", BusyTicks::INSTANT));
    let err = straight.load(&flash_blob).unwrap_err();
    assert!(err.to_string().contains("generic-wr-aaa-a9"), "{err}");
}

#[test]
fn flash_contents_carry_into_a_fresh_core() {
    let profile = board("generic-wr-aaa-a9", BusyTicks::INSTANT);
    let mut core = flash_core(profile.clone());
    select_save_bank(&mut core);
    program(&mut core, 0x4000, 0x42);
    let contents = core.flash_contents().unwrap().expect("a flash cart");

    let mut fresh = flash_core(profile);
    fresh.load_flash_contents(&contents).unwrap();
    assert_eq!(fresh.region(Region::Rom).unwrap().bytes[0x3_0000], 0x42);
    let mut mask = common::cart::core(cart(MBC5_RAM_BATTERY, 8, RAM_32K));
    assert_eq!(mask.flash_contents().unwrap(), None);
    assert!(mask.load_flash_contents(&contents).is_err());
}

#[test]
fn a_straight_board_notes_gbstudios_swapped_commands() {
    let mut core = flash_core(board("generic-wr-aaa-aa", BusyTicks::INSTANT));
    select_save_bank(&mut core);
    program(&mut core, 0x4000, 0x12);
    assert_eq!(core.bus_read(0x4000).unwrap(), 0xFF);
    let notes = core.take_flash_notes().unwrap().notes;
    assert!(
        matches!(
            notes.first().map(|n| n.kind),
            Some(NoteKind::DataLines { .. })
        ),
        "{notes:?}"
    );
}
