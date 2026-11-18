//! Help needs nothing: no session, no ROM, no `.gbcli/`.

use crate::common::{Project, golden};

#[test]
fn help_works_with_no_preconditions_and_leaves_no_trace() {
    let p = Project::new("help");
    std::fs::remove_file(p.path("demo.gb")).unwrap();
    // One golden per command, so parallel work on different help texts
    // doesn't collide in one file.
    for args in [
        &[][..],
        &["run", "--help"],
        &["power-cycle", "--help"],
        &["sessions", "rm", "-h"],
        &["help", "watch"],
        &["help", "time"],
        &["help", "rgbds"],
        &["help", "images"],
        &["build", "--help"],
        &["assets", "--help"],
        &["version", "--help"],
        &["help", "fonts"],
        &["font", "--help"],
    ] {
        let run = p.gbcli(args);
        assert_eq!(run.code, 0, "{args:?}: {}", run.stderr);
        golden(&help_golden(args), &run.transcript(args, &p.dir));
    }
    for args in [
        &["help", "addresses"][..],
        &["help", "expressions"],
        &["help", "checkpointing"],
        &["help", "writes"],
        &["help", "budgets"],
        &["help", "exit-codes"],
        &["help", "hardware"],
        &["help", "optimize"],
        &["help", "music"],
        &["recipes"],
        &["schema"],
        &["help", "schema"],
        &["--help"],
    ] {
        let run = p.gbcli(args);
        assert_eq!(run.code, 0, "{args:?}: {}", run.stderr);
        assert!(run.stdout.lines().count() >= 5, "{args:?}");
        assert!(run.stderr.is_empty(), "{args:?}: {}", run.stderr);
    }
    assert!(!p.path(".gbcli").exists(), "help made a bundle");
}

#[test]
fn the_primer_is_at_most_40_lines() {
    let run = Project::new("primer").gbcli(&[]);
    assert_eq!(run.code, 0);
    assert!(run.stdout.lines().count() <= 40);
    assert!(
        run.stdout
            .contains("gbcli run game.gb --session demo --frames 120 --shot first.png")
    );
}

#[test]
fn version_names_the_core() {
    let run = Project::new("version").gbcli(&["version"]);
    assert_eq!(run.code, 0);
    assert!(run.stdout.starts_with("gbcli 0."), "{}", run.stdout);
    assert!(run.stdout.contains("(SameBoy "), "{}", run.stdout);
    let flag = Project::new("version-flag").gbcli(&["--version"]);
    assert_eq!(flag.stdout, run.stdout);
}

/// `version` is the `version` op: `--json` is the response line `serve`
/// writes, and the text line is drawn from the same answer, unchanged for
/// the scripts that parse `gbcli --version`.
#[test]
fn version_json_is_the_ops_answer_and_the_text_is_drawn_from_it() {
    let p = Project::new("version-json");
    let json = p.gbcli(&["version", "--json"]);
    assert_eq!(json.code, 0, "{}", json.stderr);
    let response: gbcli_protocol::Response = serde_json::from_str(&json.stdout).unwrap();
    let v = response.result().unwrap().version.clone().expect("version");
    assert_eq!(v.protocol, gbcli_protocol::PROTOCOL_VERSION);
    let text = p.gbcli(&["--version"]);
    assert_eq!(
        text.stdout,
        format!(
            "gbcli {} (SameBoy {}+patches.{})\n",
            v.gbcli,
            &v.sameboy[..7],
            &v.patch_set[..8]
        )
    );
    assert!(!p.path(".gbcli").exists(), "version made a bundle");
}

/// `help/<the args, dashes dropped, joined by _>.txt`; the bare primer is
/// `help/primer.txt`.
fn help_golden(args: &[&str]) -> String {
    let name = args
        .iter()
        .map(|a| a.trim_start_matches('-'))
        .collect::<Vec<_>>()
        .join("_");
    let name = if name.is_empty() {
        "primer".into()
    } else {
        name
    };
    format!("help/{name}.txt")
}
