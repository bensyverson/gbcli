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
/// shot --help claims shape-telling only within one kind, says the SGB's
/// size, and that --screen-rows needs --json.
#[test]
fn shot_help_narrows_the_fingerprint_claim_and_names_the_sgb_size() {
    let p = Project::new("help-shot");
    let help = p.gbcli(&["shot", "--help"]).stdout;
    let flat = help.split_whitespace().collect::<Vec<_>>().join(" ");
    for want in [
        "Among screens of one kind, the rows' count and length carry the size",
        "the kind isn't hashed, so a shades crop and a colors crop can share pixels=",
        "An SGB screen is colors at 256x224, its border included",
        "256x224 on an SGB, its border included",
        "344 KB on an SGB",
        "also return the pixels as data in --json (refused without it)",
    ] {
        assert!(flat.contains(want), "{want}: {help}");
    }
}

/// help rgbds and the starter README say the same thing about where a bare
/// INCLUDE looks (the README with its code marks).
#[test]
fn help_rgbds_and_the_starter_readme_agree_on_where_include_looks() {
    let want = "INCLUDE looks in the project folder and include/ (or --include), not beside \
                the including file: src/ isn't searched.";
    let flat = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
    let p = Project::new("help-include");
    let help = flat(&p.gbcli(&["help", "rgbds"]).stdout);
    assert!(help.contains(want), "{help}");
    let readme = include_str!("../../../build/templates/README.md").replace('`', "");
    assert!(flat(&readme).contains(want), "{readme}");
}

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
