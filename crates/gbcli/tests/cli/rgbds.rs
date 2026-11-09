//! The RGBDS loop through the binary: `new`, `build` (and `build --restore`)
//! and `doctor`, with the pinned RGBDS from `PATH`. A machine without it
//! fails these tests naming the install command; none skips.

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::common::{Project, golden};

/// The directory holding the pinned rgbasm (and, by the install, rgblink and
/// rgbfix): the only `PATH` the loop is given, so nothing else on this
/// machine can be what makes it work.
pub(crate) fn rgbds_dir() -> String {
    let found = std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|dir| dir.join("rgbasm"))
            .find(|p| p.is_file())
    });
    let Some(rgbasm) = found else {
        panic!(
            "these tests need RGBDS 1.0.4 on PATH: install it with `brew install rgbds`, or \
             the release at https://github.com/gbdev/rgbds/releases/tag/v1.0.4"
        );
    };
    let version = Command::new(&rgbasm).arg("--version").output().unwrap();
    let version = String::from_utf8_lossy(&version.stdout);
    assert!(
        version.trim() == "rgbasm v1.0.4",
        "these tests need RGBDS 1.0.4; {} is {version}",
        rgbasm.display()
    );
    rgbasm.parent().unwrap().display().to_string()
}

impl Project {
    /// `gbcli args…` with only the pinned RGBDS on `PATH`.
    pub(crate) fn rgbds(&self, args: &[&str]) -> crate::common::Run {
        self.gbcli_with(args, &[("PATH", &rgbds_dir())], None)
    }

    /// `gbcli new game` then `gbcli build game`, both asserted to pass.
    fn game(name: &str) -> Project {
        let p = Project::new(name);
        let made = p.rgbds(&["new", "game"]);
        assert_eq!(made.code, 0, "{}", made.stderr);
        let built = p.rgbds(&["build", "game"]);
        assert_eq!(built.code, 0, "{}{}", built.stdout, built.stderr);
        p
    }
}

/// The number of distinct colors in a PNG and the color of pixel (x, y),
/// as 8-bit samples (the screenshot may be gray or indexed).
fn colors(path: &Path, x: usize, y: usize) -> (usize, Vec<u8>) {
    let mut decoder = png::Decoder::new(fs::File::open(path).unwrap());
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    let channels = info.color_type.samples();
    let pixel = |i: usize| buf[i * channels..(i + 1) * channels].to_vec();
    let mut seen: Vec<Vec<u8>> = (0..(info.width * info.height) as usize)
        .map(pixel)
        .collect();
    seen.sort();
    seen.dedup();
    (seen.len(), pixel(y * info.width as usize + x))
}

