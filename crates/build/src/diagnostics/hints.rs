//! What to do about an RGBDS message whose fix the message itself doesn't
//! say. A `build_failed` remedy names the first error's hint, when it has
//! one, in place of the generic "fix the first error".

use gbcli_protocol::{Diagnostic, Tool};

/// A message, recognized by a fragment of its text, and what fixes it.
struct Hint {
    tool: Tool,
    /// A piece of the message that stays the same whatever it names.
    fragment: &'static str,
    /// What to do, in a form that reads before "before the first such line
    /// (FILE:LINE)".
    remedy: &'static str,
}

/// The `-t` refusal of a ROMX section fixed past bank 1. Its truth depends on
/// the header naming no MBC, so both this hint and `project::banks`'s filter
/// find the row by this one fragment.
pub(crate) const ROMX_BANK_PAST_1: &str = "must be in bank 1 (if any) with option '-t'";

/// RGBDS 1.0.4's wording.
const HINTS: &[Hint] = &[
    Hint {
        tool: Tool::Rgbasm,
        // `Section "W" cannot contain code or data (not `ROM0` or `ROMX`)`:
        // an instruction or `db` in a RAM section.
        fragment: "cannot contain code or data (not `ROM0` or `ROMX`)",
        remedy: "code and data go in a ROM0 or ROMX section: start one (`SECTION \"Code\", ROM0`)",
    },
    Hint {
        tool: Tool::Rgblink,
        // `Unable to place "rst08" (ROM0 section) at address $0008: section
        // overlaps with "Restart vectors"`: an rst routine in a section of
        // its own, over the starter's fill of $0000-$003F. rgblink names no
        // file or line, so the remedy says where the fill is.
        fragment: "section overlaps with \"Restart vectors\"",
        remedy: "the starter's \"Restart vectors\" section in src/main.asm fills all of \
                 $0000-$003F (`ds $40, $FF`): drop your section and put the routine in that \
                 one, replacing the fill up to its address and padding after it, e.g. \
                 `ds $08, $FF`, the routine, then `ds $40 - @, $FF`",
    },
    Hint {
        tool: Tool::Rgblink,
        // `Unable to place "Sound Driver" (ROM0 section) anywhere`: bank 0
        // is full or, for a header with no MBC, which the build then links
        // as one bank (`project`), all 32 KB is.
        fragment: "(ROM0 section) anywhere",
        remedy: "the sections don't fit: bank 0 holds 16 KB ($0000-$3FFF), or 32 KB when the \
                 header names no MBC and the build links it as one bank. Shrink them or move \
                 what needn't be in bank 0 into ROMX (`SECTION \"Name\", ROMX`); past 32 KB, \
                 name an MBC in the header (the byte at $0147, e.g. $01 for MBC1) and switch \
                 banks (write the bank number to $2000) around the code that reads each \
                 section past bank 1",
    },
    Hint {
        tool: Tool::Rgblink,
        // `Section "Fixed" has type `ROMX`, which must be in bank 1 (if any)
        // with option '-t'`: a ROMX section fixed past bank 1 on a header
        // with no MBC, which the build links as one bank (`project`).
        fragment: ROMX_BANK_PAST_1,
        remedy: "the header names no MBC, so the cartridge maps 32 KB (banks 0 and 1) only and \
                 the build links it as one bank, where no section can sit in bank 2 or later: \
                 drop the section's `BANK[n]` (or make it `BANK[1]`), or name an MBC in the \
                 header (the byte at $0147, e.g. $01 for MBC1) and switch banks (write the \
                 bank number to $2000) around the code that reads it",
    },
];

