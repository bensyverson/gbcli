//! Regression tests for the one-bank relink when the first link wrote no ROM
//! (a full bank 0, or a section bigger than a 16 KB bank), so the header is
//! unknown: the report leads with the first link's error unless a one-bank
//! row is true whatever the layout, and a row true only of the one-bank
//! layout is kept as a warning that says so. Written from the review of
//! 9c4ca11 (leaf 5x5Z42).

use std::fs;
use std::path::Path;

use gbcli_build::{BuildError, build};
use gbcli_protocol::{BuildMade, Severity};

use crate::common::{rgbds, starter, without_music};

/// 15 KB of ROM0: with the starter and its music, past bank 0's 16 KB.
const ROM0_BIG: &str = "SECTION \"Tables\", ROM0\nds $3C00, 1\n";

/// The first link's row for a full bank 0 beside the starter's music.
const SOUND_DRIVER_UNPLACED: &str = "Unable to place \"Sound Driver\" (ROM0 section) anywhere";

/// How a row true only of the one-bank layout begins.
const ONE_BANK_ONLY: &str = "from the one-bank link";

fn cart_type(project: &Path, kind: u8) {
    let main = project.join("src/main.asm");
    let text = fs::read_to_string(&main).unwrap();
    let header = format!("ds $0147 - @, 0\n\tdb ${kind:02X}\n\tds $0150 - @, 0");
    assert!(text.contains("ds $0150 - @, 0"));
    fs::write(&main, text.replace("ds $0150 - @, 0", &header)).unwrap();
}