#[test]
fn new_then_build_then_a_screenshot_works_with_only_the_pinned_rgbds() {
    let p = Project::new("rgbds-first-try");
    let made = p.rgbds(&["new", "game"]);
    assert_eq!(made.code, 0, "{}", made.stderr);
    assert!(
        made.stdout.contains("created path=game/src/main.asm\n"),
        "{}",
        made.stdout
    );

    // The commands `new` said to run next, as it said them, from where it ran.
    let next: Vec<&str> = made
        .stdout
        .lines()
        .filter_map(|l| l.strip_prefix("next command=\""))
        .map(|l| l.trim_end_matches('"'))
        .collect();
    assert_eq!(next.len(), 3, "{}", made.stdout);
    let mut cwd = p.path("");
    let mut runs = Vec::new();
    for command in &next {
        let words: Vec<&str> = command.split(' ').collect();
        match words[..] {
            ["cd", dir] => cwd = cwd.join(dir),
            ["gbcli", ..] => {
                let run = p.gbcli_in(&cwd, &words[1..], &[("PATH", &rgbds_dir())]);
                assert_eq!(run.code, 0, "{command}: {}{}", run.stdout, run.stderr);
                assert!(run.stderr.is_empty(), "{command}: {}", run.stderr);
                runs.push(run);
            }
            _ => panic!("not a command this test follows: {command}"),
        }
    }
    let [built, shot] = &runs[..] else {
        panic!("{next:?}")
    };
    assert!(
        built
            .stdout
            .contains("output kind=rom path=build/game.gb bytes=32768\n"),
        "{}",
        built.stdout
    );
    assert!(
        shot.stdout.contains("vblank normal"),
        "a normal frame: {}",
        shot.stdout
    );
    // The session is in the project's own bundle, and no other was made.
    assert!(p.path("game/.gbcli/sessions/game").is_dir());
    assert!(
        !p.path(".gbcli").exists(),
        "a second bundle beside the project"
    );
    // The row of faces across the middle (tile row 8, from x = 32) is on
    // screen at 4x: the face's outline is dark at its top-middle.
    let first = p.path("game/first.png");
    let (count, top_of_face) = colors(&first, (32 + 3) * 4, 8 * 8 * 4);
    assert!(count >= 2, "the screen isn't blank: {count} color(s)");
    let (_, background) = colors(&first, 0, 0);
    assert_ne!(top_of_face, background);

    // The loop the help gives, from inside game/, finds that session
    // (review-run14's probe: the old lines put it in the parent's bundle).
    let saved = p.gbcli_in(
        &cwd,
        &["checkpoint", "start", "--session", "game"],
        &[("PATH", &rgbds_dir())],
    );
    assert_eq!(saved.code, 0, "{}", saved.stderr);
    let restored = p.gbcli_in(
        &cwd,
        &["build", "--session", "game", "--restore", "start"],
        &[("PATH", &rgbds_dir())],
    );
    assert_eq!(restored.code, 0, "{}{}", restored.stdout, restored.stderr);
}

#[test]
fn a_syntax_error_comes_back_with_its_file_and_line() {
    let p = Project::game("rgbds-syntax");
    let main = p.path("game/src/main.asm");
    let text = fs::read_to_string(&main).unwrap();
    let broken = text.replacen("\tld sp, $E000", "\tld sp,", 1);
    fs::write(&main, &broken).unwrap();
    let line = broken
        .lines()
        .position(|l| l.starts_with("\tld sp, "))
        .unwrap()
        + 1;

    let rows = p.rgbds(&["build", "game"]);
    rows.fails(
        4,
        "build_failed",
        "fix the first error at its file and line",
    );
    let expected = format!(
        "diagnostic severity=error tool=rgbasm file=game/src/main.asm line={line} \
         message=\"syntax error, unexpected end of line\"\n"
    );
    assert!(rows.stdout.starts_with(&expected), "{}", rows.stdout);
    assert!(
        rows.stderr
            .contains(&format!("game/src/main.asm:{line}: syntax error")),
        "{}",
        rows.stderr
    );

    let json = p.rgbds(&["build", "game", "--json"]);
    assert_eq!(json.code, 4);
    let line_json: serde_json::Value = serde_json::from_str(json.stdout.trim()).unwrap();
    assert_eq!(line_json["ok"], false);
    assert_eq!(line_json["error"]["code"], "build_failed");
    let first = &line_json["error"]["result"]["build"]["diagnostics"][0];
    assert_eq!(first["file"], "game/src/main.asm");
    assert_eq!(first["line"], line);
    assert_eq!(first["tool"], "rgbasm");
    assert_eq!(first["severity"], "error");
}

