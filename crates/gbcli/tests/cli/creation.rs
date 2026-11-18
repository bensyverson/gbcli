//! What creating a session says (leaf NQKwil): the `.gbcli/` it went into,
//! found by walking up, so an agent in a fresh checkout sees whose sessions
//! it is using; and a note when a ROM that supports color starts as a DMG
//! because no --model was named.

use std::fs;
use std::path::{Path, PathBuf};

use crate::common::Project;

/// A $80 (dual-mode) ROM.
const DUAL: &[u8] = include_bytes!("../../../engine/tests/roms/gbc_palette.gb");

/// The `session_created` rows in `stdout`.
fn created_rows(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .filter(|l| l.starts_with("session_created "))
        .collect()
}

/// The `bundle=` value of a row, canonicalized.
fn bundle_of(row: &str) -> PathBuf {
    let value = row
        .split(' ')
        .find_map(|f| f.strip_prefix("bundle="))
        .unwrap_or_else(|| panic!("no bundle= in {row}"));
    assert!(Path::new(value).is_absolute(), "{row}");
    fs::canonicalize(value).unwrap()
}

fn note_rows(stdout: &str) -> Vec<&str> {
    stdout.lines().filter(|l| l.starts_with("note ")).collect()
}

#[test]
fn creating_a_session_names_the_gbcli_folder_it_used_and_reopening_does_not() {
    let p = Project::new("created-row");
    let made = p.gbcli(&["run", "demo.gb", "--session", "a", "--frames", "1"]);
    assert_eq!(made.code, 0, "{}", made.stderr);
    let rows = created_rows(&made.stdout);
    assert_eq!(rows.len(), 1, "{}", made.stdout);
    assert!(
        rows[0].starts_with("session_created name=a model=dmg-b cgb=dmg "),
        "{}",
        rows[0]
    );
    let gbcli_dir = fs::canonicalize(p.path(".gbcli")).unwrap();
    assert_eq!(bundle_of(rows[0]), gbcli_dir);
    assert!(note_rows(&made.stdout).is_empty(), "{}", made.stdout);
    // The run's own rows still follow.
    assert!(made.stdout.contains("\nstop "), "{}", made.stdout);

    let again = p.gbcli(&["run", "demo.gb", "--session", "a", "--frames", "1"]);
    assert_eq!(again.code, 0, "{}", again.stderr);
    assert!(created_rows(&again.stdout).is_empty(), "{}", again.stdout);
}

/// The F-3 case: a subdirectory with no `.gbcli/` of its own uses the one
/// above it, and the row says so.
#[test]
fn a_session_made_below_a_gbcli_folder_names_the_one_above() {
    let p = Project::with_session("created-walk-up");
    let game = p.path("examples/game");
    fs::create_dir_all(&game).unwrap();
    fs::write(game.join("game.gb"), crate::common::V1).unwrap();
    let made = p.gbcli_in(&game, &["show", "--rom", "game.gb", "--session", "g"], &[]);
    assert_eq!(made.code, 0, "{}", made.stderr);
    let rows = created_rows(&made.stdout);
    assert_eq!(rows.len(), 1, "{}", made.stdout);
    assert_eq!(
        bundle_of(rows[0]),
        fs::canonicalize(p.path(".gbcli")).unwrap()
    );
    assert!(!game.join(".gbcli").exists());
}

#[test]
fn json_carries_the_created_session() {
    let p = Project::new("created-json");
    let made = p.gbcli(&["show", "--rom", "demo.gb", "--session", "a", "--json"]);
    assert_eq!(made.code, 0, "{}", made.stderr);
    let line: serde_json::Value = serde_json::from_str(made.stdout.trim()).unwrap();
    let created = &line["session_created"];
    assert_eq!(created["name"], "a", "{line}");
    assert_eq!(created["model"], "dmg-b", "{line}");
    assert_eq!(created["cgb"], "dmg", "{line}");
    let bundle = created["bundle"].as_str().unwrap();
    assert_eq!(
        fs::canonicalize(bundle).unwrap(),
        fs::canonicalize(p.path(".gbcli")).unwrap()
    );
    assert!(created.get("note").is_none(), "{line}");
}

