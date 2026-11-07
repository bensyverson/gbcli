//! RGBDS 1.0's diagnostics, as structured errors. Each one is a line
//! `error: MESSAGE`, `warning: MESSAGE [-Wflag]` or `FATAL: MESSAGE`, then
//! optionally `    at FILE(LINE)` (or `FILE::MACRO(LINE)`, the line in FILE
//! where the macro's body is) and `    <- FILE(LINE)` lines for the include
//! and macro chain, outermost last. Summary lines (`Assembly aborted with 2
//! errors`) carry nothing new and are dropped.

use gbcli_protocol::{Diagnostic, Severity, Tool};

/// Every diagnostic in `text`, a tool's stderr, with files as the tool
/// printed them.
pub fn parse(tool: Tool, text: &str) -> Vec<Diagnostic> {
    let mut found: Vec<Diagnostic> = Vec::new();
    // Whether the last diagnostic has taken its `at` line: only the first
    // (innermost) one locates it.
    let mut located = true;
    for line in text.lines() {
        if let Some((severity, message)) = head(line) {
            found.push(Diagnostic {
                tool,
                severity,
                file: None,
                line: None,
                message: message.trim().to_string(),
            });
            located = false;
            continue;
        }
        let Some(at) = line.trim_start().strip_prefix("at ") else {
            continue;
        };
        if located || !line.starts_with(char::is_whitespace) {
            continue;
        }
        if let (Some(last), Some((file, number))) = (found.last_mut(), location(at)) {
            last.file = Some(file);
            last.line = Some(number);
            located = true;
        }
    }
    found
}

fn head(line: &str) -> Option<(Severity, &str)> {
    for (prefix, severity) in [
        ("error: ", Severity::Error),
        ("FATAL: ", Severity::Error),
        ("warning: ", Severity::Warning),
    ] {
        if let Some(message) = line.strip_prefix(prefix) {
            return Some((severity, message));
        }
    }
    None
}

/// `FILE(LINE)`, `FILE::MACRO(LINE)` or `FILE::REPT~N(LINE)` → the file and
/// the line. The last parenthesis is the line's, so a file name may hold
/// parentheses of its own.
fn location(text: &str) -> Option<(String, u32)> {
    let text = text.trim_end().strip_suffix(')')?;
    let open = text.rfind('(')?;
    let number = text[open + 1..].parse().ok()?;
    let place = &text[..open];
    let file = place.split("::").next().unwrap_or(place);
    Some((file.to_string(), number))
}

#[cfg(test)]
mod tests;