/// The eval's stumble (leaf k22J86): code after a WRAM section. rgbasm
/// reports it once per byte; the build shows one row naming the lines and
/// how many messages it stands for, counts that row once in errors=, and
/// the remedy says where code goes.
#[test]
fn code_in_a_ram_section_is_one_row_with_its_lines_and_a_remedy_naming_a_rom_section() {
    let p = Project::game("rgbds-code-in-wram");
    let main = p.path("game/src/main.asm");
    let mut text = fs::read_to_string(&main).unwrap();
    // `ld a, 1` is the fourth line added.
    let first = text.lines().count() + 4;
    text += "SECTION \"W\", WRAM0\nwX: ds 1\nFoo:\n\tld a, 1\n\tld b, 2\n\tret\n";
    fs::write(&main, &text).unwrap();
    let last = first + 2;

    let rows = p.rgbds(&["build", "game"]);
    rows.fails(
        4,
        "build_failed",
        &format!(
            "code and data go in a ROM0 or ROMX section: start one (`SECTION \"Code\", ROM0`) \
             before the first such line (game/src/main.asm:{first}), then `gbcli build game` \
             again"
        ),
    );
    assert_eq!(
        rows.stdout,
        format!(
            "diagnostic severity=error tool=rgbasm file=game/src/main.asm line={first} \
             lines={first}-{last} count=5 message=\"Section \\\"W\\\" cannot contain code or \
             data (not `ROM0` or `ROMX`)\"\n\
             font source=game/assets/fonts/main.yaff label=Main glyphs=95 fixed=0\n\
             images include=game/build/images/images.inc converted=0 fonts=1 errors=0 \
             warnings=0\n\
             song path=game/assets/music/theme.song asm=game/build/music/theme.asm \
             descriptor=song_theme\n\
             build rgbds=1.0.4 target=dmg sources=7 errors=1 warnings=0\n"
        ),
        "{}",
        rows.stdout
    );

    let json = p.rgbds(&["build", "game", "--json"]);
    let line: serde_json::Value = serde_json::from_str(json.stdout.trim()).unwrap();
    let diagnostics = &line["error"]["result"]["build"]["diagnostics"];
    assert_eq!(diagnostics.as_array().unwrap().len(), 1, "{diagnostics}");
    assert_eq!(diagnostics[0]["line"], first);
    assert_eq!(diagnostics[0]["last_line"], last);
    assert_eq!(diagnostics[0]["count"], 5);
}

#[test]
fn a_macros_call_site_comes_back_in_rows_and_json() {
    let p = Project::game("rgbds-macro");
    fs::write(
        p.path("game/src/extra.asm"),
        "MACRO Bad\n\tld q, \\1\nENDM\nSECTION \"extra\", ROM0\n\tBad 1\n",
    )
    .unwrap();
    let rows = p.rgbds(&["build", "game"]);
    rows.fails(4, "build_failed", "then `gbcli build game` again");
    assert!(
        rows.stdout.starts_with(
            "diagnostic severity=error tool=rgbasm file=game/src/extra.asm line=2 within=Bad \
             message=\"syntax error, unexpected symbol\"\n  from file=game/src/extra.asm line=5\n"
        ),
        "{}",
        rows.stdout
    );

    let json = p.rgbds(&["build", "game", "--json"]);
    let line: serde_json::Value = serde_json::from_str(json.stdout.trim()).unwrap();
    let first = &line["error"]["result"]["build"]["diagnostics"][0];
    assert_eq!(first["within"], "Bad");
    assert_eq!(
        first["chain"],
        serde_json::json!([{"file": "game/src/extra.asm", "line": 5}])
    );
}

#[test]
fn without_rgbds_build_and_doctor_name_the_install_command() {
    let p = Project::new("rgbds-missing");
    let made = p.rgbds(&["new", "game"]);
    assert_eq!(made.code, 0, "{}", made.stderr);
    let empty = p.path("no-tools");
    fs::create_dir(&empty).unwrap();
    let path = empty.display().to_string();
    let build = p.gbcli_with(&["build", "game"], &[("PATH", &path)], None);
    build.fails(5, "toolchain_missing", "`brew install rgbds`");
    assert!(
        build.stderr.contains("rgbasm is not on PATH"),
        "{}",
        build.stderr
    );

    let doctor = p.gbcli_with(&["doctor"], &[("PATH", &path)], None);
    doctor.fails(5, "toolchain_missing", "releases/tag/v1.0.4");
    assert!(
        doctor.stdout.starts_with("check name=rgbasm status=fail "),
        "{}",
        doctor.stdout
    );
}

