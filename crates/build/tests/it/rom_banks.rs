//! A ROM-only cartridge (header cart type $00, $08 or $09) maps banks 0 and
//! 1 and nothing else, so a link that puts any section past bank 1 makes a
//! ROM whose code reads the wrong bytes. The build then links again as one
//! 32 KB bank (`rgblink --tiny`), which is what such a cartridge maps; past
//! 32 KB it refuses, naming the sections from the first link's `.map`. A
//! header that names an MBC builds. Needs the pinned RGBDS on `PATH`.

use crate::common;

use std::fs;
use std::path::Path;

use gbcli_build::{BuildError, build};
use gbcli_engine::equiv::{EquivSpec, Verdict, compare};
use gbcli_protocol::{ErrorCode, Severity, Tool};

use crate::player::booted;
use common::{rgbds, starter, without_music};

/// Two 12 KB ROMX sections: they can't share bank 1, so one lands in bank 2,
/// but beside the starter they fit in 32 KB.
const BIG: &str =
    "SECTION \"Level A\", ROMX\nds $3000, 1\n\nSECTION \"Level B\", ROMX\nds $3000, 2\n";

/// Three 12 KB ROMX sections: 36 KB, past what a ROM-only cartridge maps.
const TOO_BIG: &str = "SECTION \"Level A\", ROMX\nds $3000, 1\n\nSECTION \"Level B\", ROMX\n\
                       ds $3000, 2\n\nSECTION \"Level C\", ROMX\nds $3000, 3\n";

/// Sets the header's cartridge type byte ($0147) in the starter's main.asm.
fn cart_type(project: &Path, kind: u8) {
    let main = project.join("src/main.asm");
    let text = fs::read_to_string(&main).unwrap();
    let header = format!("ds $0147 - @, 0\n\tdb ${kind:02X}\n\tds $0150 - @, 0");
    assert!(text.contains("ds $0150 - @, 0"));
    fs::write(&main, text.replace("ds $0150 - @, 0", &header)).unwrap();
}

#[test]
fn a_rom_only_link_past_32_kb_fails_naming_the_sections_past_bank_1() {
    for kind in [0x00, 0x08, 0x09] {
        let (temp, options) = starter("banks-rom-only");
        let project = temp.path("demo");
        without_music(&project);
        cart_type(&project, kind);
        fs::write(project.join("src/levels.asm"), TOO_BIG).unwrap();
        let err = build(&rgbds(), &options).unwrap_err();
        let BuildError::Failed { made, .. } = &err else {
            panic!("expected a failed build, got {err:?}");
        };
        let overflow = made.overflow.as_ref().expect("the overflow, structured");
        assert_eq!(overflow.cart_type, kind);
        assert_eq!(overflow.sections.len(), 2, "{overflow:?}");
        for s in &overflow.sections {
            assert!(s.name.starts_with("Level "), "{s:?}");
            assert!([2, 3].contains(&s.bank), "{s:?}");
            assert_eq!((s.start, s.bytes), (0x4000, 0x3000), "{s:?}");
        }
        let body = err.body();
        assert_eq!(body.code, ErrorCode::BuildFailed);
        let s = &overflow.sections[0];
        assert!(body.message.contains(&s.name), "{}", body.message);
        assert!(
            body.message.contains(&format!("bank {}", s.bank)),
            "{}",
            body.message
        );
        assert!(body.message.contains("32 KB"), "{}", body.message);
        assert!(body.remedy.contains("MBC"), "{}", body.remedy);
        assert!(!project.join("build/demo.gb").exists());
    }
}

/// A ROM-only link that rgblink spreads past bank 1, but whose sections fit
/// in 32 KB, is linked again as one bank and builds, with one warning row
/// saying so.
#[test]
fn a_rom_only_link_past_bank_1_that_fits_in_32_kb_links_as_one_bank() {
    for kind in [0x00, 0x08, 0x09] {
        let (temp, options) = starter("banks-tiny");
        let project = temp.path("demo");
        without_music(&project);
        cart_type(&project, kind);
        fs::write(project.join("src/levels.asm"), BIG).unwrap();
        let made = build(&rgbds(), &options).unwrap();
        assert!(made.overflow.is_none());
        let rom = fs::read(project.join("build/demo.gb")).unwrap();
        assert_eq!(rom.len(), 0x8000);
        assert_eq!(rom[0x0147], kind);
        let map = fs::read_to_string(project.join("build/demo.map")).unwrap();
        assert!(!map.contains("ROMX bank #"), "{map}");
        for name in ["Level A", "Level B", "font Main"] {
            assert!(map.contains(&format!("[\"{name}\"]")), "{name}: {map}");
        }
        assert!(
            !made
                .diagnostics
                .iter()
                .any(|d| d.severity == Severity::Error),
            "{:?}",
            made.diagnostics
        );
        let warnings: Vec<_> = made
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .collect();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert_eq!(warnings[0].tool, Tool::Rgblink);
        assert!(
            warnings[0].message.contains("one 32 KB bank"),
            "{}",
            warnings[0].message
        );
    }
}

