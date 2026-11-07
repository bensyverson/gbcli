//! Every command and flag a message names exists (cli-design ▸ Errors that
//! teach): a remedy that names `gbcli doctor` or `--boot-rom` sends the reader
//! somewhere that isn't there. The texts are the string literals of every
//! crate that writes messages, outside comments and tests, so a new remedy is
//! checked without being listed here.

use std::fs;
use std::path::{Path, PathBuf};

use super::{VERBS, by_name};

/// The crates whose strings reach a user: errors, remedies, help.
const CRATES: &[&str] = &[
    "protocol", "bundle", "engine", "server", "cli", "view", "audio", "build",
];

/// Text a user reads whole, not as string literals: the files `gbcli new`
/// writes (crates/build/templates).
const TEMPLATES: &str = "build/templates";

/// Flags every verb takes, or that come before a verb.
const ALWAYS: &[&str] = &["help", "version"];

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if path.is_dir() {
            if name != "tests" {
                sources(&path, out);
            }
        } else if name.ends_with(".rs") && name != "tests.rs" && !name.ends_with("_tests.rs") {
            out.push(path);
        }
    }
}

/// The file's code before its test module, without comment lines.
fn code(path: &Path) -> String {
    let text = fs::read_to_string(path).unwrap();
    text.lines()
        .take_while(|l| l.trim() != "#[cfg(test)]")
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The words after `--` in `text`: `--frames`, not `--` or `--{flag}`.
fn flags(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("--") {
        rest = &rest[i + 2..];
        let word: String = rest
            .chars()
            .take_while(|c| c.is_ascii_lowercase() || *c == '-')
            .collect();
        if word.starts_with(|c: char| c.is_ascii_lowercase()) {
            found.push(word.trim_end_matches('-').to_string());
        }
    }
    found
}

/// Every backticked `gbcli …` command in `text`.
fn commands(text: &str) -> Vec<String> {
    text.split('`')
        .skip(1)
        .step_by(2)
        .filter(|span| span.starts_with("gbcli "))
        .map(str::to_string)
        .collect()
}

/// What's wrong with one named command, if anything.
fn check_command(command: &str) -> Option<String> {
    let words: Vec<&str> = command.split_whitespace().skip(1).collect();
    let first = *words.first()?;
    // A placeholder (`gbcli VERB --help`, `gbcli {} --help`): its flags are
    // checked against every verb's with the rest of the text.
    let placeholder = first.starts_with(['<', '{']) || first.chars().all(|c| c.is_uppercase());
    if placeholder {
        return None;
    }
    if first.starts_with('-') {
        return (!ALWAYS.contains(&first.trim_start_matches('-')))
            .then(|| format!("`{command}`: no such global flag"));
    }
    let spec = match words.get(1) {
        Some(sub) if first == "sessions" && by_name(&format!("sessions {sub}")).is_some() => {
            by_name(&format!("sessions {sub}"))
        }
        _ => by_name(first),
    };
    let Some(spec) = spec else {
        return Some(format!("`{command}`: no verb `{first}`"));
    };
    flags(command)
        .into_iter()
        .find(|f| spec.flag(f).is_none() && !ALWAYS.contains(&f.as_str()))
        .map(|f| format!("`{command}`: `gbcli {}` has no --{f}", spec.name))
}

#[test]
fn every_command_and_flag_a_message_names_exists() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for krate in CRATES {
        sources(&root.join(krate).join("src"), &mut files);
    }
    assert!(files.len() > 50, "found only {} sources", files.len());
    let templates: Vec<PathBuf> = fs::read_dir(root.join(TEMPLATES))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert!(templates.len() >= 4, "{templates:?}");
    let known = |f: &str| ALWAYS.contains(&f) || VERBS.iter().any(|s| s.flag(f).is_some());
    let mut wrong = Vec::new();
    for file in files.iter().chain(&templates) {
        let text = if templates.contains(file) {
            fs::read_to_string(file).unwrap()
        } else {
            code(file)
        };
        let at = file.strip_prefix(&root).unwrap().display();
        for command in commands(&text) {
            if let Some(problem) = check_command(&command) {
                wrong.push(format!("{at}: {problem}"));
            }
        }
        for flag in flags(&text) {
            if !known(&flag) {
                wrong.push(format!("{at}: --{flag} is no verb's flag"));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_checker_catches_a_missing_verb_and_a_missing_flag() {
    assert!(check_command("gbcli doctor --core").is_some());
    assert!(check_command("gbcli run --boot-rom x").is_some());
    assert!(check_command("gbcli run --frames 2").is_none());
    assert!(check_command("gbcli sessions rm demo --with-checkpoints").is_none());
    assert_eq!(
        flags("pass --boot-rom <file>, not -- or --{x}"),
        ["boot-rom"]
    );
}
