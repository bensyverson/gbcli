use gbcli_core::Region;

use super::*;
use crate::sym::Symbols;

const SYM: &str = "\
00:0150 Main\n\
00:0153 Main.loop\n\
03:4000 Banked\n\
03:7FF0 BankedEnd\n\
00:C000 wFrame\n\
00:FF80 hTemp\n";

fn symbols() -> Symbols {
    Symbols::parse(SYM).unwrap()
}

/// A DMG on a 32 KiB cart without RAM.
const DMG: RegionSizes = RegionSizes {
    rom: 0x8000,
    wram: 0x2000,
    sram: 0,
    vram: 0x2000,
    hram: 0x7F,
    oam: 0xA0,
    io: 0x80,
    ie: 1,
    basis: SizeBasis::ThisMachine,
};

/// A CGB on a 64 KiB cart with 32 KiB of RAM.
const CGB: RegionSizes = RegionSizes {
    rom: 0x10000,
    wram: 0x8000,
    sram: 0x8000,
    vram: 0x4000,
    hram: 0x7F,
    oam: 0xA0,
    io: 0x80,
    ie: 1,
    basis: SizeBasis::ThisMachine,
};

#[test]
fn a_standalone_address_needs_a_prefix_but_a_symbol_does_not() {
    let s = symbols();
    let at = parse("$C0A0", Some(&s)).unwrap();
    assert_eq!((at.addr, at.bank), (0xC0A0, None));
    let at = parse("0xC0A0", Some(&s)).unwrap();
    assert_eq!(at.addr, 0xC0A0);
    assert!(parse("C0A0", Some(&s)).is_err(), "no prefix on bare hex");
}

#[test]
fn a_symbol_resolves_to_its_bank_and_address() {
    let s = symbols();
    let at = parse("Main", Some(&s)).unwrap();
    assert_eq!((at.addr, at.bank), (0x0150, None));
    let at = parse("Banked", Some(&s)).unwrap();
    assert_eq!((at.addr, at.bank), (0x4000, Some(3)));
    assert_eq!(parse("Main+3", Some(&s)).unwrap().addr, 0x0153);
    assert_eq!(parse("Banked+$10", Some(&s)).unwrap().addr, 0x4010);
    assert_eq!(parse("Main.loop+1", Some(&s)).unwrap().addr, 0x0154);
    assert!(parse("Main+$", Some(&s)).is_err());
}

#[test]
fn a_symbol_minus_an_offset_resolves() {
    let s = symbols();
    let at = parse("Main-1", Some(&s)).unwrap();
    assert_eq!((at.addr, at.bank), (0x014F, None));
    assert_eq!(parse("Main.loop-$3", Some(&s)).unwrap().addr, 0x0150);
    let at = parse("BankedEnd-$10", Some(&s)).unwrap();
    assert_eq!((at.addr, at.bank), (0x7FE0, Some(3)));
}

#[test]
fn an_offset_after_a_name_is_decimal_unless_prefixed() {
    let s = symbols();
    assert_eq!(parse("Main+16", Some(&s)).unwrap().addr, 0x0160);
    assert_eq!(parse("Main+10", Some(&s)).unwrap().addr, 0x015A);
    assert_eq!(parse("Main+$10", Some(&s)).unwrap().addr, 0x0160);
    assert_eq!(parse("Main+0x10", Some(&s)).unwrap().addr, 0x0160);
    assert_eq!(parse("Main+%10000", Some(&s)).unwrap().addr, 0x0160);
    let err = parse("Main+1F", Some(&s)).unwrap_err().remedy();
    assert!(err.contains("$1F"), "the hint names the hex form: {err}");
    // A label stored by a checkpoint before labels were written in hex
    // (`Main+16`) reads back as the decimal it was.
    assert_eq!(split_label("Main+16"), ("Main", 16));
    assert_eq!(
        parse("Main+16", Some(&s)).unwrap().symbol.as_deref(),
        Some("Main+$10")
    );
}