/// A ROMX filler that leaves too little of bank 1 for the starter's song
/// and font, as a long song does (Chavez - Monkeys on Mars, 16,222 bytes).
const FILLER: &str = "SECTION \"Filler\", ROMX\nds $3A00, $FF\n";

/// The starter, with a filler that pushes its song or font out of bank 1,
/// builds as one 32 KB bank and shows and plays what the starter does. The
/// starter itself still links in banks, with no warning: a build that fits
/// links once, as before.
#[test]
fn a_starter_whose_romx_outgrows_bank_1_runs_as_the_starter_does() {
    let (plain_temp, plain_options) = starter("banks-plain");
    let plain = build(&rgbds(), &plain_options).unwrap();
    assert!(plain.diagnostics.is_empty(), "{:?}", plain.diagnostics);
    let plain_map = fs::read_to_string(plain_temp.path("demo/build/demo.map")).unwrap();
    assert!(plain_map.contains("ROMX bank #1:"), "{plain_map}");
    let plain_rom = fs::read(plain_temp.path("demo/build/demo.gb")).unwrap();

    let (temp, options) = starter("banks-filled");
    fs::write(temp.path("demo/src/filler.asm"), FILLER).unwrap();
    let made = build(&rgbds(), &options).unwrap();
    assert_eq!(made.diagnostics.len(), 1, "{:?}", made.diagnostics);
    let rom = fs::read(temp.path("demo/build/demo.gb")).unwrap();
    assert_eq!(rom.len(), 0x8000);

    let (mut a, mut b) = (booted(&plain_rom), booted(&rom));
    let frames = 120;
    let verdict = compare(&mut a, &mut b, &EquivSpec::new(frames)).unwrap();
    assert_eq!(
        verdict,
        Verdict::Same {
            frames,
            ended: None
        }
    );
}

#[test]
fn a_header_that_names_an_mbc_builds_past_bank_1() {
    let (temp, options) = starter("banks-mbc1");
    let project = temp.path("demo");
    without_music(&project);
    cart_type(&project, 0x01);
    fs::write(project.join("src/levels.asm"), BIG).unwrap();
    let made = build(&rgbds(), &options).unwrap();
    assert!(made.overflow.is_none());
    assert_eq!(
        fs::read(project.join("build/demo.gb")).unwrap().len(),
        0x10000
    );
}

/// A 15 KB ROM0 section: beside the starter's own ROM0 code it outgrows
/// bank 0's 16 KB, but fits in 32 KB.
const ROM0_BIG: &str = "SECTION \"Tables\", ROM0\nds $3C00, 1\n";

/// Two 15 KB ROM0 sections: with the starter, past 32 KB.
const ROM0_TOO_BIG: &str =
    "SECTION \"Tables\", ROM0\nds $3C00, 1\n\nSECTION \"More tables\", ROM0\nds $3C00, 2\n";

/// The build's warning rows.
fn warnings(made: &gbcli_protocol::BuildMade) -> Vec<&gbcli_protocol::Diagnostic> {
    made.diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Warning)
        .collect()
}

/// rgblink can't place a ROM0 section when bank 0 is full; a header with no
/// MBC maps 32 KB flat, so the build links again as one bank and builds.
#[test]
fn a_rom_only_bank_0_past_16_kb_that_fits_in_32_kb_links_as_one_bank() {
    let (temp, options) = starter("banks-rom0-tiny");
    let project = temp.path("demo");
    fs::write(project.join("src/tables.asm"), ROM0_BIG).unwrap();
    let made = build(&rgbds(), &options).unwrap();
    assert_eq!(
        fs::read(project.join("build/demo.gb")).unwrap().len(),
        0x8000
    );
    let warnings = warnings(&made);
    assert_eq!(made.diagnostics.len(), 1, "{:?}", made.diagnostics);
    assert_eq!(warnings.len(), 1, "{:?}", made.diagnostics);
    assert!(
        warnings[0].message.contains("one 32 KB bank"),
        "{}",
        warnings[0].message
    );
}

