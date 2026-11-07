//! Every request verb, in rows and in `--json`, as one golden transcript per
//! form. The same script runs in two projects, so the states match step for
//! step and each verb is seen both ways.

use crate::common::{Project, golden};

/// Each step's arguments; `--session demo` and the form's flag are added.
const SCRIPT: &[&[&str]] = &[
    &["run", "demo.gb", "--frames", "2", "--read", "u16[wFrame]"],
    &["step"],
    &["step", "frame"],
    &["show"],
    &["read", "u16[wFrame]", "pc", "frame"],
    &["break", "Work"],
    &["watch", "write", "wFrame", "--once"],
    &["run"],
    &["delete", "1"],
    &["run"],
    &["checkpoint", "before"],
    &["poke", "wSpeed=2"],
    &["write", "wSpeed=3,4"],
    &["patch", "00:0200=0"],
    &["restore", "before"],
    &["undo"],
    &[
        "run",
        "--until",
        "u16[wFrame] >= 2500",
        "--read",
        "u16[wFrame]",
    ],
    &[
        "run",
        "--frames",
        "1",
        "--keys",
        "A@0 -A@1 START@5",
        "--shot",
        "run.png",
        "--scale",
        "1",
    ],
    &["shot", "shot.png", "--scale", "1", "--crop", "0,0,16,8"],
    &["shot", "live.png", "--live", "--scale", "1"],
    &["view", "tiles"],
    &["view", "oam", "--png", "oam.png", "--scale", "1"],
    &["view", "map", "--map", "window"],
    &["sym", "Work*"],
    &["checkpoints"],
    &["events", "--limit", "2"],
    &["read", "[$FEA0]"],
];

/// Verbs that name no session.
const PROJECT_SCRIPT: &[&[&str]] = &[
    &["gaps"],
    &["sessions"],
    &["sessions", "prune", "--unused-for", "7d"],
    &["sessions", "rm", "demo"],
    &["sessions", "rm", "demo", "--with-checkpoints"],
    &["sessions"],
];

fn transcript(name: &str, form: &[&str]) -> String {
    let project = Project::new(name);
    let mut out = String::new();
    for step in SCRIPT {
        let mut args: Vec<&str> = step.to_vec();
        args.extend(["--session", "demo"]);
        args.extend(form);
        out += &project.gbcli(&args).transcript(&args, &project.dir);
    }
    for step in PROJECT_SCRIPT {
        let mut args: Vec<&str> = step.to_vec();
        args.extend(form);
        out += &project.gbcli(&args).transcript(&args, &project.dir);
    }
    out
}

#[test]
fn every_verb_prints_rows() {
    golden("verbs.rows.txt", &transcript("rows", &[]));
}

#[test]
fn every_verb_prints_the_serve_response_with_json() {
    golden("verbs.json.txt", &transcript("json", &["--json"]));
}

const OPS: &str = r#"{"id":"a","op":"run","args":{"frames":1}}

{"id":"b","op":"read","args":{"reads":["u16[wFrame]"]}}
{"id":"c","op":"read","args":{"reads":["[wNope]"]}}
not json
{"id":"d","op":"checkpoint","args":{"name":"one"}}
"#;

fn batch(name: &str, form: &[&str]) -> String {
    let project = Project::new(name);
    std::fs::write(project.path("ops.jsonl"), OPS).unwrap();
    let mut args = vec![
        "batch",
        "-F",
        "ops.jsonl",
        "--session",
        "b",
        "--rom",
        "demo.gb",
    ];
    args.extend(form);
    let mut out = project.gbcli(&args).transcript(&args, &project.dir);
    let args = ["batch", "-F", "-", "--session", "b"];
    let run = project.gbcli_with(&args, &[], Some(r#"{"id":"e","op":"show"}"#));
    out += &run.transcript(&args, &project.dir);
    out
}

#[test]
fn batch_prints_one_row_per_line_and_exits_with_the_first_failure() {
    golden("batch.rows.txt", &batch("batch-rows", &[]));
}

#[test]
fn batch_with_json_prints_each_response_line() {
    golden("batch.json.txt", &batch("batch-json", &["--json"]));
}

#[test]
fn json_prints_the_serve_response_line_rev_included() {
    let setup = ["run", "demo.gb", "--session", "demo", "--frames", "1"];
    let cli = Project::new("json-line-cli");
    let served = Project::new("json-line-serve");
    for p in [&cli, &served] {
        assert_eq!(p.gbcli(&setup).code, 0);
    }
    for (args, line) in [
        (
            &["read", "u16[wFrame]", "--session", "demo", "--json"][..],
            r#"{"id":"cli","session":"demo","op":"read","args":{"reads":["u16[wFrame]"]}}"#,
        ),
        (
            &["read", "[wNope]", "--session", "demo", "--json"][..],
            r#"{"id":"cli","session":"demo","op":"read","args":{"reads":["[wNope]"]}}"#,
        ),
    ] {
        let mine = cli.gbcli(args);
        let theirs = served.gbcli_with(&["serve"], &[], Some(&format!("{line}\n")));
        assert_eq!(mine.stdout, theirs.stdout, "{args:?}");
        assert!(mine.stdout.contains(r#""rev":"s1""#), "{}", mine.stdout);
    }
}

#[test]
fn the_environment_names_the_session_and_actor() {
    let project = Project::new("env");
    let env = [("GBCLI_SESSION", "envy"), ("GBCLI_AS", "agent-7")];
    let made = project.gbcli_with(&["show", "--rom", "demo.gb"], &env, None);
    assert_eq!(made.code, 0, "{}", made.stderr);
    let events = project.gbcli_with(&["events"], &env, None);
    assert!(
        events.stdout.contains(" as=agent-7 op=show "),
        "{}",
        events.stdout
    );
    // GBCLI_DIR moves the bundle; the project directory is its parent.
    let state = project.path("state");
    std::fs::create_dir_all(&state).unwrap();
    let state_text = state.display().to_string();
    let elsewhere = [("GBCLI_DIR", state_text.as_str())];
    let made = project.gbcli_with(
        &["show", "--session", "s", "--rom", "demo.gb"],
        &elsewhere,
        None,
    );
    assert_eq!(made.code, 0, "{}", made.stderr);
    assert!(state.join("sessions/s/session.json").is_file());
}
