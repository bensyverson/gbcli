//! The bank check after the link: a header that names no MBC (cartridge
//! type $00, $08 or $09) makes a cartridge that maps banks 0 and 1 only, so
//! a section rgblink placed in bank 2 or later is unreachable, and the code
//! that reads it reads bank 1's bytes. rgblink doesn't know the cartridge
//! and says nothing; this reads the header byte and the `.map`. Such a
//! cartridge maps 32 KB flat, so the build links it again as one bank
//! (`rgblink --tiny`) when rgblink spreads it past bank 1, can't fit bank 0,
//! or refuses a section bigger than a 16 KB bank.

use gbcli_protocol::{BankOverflow, Diagnostic, PlacedSection, Severity, Tool};

/// Where the header keeps the cartridge type.
const CART_TYPE: usize = 0x0147;

/// The cartridge types with no MBC: ROM only, ROM+RAM, ROM+RAM+battery.
const NO_MBC: [u8; 3] = [0x00, 0x08, 0x09];

/// rgblink's message for a ROM0 section that fits nowhere in bank 0
/// (RGBDS 1.0.4: `Unable to place "NAME" (ROM0 section) anywhere`), as
/// opposed to one that overlaps another at a fixed address.
pub(super) fn bank_0_full(d: &Diagnostic) -> bool {
    unplaced(d).is_some()
}

/// The section rgblink couldn't place anywhere: `Unable to place "NAME"
/// (ROM0 section) anywhere`. The space may be fragmented by a pinned
/// section rather than full, so callers must not read it as a total-size
/// failure.
pub(super) fn unplaced(d: &Diagnostic) -> Option<&str> {
    if d.tool != Tool::Rgblink || d.severity != Severity::Error {
        return None;
    }
    d.message
        .strip_prefix("Unable to place \"")?
        .strip_suffix("\" (ROM0 section) anywhere")
}

/// rgblink's refusal of a section bigger than its type allows in 16 KB banks
/// (RGBDS 1.0.4: `Section "NAME" is bigger than the max size for that type:
/// $5000 > $4000`). The max of $4000 is a ROM bank's; linked as one bank, a
/// ROM0 section (or ROM0 fragments adding up past 16 KB) may fill 32 KB.
/// rgbasm refuses a single ROMX section over 16 KB before the link, so none
/// reaches here.
pub(super) fn outgrows_a_bank(d: &Diagnostic) -> bool {
    d.tool == Tool::Rgblink
        && d.severity == Severity::Error
        && d.message.starts_with("Section \"")
        && d.message
            .contains("\" is bigger than the max size for that type: $")
        && d.message.ends_with(" > $4000")
}

/// The starts of the one-bank link's errors that no placement changes. An
/// undefined symbol is undefined in any layout; RGBDS 1.0.4 prints
/// ``Undefined symbol `NAME` ``.
const ANY_LAYOUT: [&str; 1] = ["Undefined symbol `"];

/// Whether a one-bank link's error holds whatever the layout, so it may
/// lead a report made when the first link wrote no ROM and the header is
/// unknown. An allow-list: any row not on it (an overlap, an assert, a `jr`
/// out of range, an unplaced section, a ROMX bank past 1) may exist only
/// because the one-bank link placed things where 16 KB banks wouldn't.
pub(super) fn true_in_any_layout(d: &Diagnostic) -> bool {
    d.tool == Tool::Rgblink
        && d.severity == Severity::Error
        && ANY_LAYOUT.iter().any(|start| d.message.starts_with(start))
}

/// Whether `rom`'s header names no MBC: a cartridge that maps 32 KB flat.
pub(super) fn names_no_mbc(rom: &[u8]) -> bool {
    rom.get(CART_TYPE).is_some_and(|t| NO_MBC.contains(t))
}

/// The overflow, when `rom`'s header names no MBC and `map` places
/// sections past bank 1.
pub(super) fn overflow(rom: &[u8], map: &str) -> Option<BankOverflow> {
    if !names_no_mbc(rom) {
        return None;
    }
    let cart_type = rom[CART_TYPE];
    let sections = past_bank_1(map);
    (!sections.is_empty()).then_some(BankOverflow {
        cart_type,
        sections,
    })
}

