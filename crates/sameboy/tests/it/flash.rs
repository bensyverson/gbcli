//! A flash-save cart on SameBoy (`gbcli_flash`): command writes reach both
//! the chip and the MBC, the chip's array is the ROM the CPU reads, and its
//! state rides in the blob and across a power cycle.

use crate::common;

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
fn a_chip_size_that_isnt_a_power_of_two_is_refused() {
    let mut profile = board("generic-wr-aaa-a9", BusyTicks::INSTANT);
    profile.size = 0x30_0000;
    let mut config = CoreConfig::new(Model::DmgB, cart(MBC5_RAM_BATTERY, 8, RAM_32K));
    config.flash = Some(profile);
    let err = SameBoy::new(config).err().expect("refused");
    assert!(err.to_string().contains("power of two"), "{err}");
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

/// Padding the image to the chip must not change SameBoy's mapper choice,
/// which reads the buffer's size: over 2 MiB, an MBC3 would become an MBC30
/// with an 8-bit bank register.
#[test]
fn an_mbc3_image_on_a_4mib_board_stays_an_mbc3() {
    let rom = cart(MBC3, 8, NO_RAM);
    let mut flash = on_board(rom, board("mx29lv320et", BusyTicks::INSTANT));
    poke_bus(&mut flash, 0x2000, 0x81);
    assert_eq!(
        flash.region(Region::Rom).unwrap().bank,
        1,
        "an MBC3 masks the bank to 7 bits"
    );
    poke_bus(&mut flash, 0x2000, 0x40);
    assert_eq!(
        flash.bus_read(0x4000).unwrap(),
        0xFF,
        "bank $40 is erased flash past the image, not the image's bank 0"
    );
}

/// Over 32 KiB, SameBoy takes a ROM-only header for an MBC3; the image is
/// 32 KiB, so the cart must stay without a mapper (and reach only 32 KiB).
#[test]
fn a_rom_only_image_on_a_board_gains_no_mapper() {
    let mut flash = on_board(
        cart(ROM_ONLY, 2, NO_RAM),
        board("generic-wr-aaa-a9", BusyTicks::INSTANT),
    );
    poke_bus(&mut flash, 0x2000, 0x03);
    assert_eq!(tag_at(&mut flash, 0x4000), 1, "no bank register");
}

/// SameBoy finds an MMM01's header bank at the end of the buffer, which the
/// chip's erased tail would hide; a multicart mapper with a flash save is
/// not a board the model knows.
#[test]
fn an_mmm01_image_on_a_board_is_refused() {
    let mut config = CoreConfig::new(Model::DmgB, cart(MMM01, 8, NO_RAM));
    config.flash = Some(board("generic-wr-aaa-a9", BusyTicks::INSTANT));
    let err = SameBoy::new(config).err().expect("refused");
    assert!(err.to_string().contains("MMM01"), "{err}");
}

/// The chip ignores A11 and up in command cycles, so unlock cycles at
/// $4AAA/$4555 work whichever bank is mapped, not only on even ones.
#[test]
fn unlock_cycles_in_the_4000_window_dont_depend_on_the_bank() {
    for bank in 1u8..=4 {
        let mut c = on_board(
            cart(MBC5, 8, NO_RAM),
            board("generic-wr-aaa-aa", BusyTicks::INSTANT),
        );
        poke_bus(&mut c, 0x2000, bank);
        for (a, v) in [(0x4AAA, 0xAA), (0x4555, 0x55), (0x4AAA, 0xA0)] {
            poke_bus(&mut c, a, v);
        }
        poke_bus(&mut c, 0x4100, 0x00); // over the bank's nonzero tag
        assert_eq!(c.bus_read(0x4100).unwrap(), 0, "bank {bank}");
        let notes = c.take_flash_notes().unwrap().notes;
        assert!(notes.is_empty(), "bank {bank}: {notes:?}");
    }
}

/// A load SameBoy refuses leaves the core as it was, the chip's pending
/// notes included.
#[test]
fn a_refused_load_keeps_the_pending_notes() {
    let mut core = flash_core(board("generic-wr-aaa-aa", BusyTicks::INSTANT));
    let blob = core.save();
    poke_bus(&mut core, 0x0AAA, 0xA9); // a data-lines note on a straight board
    let err = core.load(&corrupt_sameboy_section(&blob)).unwrap_err();
    assert!(err.to_string().contains("SameBoy refused"), "{err}");
    let notes = core.take_flash_notes().unwrap().notes;
    assert_eq!(notes.len(), 1, "{notes:?}");
}

fn on_board(rom: Vec<u8>, profile: FlashProfile) -> SameBoy {
    let mut config = CoreConfig::new(Model::DmgB, rom);
    config.flash = Some(profile);
    quick_boot(config)
}

/// The blob with SameBoy's own section overwritten. The blob ends
/// `u64 len | sameboy | u8 1 | u64 n | "GBFL"...`.
fn corrupt_sameboy_section(blob: &[u8]) -> Vec<u8> {
    let gbfl = blob.windows(4).rposition(|w| w == b"GBFL").expect("flash");
    let flag = gbfl - 9;
    assert_eq!(blob[flag], 1, "the flash flag");
    let start = (0..flag - 8)
        .rev()
        .find(|&p| {
            let n = u64::from_le_bytes(blob[p..p + 8].try_into().unwrap());
            n.checked_add(p as u64 + 8) == Some(flag as u64)
        })
        .expect("SameBoy's section length");
    let mut bad = blob.to_vec();
    bad[start + 8..flag].fill(0xEE);
    bad
}
