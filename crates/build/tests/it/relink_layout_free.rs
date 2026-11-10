//! From the verification of ffe3e7b (nWjPtM): a full bank 0 under an MBC
//! header links once as one bank only to say what else is wrong, and its rows
//! must not claim more than is true. An error that holds in any layout but is
//! not on the allow-list (an `assert`, a far `jr`) still shows as a warning,
//! worded "may hold"; and rgbasm, not the link, refuses one ROMX section over
//! 16 KB, so that case never reaches the one-bank link.

use std::fs;
use std::path::Path;

use gbcli_build::{BuildError, build};
use gbcli_protocol::{BuildMade, Severity};

use crate::common::{rgbds, starter, without_music};

/// ROM0 filled so the starter's driver can't be placed in bank 0.
const ROM0_BIG: &str = "SECTION \"Tables\", ROM0\nds $3C00, 1\n";
const SOUND: &str = "Unable to place \"Sound Driver\" (ROM0 section) anywhere";
const ONE_BANK: &str =
    "from the one-bank link (rgblink -t), which may hold only if the header names no MBC";

/// Sets the cartridge type byte of the starter at `project`.
fn cart_type(project: &Path, kind: u8) {
    let main = project.join("src/main.asm");
    let text = fs::read_to_string(&main).unwrap();
    let header = format!("ds $0147 - @, 0\n\tdb ${kind:02X}\n\tds $0150 - @, 0");
    fs::write(&main, text.replace("ds $0150 - @, 0", &header)).unwrap();
}

fn rows(made: &BuildMade) -> String {
    made.diagnostics
        .iter()
        .map(|d| format!("{:?} {:?} {:?}: {}", d.tool, d.severity, d.file, d.message))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Builds the starter with `files` added under src/, header type `kind`.
fn build_with(
    name: &str,
    files: &[(&str, &str)],
    kind: u8,
    music: bool,
) -> Result<BuildMade, Box<(String, BuildMade)>> {
    let (temp, options) = starter(name);
    let project = temp.path("demo");
    if !music {
        without_music(&project);
    }
    cart_type(&project, kind);
    for (file, asm) in files {
        fs::write(project.join("src").join(file), asm).unwrap();
    }
    match build(&rgbds(), &options) {
        Ok(made) => Ok(made),
        Err(err) => {
            let message = err.body().message;
            let BuildError::Failed { made, .. } = err else {
                panic!("{err:?}");
            };
            Err(Box::new((message, *made)))
        }
    }
}

fn failed(name: &str, files: &[(&str, &str)], kind: u8, music: bool) -> (String, BuildMade) {
    *build_with(name, files, kind, music).expect_err("the build fails")
}

fn first_error(made: &BuildMade) -> &str {
    made.diagnostics
        .iter()
        .find(|d| d.severity == Severity::Error)
        .map_or("", |d| d.message.as_str())
}

/// The row whose message holds `fragment`.
fn row<'a>(made: &'a BuildMade, fragment: &str) -> &'a gbcli_protocol::Diagnostic {
    made.diagnostics
        .iter()
        .find(|d| d.message.contains(fragment))
        .unwrap_or_else(|| panic!("a row with {fragment:?} in\n{}", rows(made)))
}

/// A link-time assert whose truth no placement changes: the one-bank link's
/// row for it is demoted, and must say "may hold", not that it needs no MBC.
#[test]
fn a_layout_free_assert_under_a_full_bank_0_is_a_warning_that_may_hold() {
    let (_, made) = failed(
        "lf-sizeof",
        &[
            ("tables.asm", ROM0_BIG),
            ("small.asm", "SECTION \"Small\", ROMX\n\tret\n"),
            (
                "chk.asm",
                "SECTION \"Chk\", ROMX\n\tassert SIZEOF(\"Small\") == 99, \"Small must be 99 bytes\"\n\tret\n",
            ),
        ],
        0x01,
        true,
    );
    assert!(first_error(&made).contains(SOUND), "{}", rows(&made));
    let assert = row(&made, "Small must be 99");
    assert_eq!(assert.severity, Severity::Warning, "{}", rows(&made));
    assert!(assert.message.starts_with(ONE_BANK), "{}", assert.message);
    assert!(assert.file.is_some(), "the assert's row keeps its file");
}

/// A `jr` between ROMX sections more than 127 bytes apart is out of range in
/// any layout; it too is only a warning from the one-bank link.
#[test]
fn a_far_jr_under_a_full_bank_0_is_a_warning_that_may_hold() {
    let (_, made) = failed(
        "lf-farjr",
        &[
            ("tables.asm", ROM0_BIG),
            (
                "t.asm",
                "SECTION \"T\", ROMX\nds $200, 0\nTarget::\n\tret\n",
            ),
            ("j.asm", "SECTION \"J\", ROMX\n\tjr Target\nds $200, 0\n"),
        ],
        0x01,
        true,
    );
    assert!(first_error(&made).contains(SOUND), "{}", rows(&made));
    let jr = row(&made, "`JR` target");
    assert_eq!(jr.severity, Severity::Warning, "{}", rows(&made));
    assert!(jr.message.starts_with(ONE_BANK), "{}", jr.message);
}