#[test]
fn an_offset_symbol_keeps_a_label_the_grammar_reads_back() {
    let s = symbols();
    for text in [
        "Main+$10",
        "Main-1",
        "Main+0x1F",
        "Banked+3",
        "Main+31",
        "Banked+%11",
    ] {
        let at = parse(text, Some(&s)).unwrap();
        let label = at.symbol.clone().unwrap();
        let again = parse(&label, Some(&s)).unwrap();
        assert_eq!(
            (again.addr, again.bank),
            (at.addr, at.bank),
            "{text} -> {label}"
        );
    }
    assert_eq!(
        parse("Main", Some(&s)).unwrap().symbol.as_deref(),
        Some("Main")
    );
    assert_eq!(
        parse("Main+$10", Some(&s)).unwrap().symbol.as_deref(),
        Some("Main+$10")
    );
    assert_eq!(split_label("Main-$1F"), ("Main", -0x1F));
    assert_eq!(split_label("Main"), ("Main", 0));
}

/// The offset error, or a panic naming what came back instead.
fn offset_error(text: &str) -> AddrError {
    match parse(text, Some(&symbols())) {
        Err(e @ AddrError::SymbolOffset { .. }) => e,
        other => panic!("{text}: expected SymbolOffset, got {other:?}"),
    }
}

#[test]
fn a_symbol_offset_past_the_address_space_is_refused_with_the_range_that_fits() {
    let e = offset_error("Main+$10000");
    assert!(e.to_string().contains("Main+$10000"), "{e}");
    assert!(e.remedy().contains("Main+$FEAF"), "{}", e.remedy());
    let e = offset_error("Main-$151");
    assert!(e.remedy().contains("Main-$150"), "{}", e.remedy());
    offset_error("hTemp+$80");
    offset_error("Main+$FFFFFFFFFF");
    assert_eq!(
        parse("hTemp+$7F", Some(&symbols())).unwrap().addr,
        0xFFFF,
        "the last byte fits"
    );
}

#[test]
fn a_banked_symbol_offset_may_not_leave_its_area() {
    let e = offset_error("Banked+$4000");
    assert!(e.to_string().contains("ROMX"), "{e}");
    assert!(e.remedy().contains("Banked+$3FFF"), "{}", e.remedy());
    assert!(e.remedy().contains("03:"), "{}", e.remedy());
    offset_error("Banked-1");
    assert_eq!(
        parse("Banked+$3FFF", Some(&symbols())).unwrap().addr,
        0x7FFF
    );
}

#[test]
fn a_bad_symbol_offset_is_a_syntax_error_with_a_corrected_form() {
    let err = parse("Main+zz", Some(&symbols())).unwrap_err();
    assert!(matches!(err, AddrError::Syntax { .. }), "{err:?}");
    assert!(err.remedy().contains("Main+$"), "{}", err.remedy());
    assert!(matches!(
        parse("-$10", Some(&symbols())),
        Err(AddrError::Syntax { .. })
    ));
}

#[test]
fn an_unknown_symbol_lists_near_misses() {
    let err = parse("Mian", Some(&symbols())).unwrap_err();
    let AddrError::UnknownSymbol { near, .. } = &err else {
        panic!("{err:?}")
    };
    assert_eq!(near, &["Main".to_string()]);
    assert!(err.remedy().contains("Main"));
}

#[test]
fn a_bank_qualified_address_carries_its_bank() {
    let at = parse("03:4123", None).unwrap();
    assert_eq!((at.addr, at.bank), (0x4123, Some(3)));
    let at = parse("$03:$4123", None).unwrap();
    assert_eq!((at.addr, at.bank), (0x4123, Some(3)));
}

/// The bank error, or a panic naming what came back instead.
fn bank_error(text: &str, sizes: &RegionSizes) -> AddrError {
    match parse_on(text, None, sizes) {
        Err(e @ AddrError::BankRange { .. }) => e,
        other => panic!("{text}: expected BankRange, got {other:?}"),
    }
}