/// With an MBC in the header, a full bank 0 is rgblink's own error, with a
/// hint naming ROMX and an MBC; nothing is linked as one bank.
#[test]
fn a_full_bank_0_under_an_mbc_fails_with_rgblinks_error_and_a_hint() {
    let (temp, options) = starter("banks-rom0-mbc");
    let project = temp.path("demo");
    cart_type(&project, 0x01);
    fs::write(project.join("src/tables.asm"), ROM0_BIG).unwrap();
    let err = build(&rgbds(), &options).unwrap_err();
    let BuildError::Failed { made, .. } = &err else {
        panic!("expected a failed build, got {err:?}");
    };
    assert!(made.overflow.is_none());
    assert!(warnings(made).is_empty(), "{:?}", made.diagnostics);
    let body = err.body();
    assert!(body.message.contains("Unable to place"), "{}", body.message);
    assert!(
        body.message.contains("(ROM0 section) anywhere"),
        "{}",
        body.message
    );
    for needed in ["ROMX", "MBC", "32 KB"] {
        assert!(body.remedy.contains(needed), "{needed}: {}", body.remedy);
    }
}

/// Past 32 KB of ROM0 with no MBC, linking as one bank fails too: rgblink's
/// error, with the hint naming an MBC.
#[test]
fn a_rom_only_bank_0_past_32_kb_fails_with_a_hint_naming_an_mbc() {
    let (temp, options) = starter("banks-rom0-too-big");
    let project = temp.path("demo");
    fs::write(project.join("src/tables.asm"), ROM0_TOO_BIG).unwrap();
    let err = build(&rgbds(), &options).unwrap_err();
    let BuildError::Failed { made, .. } = &err else {
        panic!("expected a failed build, got {err:?}");
    };
    assert!(made.overflow.is_none());
    assert!(warnings(made).is_empty(), "{:?}", made.diagnostics);
    let body = err.body();
    assert!(
        body.message.contains("(ROM0 section) anywhere"),
        "{}",
        body.message
    );
    assert!(body.remedy.contains("MBC"), "{}", body.remedy);
    assert!(!project.join("build/demo.gb").exists());
}

/// Code at the start and in the middle of bank 5, and nothing in banks 2-4.
const SPARSE: &str = "SECTION \"Head\", ROMX[$4000], BANK[5]\ndb $C9\n\n\
                      SECTION \"Middle\", ROMX[$7000], BANK[5]\ndb $C9\n";

/// Every byte no section holds is $FF, as erased flash is: the carried-flash
/// rule (`FlashChip::revert_over_code`) reads a build's non-$FF bytes as its
/// code, so padding of any other value would drop a save kept there.
#[test]
fn every_byte_no_section_holds_is_ff_in_unused_banks_and_gaps_alike() {
    let (temp, options) = starter("banks-padding");
    let project = temp.path("demo");
    without_music(&project);
    cart_type(&project, 0x01);
    fs::write(project.join("src/levels.asm"), SPARSE).unwrap();
    build(&rgbds(), &options).unwrap();
    let rom = fs::read(project.join("build/demo.gb")).unwrap();
    assert_eq!(rom.len(), 0x2_0000, "six banks, padded to eight");
    let not_ff = |range: std::ops::Range<usize>| range.filter(|&i| rom[i] != 0xFF).count();
    assert_eq!(not_ff(0x8000..0x1_4000), 0, "banks 2-4, which hold nothing");
    assert_eq!(rom[0x1_4000], 0xC9);
    assert_eq!(not_ff(0x1_4001..0x1_7000), 0, "the gap in bank 5");
    assert_eq!(rom[0x1_7000], 0xC9);
    assert_eq!(
        not_ff(0x1_7001..0x1_8000),
        0,
        "bank 5 past its last section"
    );
    assert_eq!(not_ff(0x1_8000..0x2_0000), 0, "banks 6-7, past the link");
}
