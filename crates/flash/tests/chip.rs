//! The flash model against the sequences programs send: GB Studio's
//! batteryless routines (gbvm `src/core/gb/flasher_s.s` at 4f01d26) and the
//! AMD command set.

use gbcli_flash::{
    BusyTicks, DataLines, FlashChip, FlashProfile, NoteKind, Operation, SectorRun, WritePin,
    contents_past,
};

const ROM_SIZE: usize = 0x40000;
/// The bank GB Studio's save starts at in these tests (`_start_save`).
const SAVE_BANK: u32 = 12;

/// A 256 KiB ROM: code-like bytes, and the save sector (banks 12-15) erased.
fn rom() -> Vec<u8> {
    let mut rom: Vec<u8> = (0..ROM_SIZE).map(|i| (i * 7 + 3) as u8).collect();
    rom[0x30000..].fill(0xFF);
    rom
}

/// The chip address of a bus address with `bank` mapped at $4000.
fn chip_addr(bank: u32, bus: u16) -> u32 {
    if bus < 0x4000 {
        u32::from(bus)
    } else {
        bank * 0x4000 + u32::from(bus - 0x4000)
    }
}

fn profile(name: &str) -> FlashProfile {
    FlashProfile::named(name).expect("named profile")
}

/// A chip plus the ROM and the bank mapped at $4000, as a core drives them.
struct Cart {
    chip: FlashChip,
    rom: Vec<u8>,
    bank: u32,
    now: u64,
}

impl Cart {
    fn new(profile: FlashProfile) -> Cart {
        Cart {
            chip: FlashChip::new(profile),
            rom: rom(),
            bank: 1,
            now: 1000,
        }
    }

    fn write(&mut self, bus: u16, value: u8) {
        self.now += 16;
        let addr = chip_addr(self.bank, bus);
        self.chip.write(self.now, bus, addr, value, &mut self.rom);
    }

    fn read(&mut self, bus: u16) -> u8 {
        self.now += 16;
        let addr = chip_addr(self.bank, bus);
        let data = self.rom[addr as usize];
        self.chip.read(self.now, addr, data)
    }

    fn wait(&mut self, ticks: u64) {
        self.now += ticks;
    }

    /// gbvm's byte program: `$0AAA<-$A9, $0555<-$56, $0AAA<-$A0`, the byte.
    fn gbstudio_program(&mut self, bus: u16, value: u8) {
        self.write(0x0AAA, 0xA9);
        self.write(0x0555, 0x56);
        self.write(0x0AAA, 0xA0);
        self.write(bus, value);
    }

    /// gbvm's sector erase, with the save bank mapped at $4000.
    fn gbstudio_erase(&mut self) {
        self.bank = SAVE_BANK;
        for (bus, value) in [
            (0x4000, 0xF0),
            (0x0AAA, 0xA9),
            (0x0555, 0x56),
            (0x0AAA, 0x80),
            (0x0AAA, 0xA9),
            (0x0555, 0x56),
            (0x4000, 0x30),
        ] {
            self.write(bus, value);
        }
    }
}

#[test]
fn gbstudios_program_sequence_writes_a_byte_on_a_swapped_board() {
    let mut cart = Cart::new(profile("generic-wr-aaa-a9"));
    cart.bank = SAVE_BANK;
    cart.gbstudio_program(0x4010, 0x3C);
    cart.wait(BusyTicks::TYPICAL.program);
    assert_eq!(cart.read(0x4010), 0x3C);
    assert_eq!(cart.rom[0x30010], 0x3C, "the array is the ROM buffer");
    assert!(cart.chip.take_notes().notes.is_empty());
}

#[test]
fn a_busy_program_answers_with_inverted_dq7_and_a_toggling_dq6() {
    let mut cart = Cart::new(profile("generic-wr-aaa-a9"));
    cart.bank = SAVE_BANK;
    cart.gbstudio_program(0x4000, 0x80);
    let first = cart.read(0x4000);
    let second = cart.read(0x4000);
    assert_eq!(
        first & 0x80,
        0,
        "DQ7 is the complement of the programmed bit 7"
    );
    assert_ne!(first & 0x40, second & 0x40, "DQ6 toggles on every read");
    assert!(cart.chip.busy(cart.now));
    cart.wait(BusyTicks::TYPICAL.program);
    assert_eq!(cart.read(0x4000), 0x80);
    assert!(!cart.chip.busy(cart.now));
}