/// A `jr` from ROMX to a ROM0 label is out of range in the 16 KB layout but
/// not an error that leads; the full bank 0 does.
#[test]
fn a_jr_across_sections_under_a_full_bank_0_does_not_lead() {
    let (_, made) = failed(
        "lf-jr",
        &[
            ("tables.asm", ROM0_BIG),
            ("t.asm", "SECTION \"T\", ROM0\nTarget::\n\tret\n"),
            ("j.asm", "SECTION \"J\", ROMX\n\tjr Target\n"),
        ],
        0x01,
        true,
    );
    assert!(first_error(&made).contains(SOUND), "{}", rows(&made));
}

/// A symbol defined twice is reported by the first link itself, as an error
/// that leads, not a demoted one.
#[test]
fn a_duplicate_symbol_under_a_full_bank_0_stays_an_error() {
    let (_, made) = failed(
        "lf-dup",
        &[
            ("tables.asm", ROM0_BIG),
            ("a.asm", "SECTION \"A\", ROMX\nDup::\n\tret\n"),
            ("b.asm", "SECTION \"B\", ROMX\nDup::\n\tret\n"),
        ],
        0x01,
        true,
    );
    assert_eq!(
        row(&made, "Dup").severity,
        Severity::Error,
        "{}",
        rows(&made)
    );
    assert_eq!(
        first_error(&made),
        row(&made, "Dup").message,
        "{}",
        rows(&made)
    );
}

/// An undefined symbol is on the allow-list: it leads, as an error, ahead of
/// the full bank 0, which is still reported.
#[test]
fn an_undefined_symbol_under_a_full_bank_0_still_leads_as_an_error() {
    let (_, made) = failed(
        "lf-undef",
        &[
            ("tables.asm", ROM0_BIG),
            ("u.asm", "SECTION \"U\", ROMX\n\tcall Nowhere\n"),
        ],
        0x01,
        true,
    );
    assert!(
        first_error(&made).contains("Undefined symbol"),
        "{}",
        rows(&made)
    );
    assert!(
        made.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error && d.message.contains(SOUND)),
        "{}",
        rows(&made)
    );
}

/// rgbasm refuses a single ROMX section over 16 KB whatever the header, so
/// the one-bank link never sees one.
#[test]
fn a_single_romx_section_over_16_kb_is_refused_by_rgbasm_with_or_without_an_mbc() {
    for (name, kind) in [("lf-romx-nombc", 0x00), ("lf-romx-mbc", 0x01)] {
        let (_, made) = failed(
            name,
            &[("h.asm", "SECTION \"HugeX\", ROMX\nds $5000, 1\n")],
            kind,
            false,
        );
        let refusal = made
            .diagnostics
            .iter()
            .find(|d| d.severity == Severity::Error)
            .unwrap_or_else(|| panic!("an error row in\n{}", rows(&made)));
        assert_eq!(
            refusal.tool,
            gbcli_protocol::Tool::Rgbasm,
            "{}",
            rows(&made)
        );
        assert!(
            !made
                .diagnostics
                .iter()
                .any(|d| d.message.contains("one 32 KB bank")),
            "no one-bank link ran: {}",
            rows(&made)
        );
    }
}

/// ROM0 fragments adding past 16 KB link as one bank without an MBC, and
/// fail under one.
#[test]
fn rom0_fragments_over_16_kb_link_as_one_bank_only_without_an_mbc() {
    let fragments = [
        ("f1.asm", "SECTION FRAGMENT \"Frag\", ROM0\nds $3000, 1\n"),
        ("f2.asm", "SECTION FRAGMENT \"Frag\", ROM0\nds $3000, 2\n"),
    ];
    let made =
        build_with("lf-frag-nombc", &fragments, 0x00, false).expect("one 32 KB bank holds it");
    assert!(
        made.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Warning
                && d.message.contains("linked as one 32 KB bank")),
        "{}",
        rows(&made)
    );
    assert!(build_with("lf-frag-mbc", &fragments, 0x01, false).is_err());
}

/// A RAM section too big is a plain failure: the one-bank link is for ROM.
#[test]
fn a_wram0_section_too_big_is_a_plain_failure() {
    let (_, made) = failed(
        "lf-wram",
        &[("w.asm", "SECTION \"W\", WRAM0\nds $1100\n")],
        0x00,
        false,
    );
    assert!(
        !made
            .diagnostics
            .iter()
            .any(|d| d.message.contains("one 32 KB bank")),
        "{}",
        rows(&made)
    );
}