fn flat(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn rows(made: &BuildMade) -> String {
    made.diagnostics
        .iter()
        .enumerate()
        .map(|(i, d)| format!("{i}: {:?} {:?}: {}", d.tool, d.severity, d.message))
        .collect::<Vec<_>>()
        .join("\n")
}

struct Report {
    message: String,
    remedy: String,
    made: Box<BuildMade>,
}

impl Report {
    fn rows(&self) -> String {
        rows(&self.made)
    }

    /// The error rows' messages.
    fn errors(&self) -> Vec<&str> {
        self.made
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .map(|d| d.message.as_str())
            .collect()
    }

    /// The warning rows' messages.
    fn warnings(&self) -> Vec<&str> {
        self.made
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .map(|d| d.message.as_str())
            .collect()
    }
}

/// The failed build of the starter plus `files`, with cartridge type `kind`.
fn report(name: &str, files: &[(&str, &str)], kind: u8, music: bool) -> Report {
    let (temp, options) = starter(name);
    let project = temp.path("demo");
    if !music {
        without_music(&project);
    }
    cart_type(&project, kind);
    for (file, asm) in files {
        fs::write(project.join("src").join(file), asm).unwrap();
    }
    let err = build(&rgbds(), &options).unwrap_err();
    let body = err.body();
    let BuildError::Failed { made, .. } = err else {
        panic!("expected a failed build, got {err:?}");
    };
    assert!(!project.join("build/demo.gb").exists());
    let r = Report {
        message: flat(&body.message),
        remedy: flat(&body.remedy),
        made,
    };
    eprintln!(
        "=== {name} ===\nMESSAGE: {}\nREMEDY: {}\nROWS:\n{}",
        r.message,
        r.remedy,
        r.rows()
    );
    r
}

/// Asserts the one-bank link's row containing `fragment` is kept, but only
/// as a warning that says it holds for a header with no MBC.
fn kept_as_one_bank_only(r: &Report, fragment: &str) {
    assert!(
        !r.errors().iter().any(|m| m.contains(fragment)),
        "a one-bank-only row is an error:\n{}",
        r.rows()
    );
    assert!(
        r.warnings()
            .iter()
            .any(|m| m.starts_with(ONE_BANK_ONLY) && m.contains(fragment)),
        "the one-bank link's row was dropped or not marked:\n{}",
        r.rows()
    );
}

/// An MBC1 header, a full bank 0, and two ROMX sections pinned at $4000 in
/// no bank: legal under an MBC (banks 1 and 2), an overlap only as one bank.
/// The one-bank link's overlap must not lead.
#[test]
fn mbc_full_bank_0_with_two_romx_pins_at_4000_leads_with_bank_0() {
    let r = report(
        "relink-mbc-overlap",
        &[
            ("tables.asm", ROM0_BIG),
            (
                "pins.asm",
                "SECTION \"Left\", ROMX[$4000]\ndb 1\n\nSECTION \"Right\", ROMX[$4000]\ndb 2\n",
            ),
        ],
        0x01,
        true,
    );
    assert!(r.message.contains(SOUND_DRIVER_UNPLACED), "{}", r.message);
    assert!(
        !r.message.contains("section overlaps with"),
        "{}",
        r.message
    );
    assert_eq!(r.errors(), [SOUND_DRIVER_UNPLACED], "{}", r.rows());
    kept_as_one_bank_only(&r, "section overlaps with \"Right\"");
}

/// An MBC1 header, a full bank 0, and a link-time ASSERT that banked code
/// sits in ROMX: true in 16 KB banks, false only where the one-bank link
/// places the section below $4000. The assert must not lead.
#[test]
fn mbc_full_bank_0_with_a_placement_assert_leads_with_bank_0() {
    let r = report(
        "relink-mbc-assert",
        &[
            ("tables.asm", ROM0_BIG),
            (
                "banked.asm",
                "SECTION \"Banked\", ROMX\nBanked:\n\tret\n\tassert Banked >= $4000, \
                 \"Banked code must live in ROMX\"\n",
            ),
        ],
        0x01,
        true,
    );
    assert!(r.message.contains(SOUND_DRIVER_UNPLACED), "{}", r.message);
    assert!(
        !r.message.contains("Banked code must live in ROMX"),
        "{}",
        r.message
    );
    kept_as_one_bank_only(&r, "Banked code must live in ROMX");
    // The kept row still says where the assert is.
    let assert_row = r
        .made
        .diagnostics
        .iter()
        .find(|d| d.message.contains("Banked code must live in ROMX"))
        .unwrap();
    assert!(
        assert_row
            .file
            .as_deref()
            .is_some_and(|f| f.ends_with("banked.asm")),
        "{assert_row:?}"
    );
}

/// No MBC and one 20 KB ROM0 section: rgblink refuses it in 16 KB banks
/// ("bigger than the max size for that type"), but such a cartridge maps
/// 32 KB flat, and linked as one bank it fits.
#[test]
fn no_mbc_rom0_section_over_16_kb_links_as_one_bank() {
    let (temp, options) = starter("relink-nombc-huge-rom0");
    let project = temp.path("demo");
    without_music(&project);
    cart_type(&project, 0x00);
    fs::write(
        project.join("src/huge.asm"),
        "SECTION \"Huge\", ROM0\nds $5000, 1\n",
    )
    .unwrap();
    let made = match build(&rgbds(), &options) {
        Ok(made) => made,
        Err(err) => panic!("{}", flat(&err.body().message)),
    };
    assert_eq!(made.diagnostics.len(), 1, "{}", rows(&made));
    assert_eq!(made.diagnostics[0].severity, Severity::Warning);
    assert!(
        made.diagnostics[0]
            .message
            .contains("linked as one 32 KB bank (rgblink -t)"),
        "{}",
        rows(&made)
    );
    let rom = fs::read(project.join("build/demo.gb")).unwrap();
    assert_eq!(rom.len(), 0x8000);
}

/// The same 20 KB section under an MBC is too big for any bank: the first
/// link's refusal leads, and the build is not accepted as one bank.
#[test]
fn mbc_rom0_section_over_16_kb_still_fails_with_rgblinks_error() {
    let r = report(
        "relink-mbc-huge-rom0",
        &[("huge.asm", "SECTION \"Huge\", ROM0\nds $5000, 1\n")],
        0x01,
        false,
    );
    assert!(
        r.message.contains("bigger than the max size for that type"),
        "{}",
        r.message
    );
    assert!(
        !r.warnings().iter().any(|m| m.contains("linked as one")),
        "{}",
        r.rows()
    );
}

/// No MBC, a full bank 0, and pins that split the rest of 32 KB so one
/// ROMX section fits no gap though the total fits. The header is unknown,
/// so the report leads with bank 0; its hint claims no total size, and the
/// one-bank link's row naming the section it couldn't place is kept.
#[test]
fn no_mbc_full_bank_0_fragmented_claims_no_total_size() {
    let r = report(
        "relink-nombc-bank0-fragmented",
        &[
            ("tables.asm", ROM0_BIG),
            (
                "pins.asm",
                "SECTION \"Pin\", ROMX[$4000]\ndb 1\n\nSECTION \"Pin2\", ROMX[$5FFF]\ndb 1\n",
            ),
            ("wide.asm", "SECTION \"Wide\", ROMX\nds $2100, 2\n"),
        ],
        0x00,
        true,
    );
    assert!(r.message.contains(SOUND_DRIVER_UNPLACED), "{}", r.message);
    assert!(!r.remedy.contains("don't fit"), "{}", r.remedy);
    assert!(r.remedy.contains("pinned"), "{}", r.remedy);
    kept_as_one_bank_only(&r, "Unable to place \"Wide\" (ROM0 section) anywhere");
}

/// The control for the fragmented test: without "Wide" the project links as
/// one bank, and its 32 KB then has room for Wide's $2100 bytes in total, so
/// that test's failure is fragmentation, not size.
#[test]
fn control_fragmented_project_has_room_in_total() {
    let (temp, options) = starter("relink-nombc-bank0-fragmented-control");
    let project = temp.path("demo");
    cart_type(&project, 0x00);
    fs::write(project.join("src/tables.asm"), ROM0_BIG).unwrap();
    fs::write(
        project.join("src/pins.asm"),
        "SECTION \"Pin\", ROMX[$4000]\ndb 1\n\nSECTION \"Pin2\", ROMX[$5FFF]\ndb 1\n",
    )
    .unwrap();
    build(&rgbds(), &options).unwrap();
    let map = fs::read_to_string(project.join("build/demo.map")).unwrap();
    let used: u64 = map
        .lines()
        .filter(|l| l.trim_start().starts_with("ROM0: ") || l.trim_start().starts_with("ROMX: "))
        .map(|l| l.split_whitespace().nth(1).unwrap().parse::<u64>().unwrap())
        .sum();
    assert!(used + 0x2100 < 0x8000, "{used} + $2100 of 32768");
}
