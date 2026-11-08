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

/// RGBDS 1.0.4's wording.
const HINTS: &[Hint] = &[Hint {
    tool: Tool::Rgbasm,
    // `Section "W" cannot contain code or data (not `ROM0` or `ROMX`)`:
    // an instruction or `db` in a RAM section.
    fragment: "cannot contain code or data (not `ROM0` or `ROMX`)",
    remedy: "code and data go in a ROM0 or ROMX section: start one (`SECTION \"Code\", ROM0`)",
}];

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