#[test]
fn programming_only_clears_bits_and_says_so() {
    let mut cart = Cart::new(profile("generic-wr-aaa-a9"));
    cart.bank = SAVE_BANK;
    cart.gbstudio_program(0x4000, 0x0F);
    cart.wait(BusyTicks::TYPICAL.program);
    cart.gbstudio_program(0x4000, 0xF1);
    cart.wait(BusyTicks::TYPICAL.program);
    assert_eq!(cart.read(0x4000), 0x01);
    let notes = cart.chip.take_notes().notes;
    assert_eq!(
        notes.iter().map(|n| n.kind).collect::<Vec<_>>(),
        [NoteKind::ProgramSetsBits {
            was: 0x0F,
            wanted: 0xF1
        }]
    );
}

#[test]
fn gbstudios_sector_erase_clears_one_64k_sector_and_polls_to_ff() {
    let mut cart = Cart::new(profile("generic-wr-aaa-a9"));
    cart.rom[0x2FFFF] = 0x00;
    cart.rom[0x30000..].fill(0x00);
    cart.gbstudio_erase();
    let status = cart.read(0x4000);
    assert_ne!(status, 0xFF, "gbvm polls $4000 until it reads $FF");
    assert_eq!(status & 0x80, 0, "DQ7 reads 0 during an erase");
    cart.wait(BusyTicks::TYPICAL.sector_erase);
    assert_eq!(cart.read(0x4000), 0xFF);
    assert!(cart.rom[0x30000..0x40000].iter().all(|&b| b == 0xFF));
    assert_eq!(cart.rom[0x2FFFF], 0x00, "the sector below is untouched");
}

#[test]
fn a_sector_map_erases_only_the_small_boot_sector() {
    let mut cart = Cart::new(profile("29dl163bd-90"));
    cart.rom[..0x10000].fill(0x00);
    cart.bank = 0;
    for (bus, value) in [
        (0x0AAA, 0xA9),
        (0x0555, 0x56),
        (0x0AAA, 0x80),
        (0x0AAA, 0xA9),
        (0x0555, 0x56),
        (0x2000, 0x30),
    ] {
        cart.write(bus, value);
    }
    cart.wait(BusyTicks::TYPICAL.sector_erase);
    assert!(cart.rom[0x2000..0x4000].iter().all(|&b| b == 0xFF));
    assert_eq!(cart.rom[0x1FFF], 0x00);
    assert_eq!(cart.rom[0x4000], 0x00);
}

#[test]
fn sector_maps_fall_back_to_64k_past_their_end() {
    let p = profile("29dl163bd-90");
    assert_eq!(p.sector_of(0x2345, 0x400000), 0x2000..0x4000);
    assert_eq!(p.sector_of(0x12345, 0x400000), 0x10000..0x20000);
    let top = profile("mx29lv320et");
    assert_eq!(top.sector_of(0x3F_E123, 0x400000), 0x3F_E000..0x40_0000);
    let mut custom = profile("generic-wr-aaa-aa");
    custom.sectors = vec![SectorRun {
        size: 0x8000,
        count: 1,
    }];
    assert_eq!(custom.sector_of(0x9000, 0x20000), 0x8000..0x18000);
}

#[test]
fn chip_erase_clears_everything() {
    let mut cart = Cart::new(profile("generic-wr-aaa-aa"));
    for (bus, value) in [
        (0x0AAA, 0xAA),
        (0x0555, 0x55),
        (0x0AAA, 0x80),
        (0x0AAA, 0xAA),
        (0x0555, 0x55),
        (0x0AAA, 0x10),
    ] {
        cart.write(bus, value);
    }
    assert!(cart.chip.busy(cart.now));
    cart.wait(BusyTicks::TYPICAL.chip_erase);
    assert!(cart.rom.iter().all(|&b| b == 0xFF));
}