#[test]
fn a_bank_the_machine_lacks_is_refused_naming_the_banks_it_has() {
    // (past the last bank, sizes, banks named, the last bank's form)
    let cases: [(&str, &RegionSizes, &str, &str); 7] = [
        ("04:4123", &CGB, "4 ROM banks (00-03)", "03:4123"),
        ("02:4000", &DMG, "2 ROM banks (00-01)", "01:4000"),
        ("08:D000", &CGB, "8 WRAM banks (00-07)", "07:D000"),
        ("02:D123", &DMG, "2 WRAM banks (00-01)", "01:D123"),
        ("04:A000", &CGB, "4 cart RAM banks (00-03)", "03:A000"),
        ("02:8000", &CGB, "2 VRAM banks (00-01)", "01:8000"),
        ("01:9FFF", &DMG, "1 VRAM bank (00-00)", "00:9FFF"),
    ];
    for (text, sizes, banks, last) in cases {
        let e = bank_error(text, sizes);
        assert!(e.to_string().contains(text), "{text}: {e}");
        assert!(e.to_string().contains(banks), "{text}: {e}");
        assert!(e.to_string().contains("this machine"), "{text}: {e}");
        assert!(e.remedy().contains(last), "{text}: {}", e.remedy());
        assert_eq!(e.code(), gbcli_protocol::ErrorCode::BadAddress);
        assert!(parse_on(last, None, sizes).is_ok(), "{last}");
    }
    let e = bank_error("00:A000", &DMG);
    assert!(e.to_string().contains("no cart RAM"), "{e}");
    assert!(
        matches!(
            parse_write_on("04:4123", None, &CGB),
            Err(AddrError::BankRange { .. })
        ),
        "a write target is checked too"
    );
}

#[test]
fn without_a_machine_only_banks_past_every_model_are_refused() {
    for text in ["200:4000", "08:D000", "10:A000", "02:8000"] {
        let e = match parse(text, None) {
            Err(e @ AddrError::BankRange { .. }) => e,
            other => panic!("{text}: expected BankRange, got {other:?}"),
        };
        assert!(e.to_string().contains("any model"), "{text}: {e}");
    }
    for text in ["1FF:4000", "07:D000", "0F:A000", "01:8000"] {
        assert!(parse(text, None).is_ok(), "{text}");
    }
}

#[test]
fn a_bank_prefix_is_optional_in_both_parts() {
    let want = parse("03:4123", None).unwrap();
    for text in [
        "0x03:0x4123",
        "$03:$4123",
        "0x03:4123",
        "03:$4123",
        "3:4123",
        "0X03:0X4123",
    ] {
        assert_eq!(parse(text, None), Ok(want.clone()), "{text}");
    }
}

#[test]
fn region_forms_map_onto_the_whole_backing_store() {
    assert_eq!(parse("wram:0x1A0", None).unwrap().addr, 0xC1A0);
    assert_eq!(parse("wram:0x1000", None).unwrap().addr, 0xD000);
    assert_eq!(parse("wram:0x1000", None).unwrap().bank, Some(1));
    assert_eq!(parse("wram:0x2000", None).unwrap().bank, Some(2));
    assert_eq!(parse("sram:0x2000", None).unwrap().addr, 0xA000);
    assert_eq!(parse("sram:0x2000", None).unwrap().bank, Some(1));
    assert_eq!(parse("vram:0x2000", None).unwrap().addr, 0x8000);
    assert_eq!(parse("vram:0x2000", None).unwrap().bank, Some(1));
    assert_eq!(parse("oam:4", None).unwrap().addr, 0xFE04);
    assert_eq!(parse("io:0x44", None).unwrap().addr, 0xFF44);
    assert_eq!(parse("rom:0x14E", None).unwrap().addr, 0x014E);
    assert_eq!(parse("rom:0x4010", None).unwrap().addr, 0x4010);
    assert_eq!(parse("rom:0x4010", None).unwrap().bank, Some(1));
}

/// The region error, or a panic naming what came back instead.
fn region_error(text: &str, sizes: &RegionSizes) -> AddrError {
    match parse_on(text, None, sizes) {
        Err(e @ AddrError::RegionOffset { .. }) => e,
        other => panic!("{text}: expected RegionOffset, got {other:?}"),
    }
}

/// Past the end, the sizes, the size the error names, the last offset the
/// remedy offers, and where that last offset lands.
type EdgeCase<'a> = (
    &'a str,
    &'a RegionSizes,
    &'a str,
    &'a str,
    (u16, Option<u16>),
);