/// Every section an rgblink `.map` places in a ROMX bank past 1.
fn past_bank_1(map: &str) -> Vec<PlacedSection> {
    let mut bank = None;
    let mut found = Vec::new();
    for line in map.lines() {
        if !line.starts_with(char::is_whitespace) {
            // A bank heading (`ROMX bank #2:`) or the summary.
            bank = line
                .strip_prefix("ROMX bank #")
                .and_then(|r| r.strip_suffix(':'))
                .and_then(|n| n.parse::<u32>().ok());
            continue;
        }
        let Some(b) = bank.filter(|b| *b > 1) else {
            continue;
        };
        if let Some(section) = line.trim().strip_prefix("SECTION: ") {
            found.extend(placed(section, b));
        }
    }
    found
}

/// `$4000-$65bb ($25bc bytes) ["name"]`, or `$4000 ($0 bytes) ["name"]` for
/// an empty section.
fn placed(line: &str, bank: u32) -> Option<PlacedSection> {
    let hex = |s: &str| s.strip_prefix('$').map(|h| u32::from_str_radix(h, 16));
    let start = hex(line.split(['-', ' ']).next()?)?.ok()?;
    let size = line.split_once('(')?.1.split_once(' ')?.0;
    let bytes = hex(size)?.ok()?;
    let name = line.split_once("[\"")?.1.strip_suffix("\"]")?;
    Some(PlacedSection {
        name: name.to_string(),
        bank,
        start: u16::try_from(start).ok()?,
        bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAP: &str = "SUMMARY:\n\tROM0: 2304 bytes used / 14080 free\n\tROMX: 23200 bytes used / 9568 free in 2 banks\n\nROM0 bank #0:\n\tSECTION: $0000-$0007 ($0008 bytes) [\"Start\"]\n\nROMX bank #1:\n\tSECTION: $4000-$74e3 ($34e4 bytes) [\"song_big Song Data\"]\n\t         $4000 = song_big\n\tEMPTY: $0b1c bytes\n\nROMX bank #2:\n\tSECTION: $4000-$65bb ($25bc bytes) [\"song_theme Song Data\"]\n\t         $4000 = song_theme\n\tSECTION: $65bc ($0000 bytes) [\"Empty one\"]\n\nROMX bank #3:\n\tSECTION: $4000-$4001 ($0002 bytes) [\"Tail\"]\n\nWRAM0 bank #0:\n\tSECTION: $c000-$c0ff ($0100 bytes) [\"Vars\"]\n";

    fn rom(cart_type: u8) -> Vec<u8> {
        let mut rom = vec![0; 0x8000];
        rom[CART_TYPE] = cart_type;
        rom
    }

    #[test]
    fn the_sections_past_bank_1_are_read_from_the_map() {
        let o = overflow(&rom(0), MAP).unwrap();
        assert_eq!(o.cart_type, 0);
        let got: Vec<(&str, u32, u16, u32)> = o
            .sections
            .iter()
            .map(|s| (s.name.as_str(), s.bank, s.start, s.bytes))
            .collect();
        assert_eq!(
            got,
            [
                ("song_theme Song Data", 2, 0x4000, 0x25bc),
                ("Empty one", 2, 0x65bc, 0),
                ("Tail", 3, 0x4000, 2),
            ]
        );
    }

    #[test]
    fn a_header_with_an_mbc_or_a_map_within_bank_1_is_no_overflow() {
        assert_eq!(overflow(&rom(0x01), MAP), None);
        assert_eq!(overflow(&rom(0x1B), MAP), None);
        let fits = MAP.split("ROMX bank #2").next().unwrap();
        assert_eq!(overflow(&rom(0), fits), None);
        for kind in NO_MBC {
            assert!(overflow(&rom(kind), MAP).is_some());
        }
    }

    /// Only rgblink's "anywhere" for a ROM0 section is a full bank 0; an
    /// overlap at a fixed address isn't, nor is the message from rgbasm.
    #[test]
    fn a_full_bank_0_is_rgblinks_rom0_section_placed_nowhere() {
        let d = |tool, message: &str| Diagnostic::new(tool, Severity::Error, message);
        let full = "Unable to place \"Sound Driver\" (ROM0 section) anywhere";
        assert!(bank_0_full(&d(Tool::Rgblink, full)));
        assert!(!bank_0_full(&d(Tool::Rgbasm, full)));
        assert!(!bank_0_full(&d(
            Tool::Rgblink,
            "Unable to place \"A\" (ROM0 section) at address $0150: section overlaps with \"B\""
        )));
        assert!(!bank_0_full(&Diagnostic::new(
            Tool::Rgblink,
            Severity::Warning,
            full
        )));
    }

    /// An unplaced section's name is read from rgblink's "anywhere" row and
    /// nowhere else; an overlap at a fixed address names two sections, not
    /// one that found nowhere to go.
    #[test]
    fn an_unplaced_rom0_section_names_itself() {
        let d = |tool, message: &str| Diagnostic::new(tool, Severity::Error, message);
        let anywhere = "Unable to place \"Wide\" (ROM0 section) anywhere";
        assert_eq!(unplaced(&d(Tool::Rgblink, anywhere)), Some("Wide"));
        assert_eq!(unplaced(&d(Tool::Rgbasm, anywhere)), None);
        assert_eq!(
            unplaced(&Diagnostic::new(Tool::Rgblink, Severity::Warning, anywhere)),
            None
        );
        assert_eq!(
            unplaced(&d(
                Tool::Rgblink,
                "Unable to place \"A\" (ROM0 section) at address $4000: section overlaps with \
                 \"B\""
            )),
            None
        );
    }

    /// A section bigger than a 16 KB bank is a refusal one flat 32 KB bank
    /// may cure; a RAM section past its own size, or the message from
    /// rgbasm, isn't.
    #[test]
    fn a_section_bigger_than_a_rom_bank_is_recognized() {
        let d = |tool, message: &str| Diagnostic::new(tool, Severity::Error, message);
        let huge = "Section \"Huge\" is bigger than the max size for that type: $5000 > $4000";
        assert!(outgrows_a_bank(&d(Tool::Rgblink, huge)));
        assert!(!outgrows_a_bank(&d(Tool::Rgbasm, huge)));
        assert!(!outgrows_a_bank(&Diagnostic::new(
            Tool::Rgblink,
            Severity::Warning,
            huge
        )));
        assert!(!outgrows_a_bank(&d(
            Tool::Rgblink,
            "Section \"Vars\" is bigger than the max size for that type: $1100 > $1000"
        )));
    }

    /// Only a row whose truth no placement changes may lead a report made
    /// without the header: an undefined symbol. An overlap, an assert, an
    /// unplaced section, a `jr` out of range and the bank-1 refusal all
    /// depend on where the one-bank link put things.
    #[test]
    fn only_an_undefined_symbol_is_true_in_any_layout() {
        let d = |message: &str| Diagnostic::new(Tool::Rgblink, Severity::Error, message);
        assert!(true_in_any_layout(&d("Undefined symbol `Nowhere`")));
        for layout in [
            "Unable to place \"Left\" (ROM0 section) at address $4000: section overlaps with \
             \"Right\"",
            "Banked code must live in ROMX",
            "Unable to place \"Wide\" (ROM0 section) anywhere",
            "jr target must be between -128 and 127 bytes away, not 300; use jp instead",
            "Section \"Fixed\" has type `ROMX`, which must be in bank 1 (if any) with option '-t'",
        ] {
            assert!(!true_in_any_layout(&d(layout)), "{layout}");
        }
        let warning = Diagnostic::new(
            Tool::Rgblink,
            Severity::Warning,
            "Undefined symbol `Nowhere`",
        );
        assert!(!true_in_any_layout(&warning));
    }
}
