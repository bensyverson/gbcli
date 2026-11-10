//! Regression tests for the one-bank relink's failure report, written from
//! the verification of 10b558a. Each asserts; run with `--nocapture` to see
//! the report it got.

use std::fs;
use std::path::Path;

use gbcli_build::{BuildError, build};
use gbcli_protocol::{BuildMade, Severity};

use crate::common::{rgbds, starter, without_music};

/// Two 12 KB ROMX sections: the first link puts one in bank 2.
const BIG: &str = "SECTION \"A\", ROMX\nds $3000, 1\n\nSECTION \"B\", ROMX\nds $3000, 2\n";

/// Three 12 KB ROMX sections: 36 KB, past what -t can place.
const TOO_BIG: &str = "SECTION \"A\", ROMX\nds $3000, 1\n\nSECTION \"B\", ROMX\nds $3000, 2\n\
                       \nSECTION \"C\", ROMX\nds $3000, 3\n";

/// 15 KB of ROM0: with the starter, past bank 0's 16 KB but inside 32 KB.
const ROM0_BIG: &str = "SECTION \"Tables\", ROM0\nds $3C00, 1\n";

/// Sets the header's cartridge type byte ($0147).
fn cart_type(project: &Path, kind: u8) {
    let main = project.join("src/main.asm");
    let text = fs::read_to_string(&main).unwrap();
    let header = format!("ds $0147 - @, 0\n\tdb ${kind:02X}\n\tds $0150 - @, 0");
    assert!(text.contains("ds $0150 - @, 0"));
    fs::write(&main, text.replace("ds $0150 - @, 0", &header)).unwrap();
}