#[test]
fn autoselect_reads_ids_until_reset() {
    let mut cart = Cart::new(profile("am29lv160db"));
    cart.bank = 1;
    cart.write(0x4AAA, 0xA9);
    cart.write(0x4555, 0x56);
    cart.write(0x4AAA, 0x90);
    let ids: Vec<u8> = (0..4).map(|i| cart.read(i)).collect();
    assert_eq!(ids, [0x02, 0x02, 0x4A, 0x4A]);
    cart.write(0x0000, 0xF0);
    assert_eq!(cart.read(0x0001), cart.rom[1]);
}

#[test]
fn reset_abandons_a_half_sent_command() {
    let mut cart = Cart::new(profile("generic-wr-aaa-a9"));
    cart.bank = SAVE_BANK;
    cart.write(0x0AAA, 0xA9);
    cart.write(0x0555, 0x56);
    cart.write(0x4000, 0xF0);
    cart.write(0x4000, 0x12);
    assert_eq!(
        cart.read(0x4000),
        0xFF,
        "the byte after a reset programs nothing"
    );
}

#[test]
fn gbstudios_sequence_does_nothing_on_a_straight_board_and_says_why() {
    let mut cart = Cart::new(profile("generic-wr-aaa-aa"));
    cart.bank = SAVE_BANK;
    cart.gbstudio_program(0x4000, 0x12);
    cart.wait(BusyTicks::TYPICAL.program);
    assert_eq!(cart.read(0x4000), 0xFF, "nothing was programmed");
    let notes = cart.chip.take_notes().notes;
    let first = notes.first().expect("a note");
    assert_eq!(
        first.kind,
        NoteKind::DataLines {
            seen: 0xA9,
            expected: 0xAA,
            written_for: DataLines::SwappedD0D1,
        }
    );
    let text = first.to_string();
    assert!(text.contains("D0 and D1 swapped"), "{text}");
    assert!(text.contains("$0AAA<-$A9"), "{text}");
}

#[test]
fn an_audio_pin_board_ignores_every_cpu_write_and_says_so() {
    let mut cart = Cart::new(profile("generic-audio-aaa-a9"));
    cart.bank = SAVE_BANK;
    cart.gbstudio_program(0x4000, 0x12);
    cart.wait(BusyTicks::TYPICAL.program);
    assert_eq!(cart.read(0x4000), 0xFF);
    let notes = cart.chip.take_notes().notes;
    assert!(!notes.is_empty());
    assert!(notes.iter().all(|n| n.kind == NoteKind::WritePinAudio));
    assert!(notes[0].to_string().contains("AUDIO pin"));
}

#[test]
fn another_boards_unlock_address_is_named() {
    let mut cart = Cart::new(profile("sst39sf040-audio"));
    cart.chip = FlashChip::new(FlashProfile {
        write_pin: WritePin::Wr,
        ..profile("sst39sf040-audio")
    });
    cart.write(0x0AAA, 0xAA);
    let notes = cart.chip.take_notes().notes;
    assert_eq!(
        notes.iter().map(|n| n.kind).collect::<Vec<_>>(),
        [NoteKind::UnlockAddress { expected: 0x5555 }]
    );
}

#[test]
fn ordinary_mapper_writes_take_no_notes() {
    let mut cart = Cart::new(profile("generic-wr-aaa-aa"));
    for (bus, value) in [
        (0x0000, 0x0A),
        (0x2000, 0x05),
        (0x3000, 0x00),
        (0x4000, 0x02),
    ] {
        cart.write(bus, value);
    }
    assert!(cart.chip.take_notes().notes.is_empty());
}

#[test]
fn a_write_while_busy_is_ignored_and_noted() {
    let mut cart = Cart::new(profile("generic-wr-aaa-a9"));
    cart.gbstudio_erase();
    cart.write(0x4000, 0xF0);
    assert!(cart.chip.busy(cart.now), "a reset can't stop an erase");
    let notes = cart.chip.take_notes().notes;
    assert_eq!(
        notes.iter().map(|n| n.kind).collect::<Vec<_>>(),
        [NoteKind::WriteWhileBusy {
            op: Operation::SectorErase
        }]
    );
}