#[test]
fn a_dual_mode_rom_without_model_is_noted_and_with_model_gbc_is_not() {
    let p = Project::new("created-note");
    fs::write(p.path("gbc.gb"), DUAL).unwrap();
    let bare = p.gbcli(&["show", "--rom", "gbc.gb", "--session", "mono"]);
    assert_eq!(bare.code, 0, "{}", bare.stderr);
    let notes = note_rows(&bare.stdout);
    assert_eq!(notes.len(), 1, "{}", bare.stdout);
    assert!(
        notes[0].starts_with("note color_rom_on_dmg session=mono cgb=dual model=dmg-b "),
        "{}",
        notes[0]
    );
    assert!(notes[0].contains("--model gbc"), "{}", notes[0]);
    let json = p.gbcli(&["show", "--rom", "gbc.gb", "--session", "mono2", "--json"]);
    let line: serde_json::Value = serde_json::from_str(json.stdout.trim()).unwrap();
    assert_eq!(
        line["session_created"]["note"], "color_rom_on_dmg",
        "{line}"
    );

    let color = p.gbcli(&[
        "show",
        "--rom",
        "gbc.gb",
        "--session",
        "color",
        "--model",
        "gbc",
    ]);
    assert_eq!(color.code, 0, "{}", color.stderr);
    assert_eq!(created_rows(&color.stdout).len(), 1, "{}", color.stdout);
    assert!(note_rows(&color.stdout).is_empty(), "{}", color.stdout);

    let dmg = p.gbcli(&["show", "--rom", "demo.gb", "--session", "dmg"]);
    assert_eq!(dmg.code, 0, "{}", dmg.stderr);
    assert!(note_rows(&dmg.stdout).is_empty(), "{}", dmg.stdout);

    // Reopened, the session is what it is: no note again.
    let again = p.gbcli(&["show", "--rom", "gbc.gb", "--session", "mono"]);
    assert!(note_rows(&again.stdout).is_empty(), "{}", again.stdout);
}

/// A batch whose first line doesn't parse still reports the creation first,
/// before that line's row, since the creation came before every line.
#[test]
fn batch_reports_the_creation_before_a_first_line_that_does_not_parse() {
    let p = Project::new("created-batch");
    let args = ["batch", "-F", "-", "--session", "b", "--rom", "demo.gb"];
    let input = "not json\n{\"id\":\"2\",\"op\":\"show\"}\n";
    let made = p.gbcli_with(&args, &[], Some(input));
    let lines: Vec<&str> = made.stdout.lines().collect();
    assert!(
        lines[0].starts_with("session_created name=b "),
        "{}",
        made.stdout
    );
    assert!(lines[1].starts_with("line n=1 ok=0 "), "{}", made.stdout);
    assert_eq!(created_rows(&made.stdout).len(), 1, "{}", made.stdout);

    let p = Project::new("created-batch-json");
    let args = [
        "batch",
        "-F",
        "-",
        "--session",
        "b",
        "--rom",
        "demo.gb",
        "--json",
    ];
    let made = p.gbcli_with(&args, &[], Some(input));
    let first: serde_json::Value =
        serde_json::from_str(made.stdout.lines().next().unwrap()).unwrap();
    assert_eq!(first["session_created"]["name"], "b", "{}", made.stdout);

    let schema = p.gbcli(&["schema"]);
    let flat = schema
        .stdout
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        flat.contains(
            "whatever that first line was, a refused one included; with no request lines \
             there is no response, so it isn't reported on stdout"
        ),
        "{}",
        schema.stdout
    );
}

/// Where a session's state goes, and the default model, are said where an
/// agent looks first: the primer and `run --help`.
#[test]
fn help_says_how_gbcli_is_found_and_that_a_new_session_is_a_dmg() {
    let p = Project::new("created-help");
    let help = p.gbcli(&["help"]);
    let flat = help.stdout.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flat.contains("It uses the nearest .gbcli/ at or above the working directory (or --dir)"),
        "{}",
        help.stdout
    );
    let run = p.gbcli(&["run", "--help"]);
    let flat = run.stdout.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flat.contains("A new session is a DMG (dmg-b) unless --model says otherwise"),
        "{}",
        run.stdout
    );
}
