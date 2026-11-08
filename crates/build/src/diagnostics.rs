//! RGBDS 1.0's diagnostics, as structured errors. Each one is a line
//! `error: MESSAGE`, `warning: MESSAGE [-Wflag]` or `FATAL: MESSAGE`, then
//! optionally `    at FILE(LINE)` (or `FILE::MACRO(LINE)`, the line in FILE
//! where the macro's body is) and `    <- FILE(LINE)` lines for the include
//! and macro chain, outermost last. rgblink may name a second place after
//! `    and also:`, with its own `at` and `<-` lines. Summary lines
//! (`Assembly aborted with 2 errors`) carry nothing new and are dropped.

use gbcli_protocol::{Diagnostic, Severity, SourceLine, Tool};

/// Every diagnostic in `text`, a tool's stderr, with files as the tool
/// printed them.
pub fn parse(tool: Tool, text: &str) -> Vec<Diagnostic> {
    let mut found: Vec<Diagnostic> = Vec::new();
    // Which place of the last diagnostic the next location lines belong to.
    let mut place = Place::Done;
    for line in text.lines() {
        if let Some((severity, message)) = head(line) {
            found.push(Diagnostic::new(tool, severity, message.trim()));
            place = Place::Main { located: false };
            continue;
        }
        let Some(last) = found.last_mut() else {
            continue;
        };
        if !line.starts_with(char::is_whitespace) {
            place = Place::Done;
            continue;
        }
        let body = line.trim();
        if body == "and also:" && place != Place::Done {
            last.also.push(Vec::new());
            place = Place::Also { located: false };
            continue;
        }
        let (first, rest) = match (body.strip_prefix("at "), body.strip_prefix("<- ")) {
            (Some(at), _) => (true, at),
            (_, Some(from)) => (false, from),
            _ => continue,
        };
        let Some(at) = location(rest) else {
            continue;
        };
        match &mut place {
            Place::Main { located } if first && !*located => {
                last.file = Some(at.file);
                last.line = Some(at.line);
                last.within = at.within;
                *located = true;
            }
            Place::Main { located: true } if !first => last.chain.push(at),
            Place::Also { located } if first != *located => {
                last.also
                    .last_mut()
                    .expect("pushed at `and also:`")
                    .push(at);
                *located = true;
            }
            _ => {}
        }
    }
    collapse(found)
}

/// One diagnostic for each run of the same message on the same or the next
/// line, back to back in the tool's output. rgbasm reports some errors once
/// per byte (code in a RAM section), which would bury the one fix under a
/// row per byte. A gap of a line, or anything printed between, starts a new
/// run, so two places that each need fixing stay two.
fn collapse(found: Vec<Diagnostic>) -> Vec<Diagnostic> {
    let mut runs: Vec<Diagnostic> = Vec::with_capacity(found.len());
    for d in found {
        if let Some(run) = runs.last_mut()
            && continues(run, &d)
        {
            run.count += d.count;
            if d.line > run.line {
                run.last_line = d.line;
            }
            continue;
        }
        runs.push(d);
    }
    runs
}

/// Whether `next` says what `run` says, one line on at most.
fn continues(run: &Diagnostic, next: &Diagnostic) -> bool {
    let same = run.tool == next.tool
        && run.severity == next.severity
        && run.file == next.file
        && run.within == next.within
        && run.message == next.message
        && run.chain == next.chain
        && run.also == next.also;
    let last = run.last_line.or(run.line);
    same && (next.line == last || matches!((last, next.line), (Some(l), Some(n)) if n == l + 1))
}

/// Where the location lines that follow belong.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    /// The diagnostic's own place: `at`, then its `<-` chain.
    Main { located: bool },
    /// The latest `and also:` place.
    Also { located: bool },
    /// Nothing more belongs to the last diagnostic.
    Done,
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

/// `FILE(LINE)`, `FILE::MACRO(LINE)` or `FILE::REPT~N(LINE)`. The last
/// parenthesis is the line's, so a file name may hold parentheses of its
/// own.
fn location(text: &str) -> Option<SourceLine> {
    let text = text.trim_end().strip_suffix(')')?;
    let open = text.rfind('(')?;
    let line = text[open + 1..].parse().ok()?;
    let place = &text[..open];
    let (file, within) = match place.split_once("::") {
        Some((file, within)) => (file, Some(within.to_string())),
        None => (place, None),
    };
    Some(SourceLine {
        file: file.to_string(),
        line,
        within,
    })
}

pub(crate) mod hints;
#[cfg(test)]
mod tests;