#[test]
fn a_saved_state_restores_a_busy_chip_and_its_array() {
    let mut cart = Cart::new(profile("generic-wr-aaa-a9"));
    cart.bank = SAVE_BANK;
    cart.gbstudio_program(0x4000, 0x80);
    let saved = cart.chip.save(&cart.rom);
    let (saved_rom, saved_now) = (cart.rom.clone(), cart.now);
    let after_save: Vec<u8> = (0..3).map(|_| cart.read(0x4000)).collect();

    // A second timeline erases the sector; loading must undo that.
    cart.wait(BusyTicks::TYPICAL.program);
    cart.gbstudio_erase();
    cart.wait(BusyTicks::TYPICAL.sector_erase);
    cart.bank = 1;
    cart.gbstudio_program(0x4000, 0x00);
    cart.wait(BusyTicks::TYPICAL.program);

    let mut fresh = Cart::new(profile("generic-wr-aaa-a9"));
    for chip in [&mut cart, &mut fresh] {
        chip.chip.load(&saved, &mut chip.rom).expect("load");
        chip.now = saved_now;
        chip.bank = SAVE_BANK;
        assert_eq!(chip.rom, saved_rom);
        let again: Vec<u8> = (0..3).map(|_| chip.read(0x4000)).collect();
        assert_eq!(again, after_save, "status and DQ6 continue as before");
    }
}

#[test]
fn a_state_from_another_board_is_refused() {
    let cart = Cart::new(profile("generic-wr-aaa-a9"));
    let saved = cart.chip.save(&cart.rom);
    let mut other = Cart::new(profile("generic-wr-aaa-aa"));
    let err = other.chip.load(&saved, &mut other.rom).unwrap_err();
    assert!(err.to_string().contains("generic-wr-aaa-a9"), "{err}");
    assert!(other.chip.load(b"junk", &mut other.rom).is_err());
}

#[test]
fn contents_survive_a_power_cycle_and_a_cut_off_operation_is_noted() {
    let mut cart = Cart::new(profile("generic-wr-aaa-a9"));
    cart.bank = SAVE_BANK;
    cart.gbstudio_program(0x4000, 0x42);
    cart.wait(BusyTicks::TYPICAL.program);
    cart.gbstudio_program(0x4001, 0x43);
    let contents = cart.chip.contents(cart.now, &cart.rom);

    let mut on = Cart::new(profile("generic-wr-aaa-a9"));
    on.chip.power_on(&contents, &mut on.rom).expect("power on");
    on.now = 0;
    on.bank = SAVE_BANK;
    assert_eq!(on.read(0x4000), 0x42);
    assert_eq!(on.read(0x4001), 0x43, "not busy after power-on");
    let notes = on.chip.take_notes().notes;
    assert_eq!(
        notes.iter().map(|n| n.kind).collect::<Vec<_>>(),
        [NoteKind::CutOff {
            op: Operation::Program
        }]
    );
}

/// Across a rebuild, the new image's extent is the new build's: the sectors
/// that overlap it leave the contents, and the rest power on as before.
#[test]
fn contents_past_an_image_drop_the_sectors_that_overlap_it() {
    let mut cart = Cart::new(profile("generic-wr-aaa-a9"));
    cart.bank = 4; // chip $10000: the sector at $10000
    cart.gbstudio_program(0x4000, 0x00);
    cart.wait(BusyTicks::TYPICAL.program);
    cart.bank = SAVE_BANK; // chip $30000
    cart.gbstudio_program(0x4000, 0x42);
    cart.wait(BusyTicks::TYPICAL.program);
    let contents = cart.chip.contents(cart.now, &cart.rom);

    let split = contents_past(&contents, 0x2_0000).expect("split");
    assert_eq!(
        split.dropped,
        vec![std::ops::Range {
            start: 0x1_0000,
            end: 0x2_0000
        }]
    );
    let mut on = Cart::new(profile("generic-wr-aaa-a9"));
    on.chip
        .power_on(&split.kept, &mut on.rom)
        .expect("power on");
    assert_eq!(on.rom[0x1_0000], rom()[0x1_0000], "the image's byte");
    assert_eq!(on.rom[0x3_0000], 0x42, "the save past the image");

    let whole = contents_past(&contents, 0x1_0000).expect("split");
    assert!(whole.dropped.is_empty(), "{:?}", whole.dropped);
    assert_eq!(whole.kept, contents);
    assert!(contents_past(b"junk", 0).is_err());
}