#[test]
fn doctor_passes_with_the_pinned_rgbds_and_needs_no_session() {
    let p = Project::new("rgbds-doctor");
    let run = p.rgbds(&["doctor"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    for name in ["rgbasm", "rgblink", "rgbfix"] {
        assert!(
            run.stdout
                .contains(&format!("check name={name} status=ok detail=\"1.0.4 at ")),
            "{}",
            run.stdout
        );
    }
    assert!(run.stdout.contains("check name=boot_roms status=ok "));
    assert!(run.stdout.contains("check name=core_support status=skip "));
    assert!(!p.path(".gbcli").exists(), "doctor leaves no trace");
}

/// `git init` in `dir`, whatever repository the test itself runs inside
/// (a pre-commit hook exports `GIT_DIR` and `GIT_INDEX_FILE`).
fn git_init(dir: &std::path::Path) {
    let mut init = std::process::Command::new("git");
    init.args(["init", "-q", "."]).current_dir(dir);
    for var in gbcli_build::GIT_LOCAL_ENV_VARS {
        init.env_remove(var);
    }
    assert!(init.status().expect("these tests need git").success());
}

#[test]
fn doctor_asks_git_without_the_environments_config_either() {
    // A git that runs a hook passes its `-c` settings on in
    // GIT_CONFIG_PARAMETERS; they belong to that call, not to the project's
    // repository, which here doesn't ignore .gbcli/.
    let p = Project::new("rgbds-doctor-git-config");
    git_init(&p.dir);
    let excludes = p.path("excludes");
    std::fs::write(&excludes, ".gbcli/\n").unwrap();
    let path = format!("{}:/usr/bin:/bin", rgbds_dir());
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_gbcli"))
        .arg("doctor")
        .current_dir(&p.dir)
        .env("PATH", &path)
        .env("GIT_CONFIG_GLOBAL", "/dev/null") // whatever this machine ignores
        .env(
            "GIT_CONFIG_PARAMETERS",
            format!("'core.excludesfile={}'", excludes.display()),
        )
        .output()
        .unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("check name=gitignore status=warn "),
        "{stdout}"
    );
}

#[test]
fn doctor_asks_git_about_the_projects_repository_not_the_environments() {
    // A doctor run from a git hook inherits GIT_DIR naming the hook's
    // repository; the gitignore check is about the project's own, here a
    // directory inside a repository whose root ignores its .gbcli/.
    let p = Project::new("rgbds-doctor-git-dir");
    git_init(&p.dir);
    std::fs::write(p.path(".gitignore"), "/game/.gbcli/\n").unwrap();
    let game = p.path("game");
    std::fs::create_dir_all(&game).unwrap();
    let other = p.path("elsewhere");
    std::fs::create_dir_all(&other).unwrap();
    git_init(&other);
    let path = format!("{}:/usr/bin:/bin", rgbds_dir());
    let git_dir = other.join(".git");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_gbcli"))
        .arg("doctor")
        .current_dir(&game)
        .env("PATH", &path)
        .env("GIT_DIR", &git_dir)
        .env("GIT_INDEX_FILE", git_dir.join("index"))
        .output()
        .unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("check name=gitignore status=ok "),
        "{stdout}"
    );
}

