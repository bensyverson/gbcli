use super::*;

/// A ROM of `len` bytes whose header says `kind`, `rom` and `ram`.
fn rom(len: usize, kind: u8, rom: u8, ram: u8) -> Vec<u8> {
    let mut bytes = vec![0u8; len];
    bytes[CART_TYPE] = kind;
    bytes[ROM_SIZE] = rom;
    bytes[RAM_SIZE] = ram;
    bytes
}

#[test]
fn a_header_that_matches_its_file_has_nothing_to_say() {
    assert_eq!(check_header(&rom(32 * KIB, 0x00, 0x00, 0x00)), []);
    assert_eq!(check_header(&rom(64 * KIB, 0x1B, 0x01, 0x03)), []);
    assert_eq!(check_header(&rom(2048 * KIB, 0x13, 0x06, 0x03)), []);
    assert_eq!(check_header(&rom(32 * KIB, 0x0F, 0x00, 0x00)), []);
    assert_eq!(check_header(&rom(256 * KIB, 0x06, 0x03, 0x00)), []);
}

#[test]
fn a_file_shorter_than_its_declared_rom_size_is_named_with_the_rgbfix_fix() {
    let found = check_header(&rom(40 * KIB, 0x19, 0x01, 0x00));
    assert_eq!(
        found,
        [HeaderMismatch::RomSize {
            byte: 0x01,
            declared: 64 * KIB,
            file: 40 * KIB
        }]
    );
    assert_eq!(
        found[0].to_string(),
        "the header declares a 64 KiB ROM ($01 at $0148) but the file is 40 KiB"
    );
    assert!(found[0].remedy().contains("rgbfix -p 0xFF"));
}

#[test]
fn a_file_longer_than_its_declared_rom_size_is_named_too() {
    let found = check_header(&rom(128 * KIB, 0x19, 0x01, 0x00));
    assert!(
        matches!(found.as_slice(), [HeaderMismatch::RomSize { declared, .. }] if *declared == 64 * KIB),
        "{found:?}"
    );
}

#[test]
fn a_rom_only_cart_past_32k_needs_a_mapper() {
    let found = check_header(&rom(64 * KIB, 0x00, 0x01, 0x00));
    assert_eq!(
        found,
        [HeaderMismatch::RomTooBigForMapper {
            cart: "ROM ONLY",
            max: 32 * KIB,
            file: 64 * KIB
        }]
    );
    assert!(found[0].remedy().contains("rgbfix -m MBC5"));
}

#[test]
fn unknown_codes_are_named() {
    let found = check_header(&rom(32 * KIB, 0x42, 0x0A, 0x09));
    assert_eq!(
        found,
        [
            HeaderMismatch::UnknownCartType { byte: 0x42 },
            HeaderMismatch::UnknownRomSize { byte: 0x0A },
            HeaderMismatch::UnknownRamSize { byte: 0x09 },
        ]
    );
}

#[test]
fn ram_declared_on_a_type_without_ram_and_the_reverse() {
    assert_eq!(
        check_header(&rom(64 * KIB, 0x19, 0x01, 0x02)),
        [HeaderMismatch::RamWithoutRamType {
            cart: "MBC5",
            declared: 8 * KIB
        }]
    );
    let found = check_header(&rom(64 * KIB, 0x1B, 0x01, 0x00));
    assert_eq!(
        found,
        [HeaderMismatch::RamTypeWithoutRam {
            cart: "MBC5+RAM+BATTERY"
        }]
    );
    assert!(found[0].remedy().contains("rgbfix -r 2"));
}

#[test]
fn mbc2_declares_no_ram_and_mbc1_banks_at_most_32k() {
    assert_eq!(
        check_header(&rom(64 * KIB, 0x06, 0x01, 0x02)),
        [HeaderMismatch::Mbc2RamSize { declared: 8 * KIB }]
    );
    assert_eq!(
        check_header(&rom(64 * KIB, 0x03, 0x01, 0x04)),
        [HeaderMismatch::RamTooBigForMapper {
            cart: "MBC1+RAM+BATTERY",
            max: 32 * KIB,
            declared: 128 * KIB
        }]
    );
}

#[test]
fn a_file_too_short_for_a_header_says_so_and_nothing_else() {
    assert_eq!(
        check_header(&[0; 0x100]),
        [HeaderMismatch::NoHeader { len: 0x100 }]
    );
}

/// The header's CGB flag ($0143) as the CGB boot ROM reads it (leaf
/// jGOxRH): bit 7 clear is a DMG game, $C0 is CGB only, any other value
/// with bit 7 set runs in CGB mode on a CGB and DMG mode on a DMG.
#[test]
fn the_cgb_flag_reads_dmg_dual_or_only_from_0143() {
    use gbcli_protocol::CgbFlag;
    let with = |byte: u8| {
        let mut bytes = vec![0u8; 0x150];
        bytes[0x143] = byte;
        cgb_flag(&bytes)
    };
    assert_eq!(with(0x00), Some(CgbFlag::Dmg));
    assert_eq!(with(0x7F), Some(CgbFlag::Dmg));
    assert_eq!(with(0x80), Some(CgbFlag::Dual));
    assert_eq!(
        with(0x84),
        Some(CgbFlag::Dual),
        "bit 7 is what the boot ROM tests"
    );
    assert_eq!(with(0xC0), Some(CgbFlag::Only));
    assert_eq!(cgb_flag(&[0; 0x143]), None, "too short for the byte");
}
