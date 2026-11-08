//! A ROM-only cartridge (header cart type $00, $08 or $09) maps banks 0 and
//! 1 and nothing else, so a link that puts any section past bank 1 makes a
//! ROM whose code reads the wrong bytes. The build refuses it, naming the
//! sections from the `.map`; a header that names an MBC builds. Needs the
//! pinned RGBDS on `PATH`.

use crate::common;

use std::fs;
use std::path::Path;

use gbcli_build::{BuildError, build};
use gbcli_protocol::ErrorCode;

use common::{rgbds, starter, without_music};

/// Two 12 KB ROMX sections: they can't share bank 1, so one lands in bank 2.
const BIG: &str =
    "SECTION \"Level A\", ROMX\nds $3000, 1\n\nSECTION \"Level B\", ROMX\nds $3000, 2\n";

/// Sets the header's cartridge type byte ($0147) in the starter's main.asm.
fn cart_type(project: &Path, kind: u8) {
    let main = project.join("src/main.asm");
    let text = fs::read_to_string(&main).unwrap();
    let header = format!("ds $0147 - @, 0\n\tdb ${kind:02X}\n\tds $0150 - @, 0");
    assert!(text.contains("ds $0150 - @, 0"));
    fs::write(&main, text.replace("ds $0150 - @, 0", &header)).unwrap();
}

#[test]
fn a_rom_only_link_past_bank_1_fails_naming_the_sections_there() {
    for kind in [0x00, 0x08, 0x09] {
        let (temp, options) = starter("banks-rom-only");
        let project = temp.path("demo");
        without_music(&project);
        cart_type(&project, kind);
        fs::write(project.join("src/levels.asm"), BIG).unwrap();
        let err = build(&rgbds(), &options).unwrap_err();
        let BuildError::Failed { made, .. } = &err else {
            panic!("expected a failed build, got {err:?}");
        };
        let overflow = made.overflow.as_ref().expect("the overflow, structured");
        assert_eq!(overflow.cart_type, kind);
        assert_eq!(overflow.sections.len(), 1, "{overflow:?}");
        let s = &overflow.sections[0];
        assert!(s.name == "Level A" || s.name == "Level B", "{s:?}");
        assert_eq!((s.bank, s.start, s.bytes), (2, 0x4000, 0x3000));
        let body = err.body();
        assert_eq!(body.code, ErrorCode::BuildFailed);
        assert!(body.message.contains(&s.name), "{}", body.message);
        assert!(body.message.contains("bank 2"), "{}", body.message);
        assert!(body.remedy.contains("MBC"), "{}", body.remedy);
        assert!(!project.join("build/demo.gb").exists());
    }
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