#[test]
fn build_restore_reloads_a_checkpoint_unless_what_it_ran_moved() {
    let p = game_in_outer_bundle("rgbds-restore");
    let run = p.rgbds(&[
        "run",
        "game/build/game.gb",
        "--session",
        "game",
        "--frames",
        "30",
        "--checkpoint",
        "start",
    ]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let main = p.path("game/src/main.asm");
    let text = fs::read_to_string(&main).unwrap();

    // A new palette moves nothing: the checkpoint loads into the rebuild.
    fs::write(&main, text.replacen("%11_10_01_00", "%00_01_10_11", 1)).unwrap();
    let restore = &["build", "game", "--session", "game", "--restore", "start"];
    let same = p.rgbds(restore);
    assert_eq!(same.code, 0, "{}{}", same.stdout, same.stderr);
    assert!(same.stdout.contains("output kind=rom "), "{}", same.stdout);
    assert!(
        same.stdout.contains("restored name=start "),
        "{}",
        same.stdout
    );
    assert!(same.stdout.ends_with("rev s2\n"), "{}", same.stdout);

    // An instruction before the main loop moves the routine the checkpoint
    // was running: refused by the rebuilt-ROM rule, the build still reported.
    fs::write(&main, text.replacen("MainLoop:\n", "\tnop\nMainLoop:\n", 1)).unwrap();
    let moved = p.rgbds(restore);
    moved.fails(3, "rom_changed", "--allow-rom-change");
    assert!(
        moved.stdout.contains("output kind=rom "),
        "{}",
        moved.stdout
    );
    let forced = p.rgbds(&[
        "build",
        "game",
        "--session",
        "game",
        "--restore",
        "start",
        "--allow-rom-change",
    ]);
    assert_eq!(forced.code, 0, "{}", forced.stderr);
    assert!(forced.stderr.starts_with("warning: "), "{}", forced.stderr);
}

/// `build DIR` takes DIR as the project, so its `--session` is found from
/// DIR too, not from the cwd (leaf emtFvQ, the racer's `no_session`).
#[test]
fn build_dir_finds_the_session_in_dir() {
    let p = Project::game("rgbds-restore-in-dir");
    let run = p.rgbds(&[
        "run",
        "build/game.gb",
        "--dir",
        "game",
        "--session",
        "game",
        "--frames",
        "30",
        "--checkpoint",
        "start",
    ]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(p.path("game/.gbcli/sessions/game").is_dir());
    assert!(!p.path(".gbcli").exists(), "a bundle in the cwd");
    let restore = p.rgbds(&["build", "game", "--session", "game", "--restore", "start"]);
    assert_eq!(restore.code, 0, "{}{}", restore.stdout, restore.stderr);
    assert!(
        restore.stdout.contains("restored name=start "),
        "{}",
        restore.stdout
    );
}

#[test]
fn build_restore_needs_a_session_over_the_rom_it_built() {
    let p = game_in_outer_bundle("rgbds-restore-errors");
    // A project bundle, so the server (not a missing `.gbcli/`) answers.
    let other = p.rgbds(&["show", "--session", "demo", "--rom", "demo.gb"]);
    assert_eq!(other.code, 0, "{}", other.stderr);
    p.rgbds(&["build", "game", "--restore", "start"])
        .fails(2, "bad_request", "--session NAME");
    p.rgbds(&["build", "game", "--session", "game"])
        .fails(2, "bad_request", "--restore NAME");
    // --as only names who reloads the session: alone, it would be ignored.
    p.rgbds(&["build", "game", "--as", "me"])
        .fails(2, "bad_request", "--restore NAME");
    // --dir is the project, as in every verb: it builds without a restore.
    let built = p.rgbds(&["build", "game", "--dir", "."]);
    assert_eq!(built.code, 0, "{}", built.stderr);
    // A session over another ROM would restore into that ROM, not the build.
    let wrong = p.rgbds(&["build", "game", "--session", "demo", "--restore", "start"]);
    wrong.fails(2, "bad_request", "gbcli run game/build/game.gb --session");
    assert!(wrong.stderr.contains("demo.gb"), "{}", wrong.stderr);
}

/// A session on the other target's build of the same project is refused like
/// any other ROM, but the remedy names the likely cause: --gbc given or left
/// out, and where each build goes.
#[test]
fn build_restore_into_the_other_targets_session_names_the_gbc_flag() {
    let p = game_with_session("rgbds-restore-other-target");
    let gbc = p.rgbds(&[
        "build",
        "game",
        "--gbc",
        "--session",
        "game",
        "--restore",
        "x",
    ]);
    gbc.fails(2, "bad_request", "without --gbc");
    assert!(gbc.stderr.contains("build/game.gb"), "{}", gbc.stderr);
    assert!(gbc.stderr.contains("build/gbc/game.gbc"), "{}", gbc.stderr);

    let run = p.rgbds(&[
        "show",
        "--rom",
        "game/build/gbc/game.gbc",
        "--session",
        "color",
    ]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let plain = p.rgbds(&["build", "game", "--session", "color", "--restore", "x"]);
    plain.fails(2, "bad_request", "with --gbc");
    assert!(
        plain.stderr.contains("build/gbc/game.gbc"),
        "{}",
        plain.stderr
    );

    // Under --out the build isn't a default ROM, so --gbc isn't the slip
    // (review-fix of 81ae475): the remedy is the generic one.
    let out = p.rgbds(&[
        "build",
        "game",
        "--gbc",
        "--out",
        "build/custom.gbc",
        "--session",
        "game",
        "--restore",
        "x",
    ]);
    out.fails(2, "bad_request", "--session NAME");
    assert!(!out.stderr.contains("--gbc"), "{}", out.stderr);
}

/// The responses `gbcli serve` writes for `lines`, run with only the pinned
/// RGBDS on `PATH`.
fn served(p: &Project, args: &[&str], lines: &[&str]) -> Vec<serde_json::Value> {
    let input: String = lines.iter().map(|l| format!("{l}\n")).collect();
    let run = p.gbcli_with(args, &[("PATH", &rgbds_dir())], Some(&input));
    assert_eq!(run.code, 0, "{}", run.stderr);
    run.stdout
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn a_serve_client_builds_and_restores_in_one_request() {
    let p = Project::game("rgbds-serve");
    let run = p.rgbds(&[
        "run",
        "game/build/game.gb",
        "--session",
        "game",
        "--frames",
        "30",
        "--checkpoint",
        "start",
    ]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let main = p.path("game/src/main.asm");
    let text = fs::read_to_string(&main).unwrap();
    fs::write(&main, text.replacen("%11_10_01_00", "%00_01_10_11", 1)).unwrap();

    let line = r#"{"id":"b","op":"build","args":{"dir":"game","restore":"start"}}"#;
    let answers = served(&p, &["serve", "--session", "game"], &[line]);
    let answer = &answers[0];
    assert_eq!(answer["ok"], true, "{answer}");
    assert_eq!(answer["id"], "b");
    assert_eq!(answer["rev"], "s2");
    let result = &answer["result"];
    assert_eq!(result["build"]["outputs"][0]["path"], "game/build/game.gb");
    assert_eq!(result["restored"]["name"], "start", "{result}");

    // The restore is in the session's event log, as any restore is.
    let events = p.rgbds(&["events", "--session", "game"]);
    assert!(events.stdout.contains("op=restore"), "{}", events.stdout);

    // batch -F takes the same line.
    fs::write(p.path("ops.jsonl"), format!("{line}\n")).unwrap();
    let batch = p.rgbds(&["batch", "-F", "ops.jsonl", "--session", "game"]);
    assert_eq!(batch.code, 0, "{}{}", batch.stdout, batch.stderr);
    assert!(
        batch
            .stdout
            .starts_with("line n=1 id=b op=build ok=1 rev=s3\n"),
        "{}",
        batch.stdout
    );
}

#[test]
fn serve_answers_doctor_and_new_and_a_build_refusal_keeps_its_report() {
    let p = Project::game("rgbds-serve-ops");
    let run = p.rgbds(&["show", "--rom", "game/build/game.gb", "--session", "game"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    fs::write(p.path("game/src/extra.asm"), "\tld q, 1\n").unwrap();
    let answers = served(
        &p,
        &["serve", "--session", "game"],
        &[
            r#"{"id":"1","op":"doctor"}"#,
            r#"{"id":"2","op":"new","args":{"dir":"other"}}"#,
            r#"{"id":"3","op":"build","args":{"dir":"game"}}"#,
            r#"{"id":"4","op":"build","args":{"dir":"game","restore":"start"}}"#,
        ],
    );
    assert_eq!(answers[0]["ok"], true, "{}", answers[0]);
    assert_eq!(
        answers[0]["result"]["doctor"]["checks"][0]["name"],
        "rgbasm"
    );
    assert_eq!(answers[1]["result"]["created"]["name"], "other");
    assert!(p.path("other/src/main.asm").is_file());
    let error = &answers[2]["error"];
    assert_eq!(error["code"], "build_failed", "{}", answers[2]);
    assert!(
        error["remedy"]
            .as_str()
            .unwrap()
            .contains("`gbcli build game`"),
        "{error}"
    );
    assert_eq!(
        error["result"]["build"]["diagnostics"][0]["file"],
        "game/src/extra.asm"
    );
    // A failed build restores nothing.
    assert_eq!(answers[3]["error"]["code"], "build_failed");
}

#[test]
fn the_clis_build_is_the_serve_op() {
    let p = Project::game("rgbds-same-path");
    let cli = p.rgbds(&["build", "game", "--json"]);
    assert_eq!(cli.code, 0, "{}", cli.stderr);
    // serve needs a bundle; build itself doesn't touch it.
    let run = p.rgbds(&["show", "--rom", "game/build/game.gb", "--session", "game"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let input = r#"{"id":"cli","op":"build","args":{"dir":"game"}}"#;
    let theirs = p.gbcli_with(
        &["serve"],
        &[("PATH", &rgbds_dir())],
        Some(&format!("{input}\n")),
    );
    assert_eq!(cli.stdout, theirs.stdout);
}

/// `--gbc` is the wire's `"target":"gbc"`, and the ROM it writes is dual
/// mode ($0143 = $80).
#[test]
fn the_clis_gbc_build_is_the_serve_op_with_target_gbc() {
    let p = Project::game("rgbds-gbc-same-path");
    let cli = p.rgbds(&["build", "game", "--gbc", "--json"]);
    assert_eq!(cli.code, 0, "{}", cli.stderr);
    // Its own ROM, beside the plain build's (leaf XtevSe).
    assert_eq!(
        fs::read(p.path("game/build/gbc/game.gbc")).unwrap()[0x143],
        0x80
    );
    let run = p.rgbds(&[
        "show",
        "--rom",
        "game/build/gbc/game.gbc",
        "--session",
        "game",
    ]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let input = r#"{"id":"cli","op":"build","args":{"dir":"game","target":"gbc"}}"#;
    let theirs = p.gbcli_with(
        &["serve"],
        &[("PATH", &rgbds_dir())],
        Some(&format!("{input}\n")),
    );
    assert_eq!(cli.stdout, theirs.stdout);
}

/// `gbcli build ARGS --json` and `gbcli serve` given the CLI's request line
/// (`id` "cli"): the same answer, and the ROM as it was. A palette change in
/// the source means a build that ran would have rewritten the ROM.
fn same_refusal(p: &Project, args: &[&str], line: &str, code: &str) {
    let rom = p.path("game/build/game.gb");
    let before = fs::read(&rom).unwrap();
    let cli = p.rgbds(&[args, &["--json"]].concat());
    let answers = served(p, &["serve"], &[line]);
    let ours: serde_json::Value = serde_json::from_str(&cli.stdout)
        .unwrap_or_else(|e| panic!("{args:?} printed no response ({e}): {}", cli.stderr));
    assert_eq!(ours, answers[0], "{args:?}");
    assert_eq!(
        answers[0]["error"]["code"], code,
        "{args:?}: {}",
        answers[0]
    );
    assert_eq!(fs::read(&rom).unwrap(), before, "{args:?} rebuilt the ROM");
}

/// A project with a session `game` on its build, and a source change that
/// a build would pick up.
/// A starter project without its own `.gbcli/`, so its sessions live in the
/// outer bundle that the CLI (searching from `game/`) and a `serve` started
/// outside it both find. `gbcli new` makes the project's own bundle; these
/// tests are about restoring, not about which bundle a session lives in.
fn game_in_outer_bundle(name: &str) -> Project {
    let p = Project::game(name);
    fs::remove_dir_all(p.path("game/.gbcli")).unwrap();
    p
}

fn game_with_session(name: &str) -> Project {
    let p = game_in_outer_bundle(name);
    let run = p.rgbds(&["show", "--rom", "game/build/game.gb", "--session", "game"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let main = p.path("game/src/main.asm");
    let text = fs::read_to_string(&main).unwrap();
    fs::write(&main, text.replacen("%11_10_01_00", "%00_01_10_11", 1)).unwrap();
    p
}

#[test]
fn build_restore_into_a_missing_session_is_refused_before_building_by_cli_and_serve_alike() {
    let p = game_with_session("rgbds-restore-missing");
    same_refusal(
        &p,
        &["build", "game", "--session", "nope", "--restore", "start"],
        r#"{"id":"cli","session":"nope","op":"build","args":{"dir":"game","restore":"start"}}"#,
        "no_session",
    );
    same_refusal(
        &p,
        &["build", "game", "--restore", "start"],
        r#"{"id":"cli","op":"build","args":{"dir":"game","restore":"start"}}"#,
        "bad_request",
    );
}

#[test]
fn restore_only_fields_without_restore_are_refused_by_cli_and_serve_alike() {
    let p = game_with_session("rgbds-restore-only");
    let cases: [(&[&str], &str); 3] = [
        (
            &["build", "game", "--session", "game"],
            r#"{"id":"cli","session":"game","op":"build","args":{"dir":"game"}}"#,
        ),
        (
            &["build", "game", "--rev", "s1"],
            r#"{"id":"cli","rev":"s1","op":"build","args":{"dir":"game"}}"#,
        ),
        (
            &["build", "game", "--allow-rom-change"],
            r#"{"id":"cli","allow_rom_change":true,"op":"build","args":{"dir":"game"}}"#,
        ),
    ];
    for (args, line) in cases {
        same_refusal(&p, args, line, "bad_request");
    }
}

/// GBCLI_SESSION is a default, as `serve --session` is: it doesn't make a
/// build that restores nothing name a session.
#[test]
fn a_build_with_gbcli_session_set_still_builds() {
    let p = game_with_session("rgbds-env-session");
    let run = p.gbcli_with(
        &["build", "game"],
        &[("PATH", &rgbds_dir()), ("GBCLI_SESSION", "game")],
        None,
    );
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
}

#[test]
fn new_refuses_a_directory_that_holds_files() {
    let p = Project::new("rgbds-new-full");
    fs::create_dir(p.path("game")).unwrap();
    fs::write(p.path("game/keep.txt"), "mine").unwrap();
    p.rgbds(&["new", "game"])
        .fails(2, "bad_request", "gbcli new my-game");
    p.rgbds(&["new"]).fails(2, "bad_request", "gbcli new DIR");
}

#[test]
fn the_loop_in_rows_and_json() {
    let p = Project::new("rgbds-golden");
    let mut transcript = String::new();
    let scripts: &[&[&str]] = &[
        &["new", "game"],
        &["build", "game"],
        &["build", "game", "--json"],
        &["build", "game", "--gbc"],
        &["build", "game", "--gbc", "--json"],
    ];
    for args in scripts {
        transcript += &p.rgbds(args).transcript(args, &p.dir);
    }
    let main = p.path("game/src/main.asm");
    let text = fs::read_to_string(&main).unwrap();
    fs::write(&main, text.replacen("\tld sp, $E000", "\tld sp,", 1)).unwrap();
    for args in [&["build", "game"][..], &["build", "game", "--json"]] {
        transcript += &p.rgbds(args).transcript(args, &p.dir);
    }
    golden("rgbds.txt", &transcript);
}

#[test]
fn build_dir_names_the_project_as_it_does_for_every_verb() {
    let p = Project::new("rgbds-build-dir");
    let made = p.rgbds(&["new", "game"]);
    assert_eq!(made.code, 0, "{}", made.stderr);
    let built = p.rgbds(&["build", "--dir", "game"]);
    assert_eq!(built.code, 0, "{}{}", built.stdout, built.stderr);
    assert!(
        p.path("game/build").exists(),
        "built the project --dir names"
    );
}