#[test]
fn every_region_offset_past_the_end_is_refused_per_model_naming_the_real_size() {
    let cases: [EdgeCase; 12] = [
        ("hram:0x7F", &DMG, "0x7F bytes", "hram:0x7E", (0xFFFE, None)),
        ("io:0x80", &DMG, "0x80 bytes", "io:0x7F", (0xFF7F, None)),
        ("oam:0xA0", &DMG, "0xA0 bytes", "oam:0x9F", (0xFE9F, None)),
        ("ie:1", &DMG, "1 byte", "ie:0", (0xFFFF, None)),
        (
            "wram:0x2000",
            &DMG,
            "0x2000 bytes",
            "wram:0x1FFF",
            (0xDFFF, Some(1)),
        ),
        (
            "wram:0x8000",
            &DMG,
            "0x2000 bytes",
            "wram:0x1FFF",
            (0xDFFF, Some(1)),
        ),
        (
            "wram:0x8000",
            &CGB,
            "0x8000 bytes",
            "wram:0x7FFF",
            (0xDFFF, Some(7)),
        ),
        (
            "vram:0x2000",
            &DMG,
            "0x2000 bytes",
            "vram:0x1FFF",
            (0x9FFF, Some(0)),
        ),
        (
            "vram:0x4000",
            &CGB,
            "0x4000 bytes",
            "vram:0x3FFF",
            (0x9FFF, Some(1)),
        ),
        (
            "sram:0x8000",
            &CGB,
            "0x8000 bytes",
            "sram:0x7FFF",
            (0xBFFF, Some(3)),
        ),
        (
            "rom:0x8000",
            &DMG,
            "0x8000 bytes",
            "rom:0x7FFF",
            (0x7FFF, Some(1)),
        ),
        (
            "rom:0x10000",
            &CGB,
            "0x10000 bytes",
            "rom:0xFFFF",
            (0x7FFF, Some(3)),
        ),
    ];
    for (text, sizes, size, last, lands) in cases {
        let e = region_error(text, sizes);
        assert!(e.to_string().contains(text), "{text}: {e}");
        assert!(e.to_string().contains(size), "{text}: {e}");
        assert!(e.to_string().contains("this machine"), "{text}: {e}");
        assert!(e.remedy().contains(last), "{text}: {}", e.remedy());
        assert_eq!(e.code(), gbcli_protocol::ErrorCode::BadAddress);
        let at = parse_on(last, None, sizes).unwrap();
        assert_eq!((at.addr, at.bank), lands, "{last}");
    }
}

#[test]
fn a_cart_without_ram_has_no_sram_offsets() {
    let e = region_error("sram:0", &DMG);
    assert!(e.to_string().contains("no cart RAM"), "{e}");
    assert!(e.remedy().contains("wram:"), "{}", e.remedy());
}

#[test]
fn without_a_machine_offsets_past_every_model_are_refused_not_wrapped() {
    for text in [
        "hram:0x80",
        "io:0x100",
        "oam:0x200",
        "oam:0xA0",
        "ie:1",
        "vram:0x4000",
        "wram:0x8000",
        "sram:0x20000",
        "rom:0x800000",
        "wram:0xFFFFFFFFFFFF",
    ] {
        let e = match parse(text, None) {
            Err(e @ AddrError::RegionOffset { .. }) => e,
            other => panic!("{text}: expected RegionOffset, got {other:?}"),
        };
        assert!(e.to_string().contains("any model"), "{text}: {e}");
    }
}

#[test]
fn a_rom_offset_write_is_checked_against_the_cart() {
    assert_eq!(
        parse_write_on("rom:0x7FFF", None, &DMG),
        Ok(WriteTarget::RomOffset(0x7FFF))
    );
    assert!(matches!(
        parse_write_on("rom:0x8000", None, &DMG),
        Err(AddrError::RegionOffset {
            region: Region::Rom,
            ..
        })
    ));
}

#[test]
fn a_region_without_a_cpu_address_is_refused() {
    assert!(matches!(
        parse("bgpal:2", None),
        Err(AddrError::NoCpuAddress {
            region: Region::BgPalette
        })
    ));
}

#[test]
fn echo_ram_canonicalizes() {
    assert_eq!(parse("$E000", None).unwrap().addr, 0xC000);
}

#[test]
fn a_rom_offset_write_stays_a_file_offset() {
    assert_eq!(
        parse_write("rom:0x14E", None).unwrap(),
        WriteTarget::RomOffset(0x014E)
    );
    assert!(matches!(
        parse_write("wFrame", Some(&symbols())).unwrap(),
        WriteTarget::Cpu(_)
    ));
}
