//! Help needs nothing: no session, no ROM, no `.gbcli/`.

use crate::common::{Project, golden};

#[test]
fn help_works_with_no_preconditions_and_leaves_no_trace() {
    let p = Project::new("help");
    std::fs::remove_file(p.path("demo.gb")).unwrap();
    let mut out = String::new();
    for args in [
        &[][..],
        &["run", "--help"],
        &["sessions", "rm", "-h"],
        &["help", "watch"],
        &["help", "time"],
        &["help", "rgbds"],
        &["build", "--help"],
    ] {
        let run = p.gbcli(args);
        assert_eq!(run.code, 0, "{args:?}: {}", run.stderr);
        out += &run.transcript(args, &p.dir);
    }
    golden("help.txt", &out);
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