/// Each mask is its chip family's command decode (project/2026-11-07-flash-
/// model.md, "Command-address decode"): AMD-style parts compare A10-A0 and,
/// in byte mode, A-1 (byte bits 11-0) and treat A11 and up as don't-care; on
/// a "555" board the cart's A0 is the chip's A0, so 11 bits; SST39SF040
/// compares A14-A0.
#[test]
fn every_profiles_command_mask_is_its_chip_familys() {
    let expected = [
        ("generic-wr-aaa-a9", 0xFFF),
        ("generic-wr-aaa-aa", 0xFFF),
        ("generic-wr-555-a9", 0x7FF),
        ("generic-wr-555-aa", 0x7FF),
        ("generic-audio-aaa-a9", 0xFFF),
        ("generic-audio-aaa-aa", 0xFFF),
        ("29dl163bd-90", 0xFFF),
        ("am29lv160db", 0xFFF),
        ("mx29lv320et", 0xFFF),
        ("sst39sf040-audio", 0x7FFF),
    ];
    assert_eq!(FlashProfile::names().count(), expected.len());
    for (name, mask) in expected {
        assert_eq!(profile(name).command_mask, mask, "{name}");
    }
}

/// A11 and up being don't-care, a command in the $4000 window reaches the
/// chip whichever bank is mapped.
#[test]
fn commands_in_the_4000_window_work_on_every_bank() {
    for bank in 1..=4 {
        let mut cart = Cart::new(profile("am29lv160db"));
        cart.bank = bank;
        for (bus, value) in [
            (0x4AAA, 0xA9),
            (0x4555, 0x56),
            (0x4AAA, 0xA0),
            (0x4100, 0x00),
        ] {
            cart.write(bus, value);
        }
        cart.wait(BusyTicks::TYPICAL.program);
        assert_eq!(cart.read(0x4100), 0x00, "bank {bank}");
        assert!(cart.chip.take_notes().notes.is_empty(), "bank {bank}");
    }
}

/// The board a mask-ROM cart's write would unlock: the first named profile,
/// /WE on WR, whose first unlock cycle it is.
#[test]
fn a_write_names_the_board_it_unlocks() {
    assert_eq!(
        FlashProfile::unlocked_by(0x0AAA, 0xA9),
        Some("generic-wr-aaa-a9")
    );
    assert_eq!(
        FlashProfile::unlocked_by(0x0AAA, 0xAA),
        Some("generic-wr-aaa-aa")
    );
    assert_eq!(
        FlashProfile::unlocked_by(0x0555, 0xAA),
        Some("generic-wr-555-aa")
    );
    for (addr, value) in [
        (0x0000, 0x0A),
        (0x2000, 0x05),
        (0x0AAA, 0x0A),
        (0x0555, 0x55),
    ] {
        assert_eq!(FlashProfile::unlocked_by(addr, value), None, "{addr:#x}");
    }
}

#[test]
fn every_named_profile_resolves_and_cites_flashgbx() {
    let names: Vec<_> = FlashProfile::names().collect();
    assert!(names.contains(&"generic-wr-aaa-a9"));
    for name in names {
        let p = profile(name);
        assert_eq!(p.name, name);
        assert!(
            p.source.starts_with("FlashGBX 5b2398b config/fc_DMG_"),
            "{}",
            p.source
        );
    }
    assert!(FlashProfile::named("nope").is_none());
    let gbs = profile("generic-wr-aaa-a9");
    assert_eq!(gbs.data_lines, DataLines::SwappedD0D1);
    assert_eq!(gbs.write_pin, WritePin::Wr);
    assert_eq!(gbs.unlock, [0x0AAA, 0x0555]);
}

#[test]
fn the_swap_exchanges_d0_and_d1_only() {
    let s = DataLines::SwappedD0D1;
    assert_eq!(s.cross(0xAA), 0xA9);
    assert_eq!(s.cross(0x55), 0x56);
    assert_eq!(s.cross(0xA0), 0xA0);
    assert_eq!(DataLines::Straight.cross(0xA9), 0xA9);
    assert!((0..=255u8).all(|v| s.cross(s.cross(v)) == v));
}
