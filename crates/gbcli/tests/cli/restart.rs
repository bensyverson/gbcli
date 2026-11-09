//! A session across a rebuild: `rom_changed` names `run --from boot`, which
//! restarts the session on the ROM on disk with its breakpoints, and `new`
//! gives its project its own `.gbcli/`.

use std::fs;

use crate::common::{Project, V1, V1_SYM, V2, V2_SYM, golden};

#[test]
fn a_rebuild_is_refused_with_the_restart_and_run_from_boot_restarts() {
    let p = Project::with_session("restart");
    let mut transcript = String::new();
    let before: &[&[&str]] = &[
        &["break", "Work.done", "--session", "demo"],
        &["run", "--session", "demo", "--frames", "1"],
        &["checkpoint", "a", "--session", "demo"],
    ];
    for args in before {
        transcript += &p.gbcli(args).transcript(args, &p.dir);
    }
    fs::write(p.path("demo.gb"), V2).unwrap();
    fs::write(p.path("demo.sym"), V2_SYM).unwrap();
    let refused = &["run", "--session", "demo", "--frames", "1"];
    let run = p.gbcli(refused);
    run.fails(3, "rom_changed", "gbcli run --from boot");
    transcript += &run.transcript(refused, &p.dir);
    let after: &[&[&str]] = &[
        &[
            "run",
            "--session",
            "demo",
            "--from",
            "boot",
            "--frames",
            "10",
        ],
        &["run", "--session", "demo", "--frames", "1"],
        &["checkpoints", "--session", "demo"],
    ];
    for args in after {
        transcript += &p.gbcli(args).transcript(args, &p.dir);
    }
    golden("restart.txt", &transcript);
}

#[test]
fn new_makes_the_projects_bundle_so_its_sessions_stay_inside_it() {
    let outer = Project::with_session("nested");
    let made = outer.gbcli(&["new", "game"]);
    assert_eq!(made.code, 0, "{}", made.stderr);
    assert!(outer.path("game/.gbcli").is_dir(), "{}", made.stdout);
    assert!(
        made.stdout.contains("created path=game/.gbcli/\n"),
        "{}",
        made.stdout
    );

    // Run from inside the new project, under the outer project's bundle.
    let inner = Project {
        dir: outer.path("game"),
    };
    fs::write(inner.path("game.gb"), V1).unwrap();
    fs::write(inner.path("game.sym"), V1_SYM).unwrap();
    let run = inner.gbcli(&["run", "game.gb", "--session", "game", "--frames", "1"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(inner.path(".gbcli/sessions/game").is_dir());
    assert!(!outer.path(".gbcli/sessions/game").exists());
}
