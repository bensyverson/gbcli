//! The texts are RGBDS 1.0.4's own output, captured 2026-11-07.

use gbcli_protocol::{Diagnostic, Severity, SourceLine, Tool};

use super::parse;

fn diag(
    tool: Tool,
    severity: Severity,
    file: Option<&str>,
    line: Option<u32>,
    message: &str,
) -> Diagnostic {
    Diagnostic {
        file: file.map(str::to_string),
        line,
        ..Diagnostic::new(tool, severity, message)
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
            Diagnostic {
                within: Some("m".into()),
                chain: vec![at("src/bad.asm", 9, None)],
                ..diag(
                    Tool::Rgbasm,
                    Severity::Error,
                    Some("src/bad.asm"),
                    Some(7),
                    "syntax error, unexpected symbol"
                )
            },
        ]
    );
}

fn at(file: &str, line: u32, within: Option<&str>) -> SourceLine {
    SourceLine {
        file: file.into(),
        line,
        within: within.map(str::to_string),
    }
}

#[test]
fn a_macro_call_an_include_and_a_rept_are_the_chain_innermost_first() {
    let text = "\
error: Undefined macro `Bad2`
    at inc/m.inc(4)
    <- src/a.asm(1)
error: Undefined symbol `Elsewhere`
    at inc/m.inc::Bad(2)
    <- src/a.asm::REPT~2(8)
    <- src/a.asm(7)
Linking failed with 2 errors
";
    let found = parse(Tool::Rgblink, text);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].file.as_deref(), Some("inc/m.inc"));
    assert_eq!(found[0].line, Some(4));
    assert_eq!(found[0].within, None);
    assert_eq!(found[0].chain, [at("src/a.asm", 1, None)]);
    assert_eq!(found[1].file.as_deref(), Some("inc/m.inc"));
    assert_eq!(found[1].line, Some(2));
    assert_eq!(found[1].within.as_deref(), Some("Bad"));
    assert_eq!(
        found[1].chain,
        [at("src/a.asm", 8, Some("REPT~2")), at("src/a.asm", 7, None)]
    );
    assert!(found.iter().all(|d| d.also.is_empty()));
}

#[test]
fn the_linkers_and_also_is_a_second_place() {
    let text = "\
FATAL: `Dup` is defined as a label, but also as another label
    at src/b.asm(2)
    and also:
    at src/a.asm(7)
Linking aborted with 1 error
";
    let found = parse(Tool::Rgblink, text);
    assert_eq!(found.len(), 1);
    let d = &found[0];
    assert_eq!(d.file.as_deref(), Some("src/b.asm"));
    assert_eq!(d.line, Some(2));
    assert!(d.chain.is_empty(), "{d:?}");
    assert_eq!(d.also, [vec![at("src/a.asm", 7, None)]]);
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

/// Code in a WRAM section: rgbasm says so once per byte it would emit, so
/// `ld a, 1` (two bytes) is reported twice. Line 6 is blank.
const CODE_IN_WRAM: &str = "\
error: Section \"W\" cannot contain code or data (not `ROM0` or `ROMX`)
    at w.asm(4)
error: Section \"W\" cannot contain code or data (not `ROM0` or `ROMX`)
    at w.asm(4)
error: Section \"W\" cannot contain code or data (not `ROM0` or `ROMX`)
    at w.asm(5)
error: Section \"W\" cannot contain code or data (not `ROM0` or `ROMX`)
    at w.asm(5)
error: Section \"W\" cannot contain code or data (not `ROM0` or `ROMX`)
    at w.asm(7)
error: Section \"W\" cannot contain code or data (not `ROM0` or `ROMX`)
    at w.asm(8)
error: Section \"W\" cannot contain code or data (not `ROM0` or `ROMX`)
    at w.asm(8)
Assembly aborted with 7 errors
";

const IN_WRAM: &str = "Section \"W\" cannot contain code or data (not `ROM0` or `ROMX`)";

fn run(first: u32, last: Option<u32>, count: u32) -> Diagnostic {
    Diagnostic {
        last_line: last,
        count,
        ..diag(
            Tool::Rgbasm,
            Severity::Error,
            Some("w.asm"),
            Some(first),
            IN_WRAM,
        )
    }
}

#[test]
fn a_message_repeated_on_consecutive_lines_is_one_diagnostic_per_run() {
    assert_eq!(
        parse(Tool::Rgbasm, CODE_IN_WRAM),
        [run(4, Some(5), 4), run(7, Some(8), 3)]
    );
}

#[test]
fn a_message_repeated_on_one_line_counts_without_a_last_line() {
    let text = "\
error: Section \"W\" cannot contain code or data (not `ROM0` or `ROMX`)
    at w.asm(4)
error: Section \"W\" cannot contain code or data (not `ROM0` or `ROMX`)
    at w.asm(4)
";
    assert_eq!(parse(Tool::Rgbasm, text), [run(4, None, 2)]);
}

#[test]
fn a_different_message_file_macro_or_chain_breaks_a_run() {
    let text = "\
error: Section \"W\" cannot contain code or data (not `ROM0` or `ROMX`)
    at w.asm(1)
error: syntax error, unexpected symbol
    at w.asm(2)
error: Section \"W\" cannot contain code or data (not `ROM0` or `ROMX`)
    at w.asm(3)
error: Section \"W\" cannot contain code or data (not `ROM0` or `ROMX`)
    at other.asm(4)
error: Section \"W\" cannot contain code or data (not `ROM0` or `ROMX`)
    at other.asm::M(5)
    <- other.asm(9)
error: Section \"W\" cannot contain code or data (not `ROM0` or `ROMX`)
    at other.asm::M(6)
    <- other.asm(10)
error: Section \"W\" cannot contain code or data (not `ROM0` or `ROMX`)
    at other.asm::M(6)
    <- other.asm(10)
";
    let found = parse(Tool::Rgbasm, text);
    let places: Vec<_> = found
        .iter()
        .map(|d| (d.file.as_deref().unwrap(), d.line.unwrap(), d.count))
        .collect();
    assert_eq!(
        places,
        [
            ("w.asm", 1, 1),
            ("w.asm", 2, 1),
            ("w.asm", 3, 1),
            ("other.asm", 4, 1),
            ("other.asm", 5, 1),
            ("other.asm", 6, 2),
        ]
    );
    assert!(found.iter().all(|d| d.last_line.is_none()), "{found:?}");
}

#[test]
fn a_line_going_back_or_a_warning_between_breaks_a_run() {
    let text = "\
error: boom
    at w.asm(5)
error: boom
    at w.asm(4)
warning: boom
    at w.asm(5)
error: boom
    at w.asm(6)
error: boom
    at w.asm(7)
";
    let found = parse(Tool::Rgbasm, text);
    let runs: Vec<_> = found
        .iter()
        .map(|d| (d.severity, d.line.unwrap(), d.last_line, d.count))
        .collect();
    assert_eq!(
        runs,
        [
            (Severity::Error, 5, None, 1),
            (Severity::Error, 4, None, 1),
            (Severity::Warning, 5, None, 1),
            (Severity::Error, 6, Some(7), 2),
        ]
    );
}

#[test]
fn identical_messages_without_a_line_collapse_too() {
    let text = "error: boom\nerror: boom\nerror: other\n";
    let found = parse(Tool::Rgblink, text);
    assert_eq!(
        found.iter().map(|d| d.count).collect::<Vec<_>>(),
        [2, 1],
        "{found:?}"
    );
    assert!(
        found
            .iter()
            .all(|d| d.line.is_none() && d.last_line.is_none())
    );
}