fn rows(made: &BuildMade) -> String {
    made.diagnostics
        .iter()
        .enumerate()
        .map(|(i, d)| {
            format!(
                "{i}: {:?} {:?} [{}:{}]: {}",
                d.tool,
                d.severity,
                d.file.as_deref().unwrap_or("-"),
                d.line.map_or_else(|| "-".to_string(), |l| l.to_string()),
                d.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn flat(message: &str) -> String {
    message.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn entries(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = fs::read_dir(dir)
        .map(|r| {
            r.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// The section named by rgblink's `Unable to place "NAME" (ROM0 section)
/// anywhere` row, which the report must keep.
fn unplaced_in(rows: &str) -> String {
    rows.lines()
        .find_map(|l| l.split_once("Unable to place \"").map(|(_, rest)| rest))
        .and_then(|rest| rest.split_once("\" (ROM0 section) anywhere"))
        .map(|(name, _)| name.to_string())
        .unwrap_or_else(|| panic!("no unplaced-section row in:\n{rows}"))
}

struct Failed {
    made: Box<BuildMade>,
    message: String,
    remedy: String,
}

/// Builds the starter with `files` under src/ and an optional cartridge
/// type, expecting a failure, and returns its report and the entries of
/// build/. `music` keeps the song: its "Sound Driver" is ROM0, so without
/// it 15 KB of ROM0 still fits bank 0 and no spill happens.
fn failed(name: &str, files: &[(&str, &str)], kind: Option<u8>, music: bool) -> (Failed, String) {
    let (temp, options) = starter(name);
    let project = temp.path("demo");
    if !music {
        without_music(&project);
    }
    if let Some(k) = kind {
        cart_type(&project, k);
    }
    for (file, asm) in files {
        fs::write(project.join("src").join(file), asm).unwrap();
    }
    let err = build(&rgbds(), &options).unwrap_err();
    let body = err.body();
    let BuildError::Failed { made, .. } = err else {
        panic!("expected a failed build, got {err:?}");
    };
    let listing = entries(&project.join("build"));
    (
        Failed {
            made,
            message: body.message,
            remedy: body.remedy,
        },
        listing.join(" "),
    )
}

fn show(label: &str, f: &Failed, files: &str) {
    eprintln!(
        "=== {label} ===\nMESSAGE: {}\nREMEDY:  {}\nROWS:\n{}\nOVERFLOW: {}\nBUILD DIR: {files}",
        flat(&f.message),
        flat(&f.remedy),
        rows(&f.made),
        f.made.overflow.as_ref().map_or_else(
            || "none".to_string(),
            |o| format!(
                "cart ${:02X}, sections {:?}",
                o.cart_type,
                o.sections
                    .iter()
                    .map(|s| (&s.name, s.bank, s.start, s.bytes))
                    .collect::<Vec<_>>()
            )
        ),
    );
}

fn no_rom(files: &str) {
    for name in ["demo.gb", "demo.sym", "demo.map"] {
        assert!(!files.contains(name), "{name} was written: {files}");
    }
}

// 1. ROM-only, spilling past bank 1, plus a small ROMX section fixed BANK[2].
#[test]
fn case1_rom_only_spill_with_a_section_fixed_in_bank_2() {
    let (f, files) = failed(
        "verify-bank2",
        &[
            ("levels.asm", BIG),
            ("probe.asm", "SECTION \"Fixed\", ROMX, BANK[2]\ndb 1\n"),
        ],
        Some(0x00),
        false,
    );
    show("case1", &f, &files);
    let (message, rws) = (flat(&f.message), rows(&f.made));
    assert!(
        message.contains("must be in bank 1 (if any) with option '-t'"),
        "{message}"
    );
    assert!(!rws.contains("32 KB even"), "{rws}");
    assert!(!message.contains("32 KB even"), "{message}");
    let o = f.made.overflow.as_ref().expect("bank-2 overflow");
    assert!(
        o.sections.iter().any(|s| s.name == "Fixed" && s.bank == 2),
        "{o:?}"
    );
    no_rom(&files);
    assert!(
        f.made
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Warning
                && d.message.contains("linked again as one 32 KB bank")),
        "{rws}"
    );
}

// 2. Two ROMX sections fixed at $4000 in no bank: an overlap when linked -t.
#[test]
fn case2_two_romx_sections_fixed_at_4000() {
    let (f, files) = failed(
        "verify-overlap",
        &[(
            "probe.asm",
            "SECTION \"Left\", ROMX[$4000]\ndb 1\n\nSECTION \"Right\", ROMX[$4000]\ndb 2\n",
        )],
        Some(0x00),
        false,
    );
    show("case2", &f, &files);
    let (message, rws) = (flat(&f.message), rows(&f.made));
    assert!(message.contains("section overlaps with"), "{message}");
    assert!(!rws.contains("32 KB even"), "{rws}");
    assert!(!message.contains("32 KB even"), "{message}");
    assert!(f.made.overflow.is_some(), "{rws}");
    no_rom(&files);
}

// 3. ROM-only bank 0 past 16 KB but under 32 KB, plus `call Nowhere`: the
// first link stops at placement, and the one-bank link reaches the symbol.
#[test]
fn case3_full_bank_0_that_fits_in_32_kb_with_an_undefined_symbol() {
    let (f, files) = failed(
        "verify-bank0-undef",
        &[
            ("tables.asm", ROM0_BIG),
            ("probe.asm", "SECTION \"Uses\", ROM0\ncall Nowhere\n"),
        ],
        Some(0x00),
        true,
    );
    show("case3", &f, &files);
    let (message, rws) = (flat(&f.message), rows(&f.made));
    assert!(message.contains("Undefined symbol"), "{message}");
    assert!(message.contains("Nowhere"), "{message}");
    assert!(
        rws.contains("Unable to place \"Sound Driver\" (ROM0 section) anywhere"),
        "the first link did not stop at a full bank 0:\n{rws}"
    );
    assert!(
        rws.lines()
            .next()
            .is_some_and(|r| r.contains("Undefined symbol `Nowhere`")),
        "the one-bank link's symbol error must lead:\n{rws}"
    );
    assert!(f.made.overflow.is_none(), "{:?}", f.made.overflow);
    no_rom(&files);
}

// 4. A genuine size failure: 36 KB of ROMX. The report must not claim total
// size, and must name the section the one-bank link could not place, keeping
// rgblink's own row.
#[test]
fn case4_genuine_size_failure_names_the_unplaced_section() {
    let (f, files) = failed(
        "verify-too-big",
        &[("levels.asm", TOO_BIG)],
        Some(0x00),
        false,
    );
    show("case4", &f, &files);
    let (message, rws) = (flat(&f.message), rows(&f.made));
    assert!(!message.contains("don't fit in 32 KB"), "{message}");
    assert!(!rws.contains("32 KB even"), "{rws}");
    let name = unplaced_in(&rws);
    assert!(message.contains(&name), "{name}: {message}\n{rws}");
    assert!(
        rws.contains("(ROM0 section) anywhere"),
        "rgblink's row was discarded:\n{rws}"
    );
    assert!(f.remedy.contains("pinned"), "{}", f.remedy);
    assert!(f.remedy.contains("MBC"), "{}", f.remedy);
    assert!(f.made.overflow.is_some(), "{rws}");
    no_rom(&files);
}

// 5. A pure undefined symbol with no spill must not claim an overflow.
#[test]
fn case5_pure_undefined_symbol_is_not_an_overflow() {
    let (f, files) = failed(
        "verify-undef",
        &[("probe.asm", "SECTION \"Uses\", ROM0\ncall Nowhere\n")],
        Some(0x00),
        false,
    );
    show("case5", &f, &files);
    assert!(f.made.overflow.is_none(), "{:?}", f.made.overflow);
    assert!(
        flat(&f.message).contains("Undefined symbol"),
        "{}",
        f.message
    );
    assert!(!flat(&f.message).contains("32 KB"), "{}", f.message);
    no_rom(&files);
}

// 6. An MBC header with a full bank 0 and a legal ROMX BANK[2] section. The
// first link wrote no ROM, so the header is unknown: the one-bank link's
// bank-1 refusal must not lead or be an error, and its no-MBC hint must not
// attach. It is kept only as a warning saying it holds for a header with no
// MBC (leaf 5x5Z42), which for this header it doesn't.
#[test]
fn extra_a_mbc_full_bank_0_plus_a_bank_2_section_leads_with_bank_0() {
    let (f, files) = failed(
        "verify-mbc-bank2",
        &[
            ("tables.asm", ROM0_BIG),
            ("probe.asm", "SECTION \"Fixed\", ROMX, BANK[2]\ndb 1\n"),
        ],
        Some(0x01),
        true,
    );
    show("extra-a-mbc-bank0-bank2", &f, &files);
    let (message, rws, remedy) = (flat(&f.message), rows(&f.made), flat(&f.remedy));
    assert!(
        message.contains("Unable to place \"Sound Driver\" (ROM0 section) anywhere"),
        "{message}\n{rws}"
    );
    assert!(!message.contains("must be in bank 1"), "{message}");
    for d in
        f.made.diagnostics.iter().filter(|d| {
            d.message.contains("must be in bank 1") || d.message.contains("names no MBC")
        })
    {
        assert_eq!(d.severity, Severity::Warning, "{rws}");
        assert!(
            d.message.starts_with("only if the header names no MBC"),
            "{rws}"
        );
    }
    assert!(
        rws.contains("must be in bank 1"),
        "the row was dropped:\n{rws}"
    );
    // The surviving hint is the full-bank-0 one; the no-MBC hint's claim
    // and its BANK[n] advice must not appear.
    assert!(!remedy.contains("the header names no MBC, so"), "{remedy}");
    assert!(!remedy.contains("BANK["), "{remedy}");
    assert!(f.made.overflow.is_none(), "{:?}", f.made.overflow);
    no_rom(&files);
}

// 7. An MBC header and a ROMX BANK[2] section with room: builds, so the
// bank-1 refusal is specific to the no-MBC one-bank link.
#[test]
fn extra_b_mbc_bank_2_section_still_builds() {
    let (temp, options) = starter("verify-mbc-build");
    let project = temp.path("demo");
    without_music(&project);
    cart_type(&project, 0x01);
    fs::write(project.join("src/levels.asm"), BIG).unwrap();
    fs::write(
        project.join("src/probe.asm"),
        "SECTION \"Fixed\", ROMX, BANK[2]\ndb 1\n",
    )
    .unwrap();
    let made = build(&rgbds(), &options).unwrap();
    assert_eq!(
        fs::metadata(project.join("build/demo.gb")).unwrap().len(),
        65536
    );
    assert!(made.diagnostics.is_empty(), "{:?}", made.diagnostics);
}

// 8. A ROMX section fixed in BANK[1] beside a spill still builds.
#[test]
fn extra_c_rom_only_bank_1_section_still_builds() {
    let (temp, options) = starter("verify-bank1");
    let project = temp.path("demo");
    without_music(&project);
    cart_type(&project, 0x00);
    fs::write(project.join("src/levels.asm"), BIG).unwrap();
    fs::write(
        project.join("src/probe.asm"),
        "SECTION \"One\", ROMX[$7F00], BANK[1]\ndb $AB\n",
    )
    .unwrap();
    let made = build(&rgbds(), &options).unwrap();
    let rom = fs::read(project.join("build/demo.gb")).unwrap();
    assert_eq!(rom.len(), 32768);
    assert_eq!(rom[0x7F00], 0xAB);
    let warnings: Vec<_> = made
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Warning)
        .collect();
    assert_eq!(warnings.len(), 1, "{:?}", made.diagnostics);
    assert!(warnings[0].message.contains("one 32 KB bank"));
}

// 9. An MBC header and a full bank 0 with an undefined symbol: any one-bank
// error that is not the header-dependent bank-1 refusal still leads.
#[test]
fn extra_d_mbc_full_bank_0_with_an_undefined_symbol_still_leads_with_it() {
    let (f, files) = failed(
        "verify-mbc-undef",
        &[
            ("tables.asm", ROM0_BIG),
            ("probe.asm", "SECTION \"Uses\", ROM0\ncall Nowhere\n"),
        ],
        Some(0x01),
        true,
    );
    show("extra-d-mbc-bank0-undef", &f, &files);
    let (message, rws) = (flat(&f.message), rows(&f.made));
    assert!(message.contains("Undefined symbol `Nowhere`"), "{message}");
    assert!(
        rws.contains("Unable to place \"Sound Driver\" (ROM0 section) anywhere"),
        "{rws}"
    );
    assert!(f.made.overflow.is_none(), "{:?}", f.made.overflow);
    no_rom(&files);
}

// 10. Pinned sections split the 32 KB space so a 16 KB section fits in no
// gap, though 6.5 KB is free. The report must not claim the code does not
// fit in 32 KB; it names "Wide" and keeps rgblink's row; the remedy offers
// moving a pinned section.
#[test]
fn extra_e_fragmented_space_is_not_a_total_size_failure() {
    let (temp, options) = starter("verify-fragmented");
    let project = temp.path("demo");
    without_music(&project);
    cart_type(&project, 0x00);
    for (file, asm) in [
        ("one.asm", "SECTION \"Pin\", ROMX[$4000]\ndb 1\n"),
        ("four.asm", "SECTION \"Pin2\", ROMX[$5FFF]\ndb 1\n"),
        ("two.asm", "SECTION \"Wide\", ROMX\nds $4000, 2\n"),
        ("three.asm", "SECTION \"Narrow\", ROMX\nds $2000, 3\n"),
    ] {
        fs::write(project.join("src").join(file), asm).unwrap();
    }
    let err = build(&rgbds(), &options).unwrap_err();
    let body = err.body();
    let BuildError::Failed { made, .. } = err else {
        panic!("expected a failed build, got {err:?}");
    };
    let f = Failed {
        made,
        message: body.message,
        remedy: body.remedy,
    };
    let files = entries(&project.join("build")).join(" ");
    show("extra-e-fragmented", &f, &files);
    let (message, rws, remedy) = (flat(&f.message), rows(&f.made), flat(&f.remedy));
    assert!(!message.contains("don't fit in 32 KB"), "{message}");
    assert!(!rws.contains("32 KB even"), "{rws}");
    assert_eq!(unplaced_in(&rws), "Wide", "{rws}");
    assert!(message.contains("Wide"), "{message}");
    assert!(remedy.contains("pinned"), "{remedy}");
    assert!(remedy.contains("MBC"), "{remedy}");
    let o = f.made.overflow.as_ref().expect("overflow");
    assert!(
        o.sections.iter().any(|s| s.name == "Wide" && s.bank == 2),
        "{o:?}"
    );
    no_rom(&files);

    // The same project without the 16 KB section proves the old claim false:
    // the failed -t link had room in total, only not in one piece.
    fs::remove_file(project.join("src/two.asm")).unwrap();
    build(&rgbds(), &options).unwrap();
    let map = fs::read_to_string(project.join("build/demo.map")).unwrap();
    let used = map_used(&map, "ROM0") + map_used(&map, "ROMX") + 0x4000;
    eprintln!("extra-e total with Wide: {used} of 32768 bytes");
    assert!(used < 0x8000, "{used} of 32768 is not a total-size failure");
}

/// The bytes used in `bank`'s line of an rgblink `.map` SUMMARY.
fn map_used(map: &str, bank: &str) -> u64 {
    let line = map
        .lines()
        .find(|l| l.trim_start().starts_with(&format!("{bank}: ")))
        .unwrap_or_else(|| panic!("{bank} in the map:\n{map}"));
    line.split_whitespace().nth(1).unwrap().parse().unwrap()
}
