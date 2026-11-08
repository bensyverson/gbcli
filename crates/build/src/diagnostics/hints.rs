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
    // The ROM section belongs before the line that invoked the macro (the
    // chain's last entry), not inside its body, where `d.line` points.
    let place = match (d.chain.last(), &d.file, d.line) {
        (Some(outer), _, _) => Some((outer.file.as_str(), outer.line)),
        (None, Some(file), Some(line)) => Some((file.as_str(), line)),
        _ => None,
    }
    .map(|(file, line)| format!(" before the first such line ({file}:{line})"))
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

    /// Code a macro emits is reported at the macro body's line; the ROM
    /// section has to start before the line that invoked it, the last (outermost)
    /// chain entry.
    #[test]
    fn code_from_a_macro_names_the_outermost_invocation() {
        let line = |file: &str, line| gbcli_protocol::SourceLine {
            file: file.into(),
            line,
            within: None,
        };
        let d = Diagnostic {
            file: Some("src/main.asm".into()),
            line: Some(12),
            within: Some("Emit".into()),
            chain: vec![line("src/inner.asm", 40), line("src/main.asm", 77)],
            ..Diagnostic::new(
                Tool::Rgbasm,
                Severity::Error,
                "Section \"W\" cannot contain code or data (not `ROM0` or `ROMX`)",
            )
        };
        let text = remedy(&d).unwrap();
        assert!(text.ends_with("(src/main.asm:77)"), "{text}");
    }
}
