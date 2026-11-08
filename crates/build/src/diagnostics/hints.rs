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
    let place = match (&d.file, d.line) {
        (Some(file), Some(line)) => format!(" before the first such line ({file}:{line})"),
        _ => String::new(),
    };
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
}