/// What fixes `d`, ending with where, when its message has a hint.
pub fn remedy(d: &Diagnostic) -> Option<String> {
    let hint = HINTS
        .iter()
        .find(|h| h.tool == d.tool && d.message.contains(h.fragment))?;
    // A macro's or REPT's body is no place for the section: it belongs
    // before the line that ran it. An INCLUDE line is no place either, since
    // the included file opens its own sections: the innermost line that is
    // in no macro or REPT is, `d`'s own or the first of its chain.
    let own = match (&d.file, d.line) {
        (Some(file), Some(line)) => Some((file.as_str(), line, d.within.is_some())),
        _ => None,
    };
    let place = own
        .into_iter()
        .chain(
            d.chain
                .iter()
                .map(|at| (at.file.as_str(), at.line, at.within.is_some())),
        )
        .find(|&(_, _, in_macro)| !in_macro)
        .map(|(file, line, _)| format!(" before the first such line ({file}:{line})"))
        .unwrap_or_default();
    Some(format!("{}{place}", hint.remedy))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gbcli_protocol::Severity;

    /// Only the message it was written for, and only from rgbasm, gets the
    /// hint; anything else keeps the generic remedy.
    #[test]
    fn code_in_a_ram_section_says_to_start_a_rom_section_before_its_line() {
        let syntax = Diagnostic::new(Tool::Rgbasm, Severity::Error, "syntax error");
        assert_eq!(remedy(&syntax), None);
        let linker = Diagnostic::new(
            Tool::Rgblink,
            Severity::Error,
            "Section \"W\" cannot contain code or data (not `ROM0` or `ROMX`)",
        );
        assert_eq!(remedy(&linker), None);
        let d = Diagnostic {
            file: Some("src/main.asm".into()),
            line: Some(193),
            ..Diagnostic::new(
                Tool::Rgbasm,
                Severity::Error,
                "Section \"W\" cannot contain code or data (not `ROM0` or `ROMX`)",
            )
        };
        assert_eq!(
            remedy(&d).as_deref(),
            Some(
                "code and data go in a ROM0 or ROMX section: start one (`SECTION \"Code\", \
                 ROM0`) before the first such line (src/main.asm:193)"
            )
        );
    }

    /// A section placed in $0000-$003F of a starter project collides with
    /// its "Restart vectors" fill; rgblink names no file or line, so the hint
    /// says where that section is and how to make room in it.
    #[test]
    fn a_section_over_the_starters_restart_vectors_points_at_its_fill() {
        let d = Diagnostic::new(
            Tool::Rgblink,
            Severity::Error,
            "Unable to place \"rst08\" (ROM0 section) at address $0008: section overlaps with \
             \"Restart vectors\"",
        );
        let text = remedy(&d).unwrap();
        for needed in ["src/main.asm", "\"Restart vectors\"", "ds $40 - @, $FF"] {
            assert!(text.contains(needed), "{needed}: {text}");
        }
        // Another section's overlap is not the starter's fill.
        let other = Diagnostic::new(
            Tool::Rgblink,
            Severity::Error,
            "Unable to place \"A\" (ROM0 section) at address $0150: section overlaps with \"B\"",
        );
        assert_eq!(remedy(&other), None);
    }

    /// rgblink can't place a ROM0 section anywhere when bank 0 (or, linked
    /// as one bank, all 32 KB) is full: the hint names ROMX and an MBC.
    #[test]
    fn a_full_bank_0_names_romx_and_an_mbc() {
        let d = Diagnostic::new(
            Tool::Rgblink,
            Severity::Error,
            "Unable to place \"Sound Driver\" (ROM0 section) anywhere",
        );
        let text = remedy(&d).unwrap();
        for needed in ["ROMX", "MBC", "$0147", "32 KB"] {
            assert!(text.contains(needed), "{needed}: {text}");
        }
        let assembler = Diagnostic {
            tool: Tool::Rgbasm,
            ..d.clone()
        };
        assert_eq!(remedy(&assembler), None);
    }

    /// A ROMX section fixed past bank 1 can't link as one bank: the hint
    /// names the bank and an MBC.
    #[test]
    fn a_romx_bank_past_1_in_the_one_bank_link_names_the_bank_and_an_mbc() {
        let d = Diagnostic::new(
            Tool::Rgblink,
            Severity::Error,
            "Section \"Fixed\" has type `ROMX`, which must be in bank 1 (if any) with option '-t'",
        );
        let text = remedy(&d).unwrap();
        for needed in ["BANK[", "MBC", "$0147", "32 KB"] {
            assert!(text.contains(needed), "{needed}: {text}");
        }
    }

    fn line(file: &str, line: u32, within: Option<&str>) -> gbcli_protocol::SourceLine {
        gbcli_protocol::SourceLine {
            file: file.into(),
            line,
            within: within.map(Into::into),
        }
    }

    /// An rgbasm error with the RAM-section message at `file:line`.
    fn ram_error(
        file: &str,
        line: u32,
        within: Option<&str>,
        chain: Vec<gbcli_protocol::SourceLine>,
    ) -> Diagnostic {
        Diagnostic {
            file: Some(file.into()),
            line: Some(line),
            within: within.map(Into::into),
            chain,
            ..Diagnostic::new(
                Tool::Rgbasm,
                Severity::Error,
                "Section \"W\" cannot contain code or data (not `ROM0` or `ROMX`)",
            )
        }
    }

    /// Code a macro emits is reported at the macro body's line; the ROM
    /// section has to start before the line that invoked it.
    #[test]
    fn code_from_a_macro_names_its_invocation() {
        let d = ram_error(
            "src/main.asm",
            12,
            Some("Emit"),
            vec![line("src/main.asm", 77, None)],
        );
        let text = remedy(&d).unwrap();
        assert!(text.ends_with("(src/main.asm:77)"), "{text}");
    }

    /// A macro called from another macro's body: the invocation that is
    /// itself outside any macro, further out in the chain.
    #[test]
    fn code_from_nested_macros_names_the_outermost_invocation() {
        let d = ram_error(
            "src/main.asm",
            12,
            Some("Inner"),
            vec![
                line("src/main.asm", 20, Some("Outer")),
                line("src/main.asm", 77, None),
            ],
        );
        let text = remedy(&d).unwrap();
        assert!(text.ends_with("(src/main.asm:77)"), "{text}");
    }

    /// Code in an included file's own RAM section: the ROM section belongs
    /// in that file, so the INCLUDE line (the chain) is not the place.
    #[test]
    fn code_in_an_included_file_names_its_own_line() {
        let d = ram_error(
            "src/vars.inc",
            4,
            None,
            vec![line("src/main.asm", 312, None)],
        );
        let text = remedy(&d).unwrap();
        assert!(text.ends_with("(src/vars.inc:4)"), "{text}");
    }

    /// A macro invoked in an included file: the invocation's line there,
    /// not the INCLUDE line in the file that included it.
    #[test]
    fn code_from_a_macro_in_an_included_file_names_the_invocation_there() {
        let d = ram_error(
            "src/vars.inc",
            9,
            Some("Emit"),
            vec![
                line("src/vars.inc", 20, None),
                line("src/main.asm", 312, None),
            ],
        );
        let text = remedy(&d).unwrap();
        assert!(text.ends_with("(src/vars.inc:20)"), "{text}");
    }
}
