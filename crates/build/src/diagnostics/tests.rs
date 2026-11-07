//! The texts are RGBDS 1.0.4's own output, captured 2026-11-07.

use gbcli_protocol::{Diagnostic, Severity, Tool};

use super::parse;

fn diag(
    tool: Tool,
    severity: Severity,
    file: Option<&str>,
    line: Option<u32>,
    message: &str,
) -> Diagnostic {
    Diagnostic {
        tool,
        severity,
        file: file.map(str::to_string),
        line,
        message: message.to_string(),
    }
}

#[test]
fn errors_and_warnings_take_the_innermost_location() {
    let text = "\
error: syntax error, unexpected end of line
    at src/bad.asm(2)
warning: Expression must be 8-bit; use `LOW()` to force 8-bit [-Wtruncation]
    at src/bad.asm(5)
error: syntax error, unexpected symbol
    at src/bad.asm::m(7)
    <- src/bad.asm(9)
Assembly aborted with 2 errors
";
    assert_eq!(
        parse(Tool::Rgbasm, text),
        [
            diag(
                Tool::Rgbasm,
                Severity::Error,
                Some("src/bad.asm"),
                Some(2),
                "syntax error, unexpected end of line"
            ),
            diag(
                Tool::Rgbasm,
                Severity::Warning,
                Some("src/bad.asm"),
                Some(5),
                "Expression must be 8-bit; use `LOW()` to force 8-bit [-Wtruncation]"
            ),
            diag(
                Tool::Rgbasm,
                Severity::Error,
                Some("src/bad.asm"),
                Some(7),
                "syntax error, unexpected symbol"
            ),
        ]
    );
}

#[test]
fn a_linker_error_without_a_location_has_no_file_or_line() {
    let text = "\
error: Section \"a\" is bigger than the max size for that type: $8000 > $4000
error: Undefined symbol `Nowhere`
    at src/warn.asm(2)
Linking failed with 2 errors
";
    assert_eq!(
        parse(Tool::Rgblink, text),
        [
            diag(
                Tool::Rgblink,
                Severity::Error,
                None,
                None,
                "Section \"a\" is bigger than the max size for that type: $8000 > $4000"
            ),
            diag(
                Tool::Rgblink,
                Severity::Error,
                Some("src/warn.asm"),
                Some(2),
                "Undefined symbol `Nowhere`"
            ),
        ]
    );
}

#[test]
fn fatal_is_an_error() {
    let text = "FATAL: Failed to open file \"x.o\": No such file or directory\n\
                Linking aborted with 1 error\n";
    let found = parse(Tool::Rgblink, text);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].severity, Severity::Error);
    assert_eq!(
        found[0].message,
        "Failed to open file \"x.o\": No such file or directory"
    );
}

#[test]
fn a_location_inside_a_rept_or_with_a_path_in_parentheses_keeps_the_file() {
    let text = "error: boom\n    at src/(old)/a.asm::REPT~2(14)\n";
    let found = parse(Tool::Rgbasm, text);
    assert_eq!(found[0].file.as_deref(), Some("src/(old)/a.asm"));
    assert_eq!(found[0].line, Some(14));
}

#[test]
fn text_with_no_diagnostic_is_none() {
    assert!(parse(Tool::Rgbfix, "").is_empty());
    assert!(parse(Tool::Rgbfix, "Usage: rgbfix [-hjOsVvw]\n").is_empty());
}
