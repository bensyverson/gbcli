//! Every error code a CLI invocation can reach, through the binary: its exit
//! code, its name on stderr and the remedy it teaches (cli-design ▸ Errors
//! that teach). `build_failed` and `toolchain_missing` are tested with the
//! RGBDS loop in `rgbds.rs`, and `song_unreadable` with `song` in `song.rs`;
//! `internal` has no way to make the handler panic on purpose.

use std::fs;

use crate::common::{Project, V2, V2_SYM};

fn run1(project: &Project) {
    let run = project.gbcli(&["run", "--session", "demo", "--frames", "1"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
}

#[test]
fn usage_errors_name_near_misses_and_the_next_command() {
    let p = Project::new("usage");
    p.gbcli(&["rn"])
        .fails(2, "bad_request", "did you mean `run`?");
    p.gbcli(&["run", "--frame", "2"]).fails(
        2,
        "bad_request",
        "did you mean `--frames` or `--frame-budget`?",
    );
    p.gbcli(&["run", "--frames"])
        .fails(2, "bad_request", "`--frames N`");
    p.gbcli(&["run", "--frames", "ten"])
        .fails(2, "bad_request", "e.g. `--frames 120`");
    p.gbcli(&["watch", "wFrame"]).fails(
        2,
        "bad_request",
        "usage: gbcli watch read|write|access ADDR",
    );
    p.gbcli(&["help", "adresses"])
        .fails(2, "bad_request", "did you mean `gbcli help addresses`?");
    assert!(!p.path(".gbcli").exists(), "a usage error leaves no trace");
}

#[test]
fn frame_not_normal_offers_any_frame_and_keeps_the_run() {
    let p = Project::with_session("frame-not-normal");
    let run = p.gbcli(&[
        "run",
        "--session",
        "demo",
        "--frames",
        "2",
        "--write",
        "rLCDC=0",
        "--shot",
        "off.png",
    ]);
    run.fails(1, "frame_not_normal", "pass --any-frame");
    assert!(run.stdout.ends_with("rev s1\n"), "{}", run.stdout);
    assert!(!p.path("off.png").exists());
}

#[test]
fn bad_request_lists_the_checkpoints_that_exist() {
    let p = Project::with_session("bad-request");
    p.gbcli(&["restore", "nosuch", "--session", "demo"]).fails(
        2,
        "bad_request",
        "this session has no named checkpoints",
    );
}

#[test]
fn bad_address_says_what_fits() {
    let p = Project::with_session("bad-address");
    p.gbcli(&["break", "wram:0x2000", "--session", "demo"])
        .fails(2, "bad_address", "wram:");
}

#[test]
fn bad_expression_points_at_the_grammar() {
    let p = Project::with_session("bad-expression");
    p.gbcli(&["read", "u16[", "--session", "demo"]).fails(
        2,
        "bad_expression",
        "gbcli help expressions",
    );
}

#[test]
fn an_oam_entry_past_the_last_or_an_unknown_field_names_what_fits() {
    let p = Project::with_session("bad-oam");
    p.gbcli(&["read", "oam[40]", "--session", "demo"])
        .fails(2, "bad_expression", "oam[39]");
    p.gbcli(&["read", "oam[0].flags", "--session", "demo"])
        .fails(2, "bad_expression", "oam[0].attr");
}

#[test]
fn ambiguous_symbol_lists_every_candidate() {
    let p = Project::new("ambiguous");
    fs::write(
        p.path("demo.sym"),
        "00:0150 Main\n01:4000 Init\n02:4000 Init\n00:c000 wFrame\n",
    )
    .unwrap();
    p.gbcli(&["show", "--session", "demo", "--rom", "demo.gb"]);
    p.gbcli(&["read", "pc == Init", "--session", "demo"])
        .fails(2, "ambiguous_symbol", "01:4000");
}

#[test]
fn unknown_symbol_names_the_near_misses() {
    let p = Project::with_session("unknown-symbol");
    p.gbcli(&["read", "[wFrme]", "--session", "demo"])
        .fails(2, "unknown_symbol", "wFrame");
}

#[test]
fn refused_names_the_flag_it_needs() {
    let p = Project::with_session("refused");
    p.gbcli(&["checkpoint", "keep", "--session", "demo"]);
    p.gbcli(&["sessions", "rm", "demo"])
        .fails(2, "refused", "--with-checkpoints");
    p.gbcli(&["poke", "$0150=0", "--session", "demo"])
        .fails(2, "refused", "use --patch");
}

#[test]
fn budget_exhausted_prints_the_stop_it_reached() {
    let p = Project::with_session("budget");
    let run = p.gbcli(&[
        "run",
        "--session",
        "demo",
        "--until",
        "u16[wFrame] >= 60000",
        "--frames",
        "1",
    ]);
    run.fails(3, "budget_exhausted", "--frames");
    assert!(
        run.stdout.starts_with("stop budget_exhausted "),
        "{}",
        run.stdout
    );
    assert!(run.stdout.ends_with("rev s1\n"), "{}", run.stdout);
}

#[test]
fn predicate_false_prints_the_measurement_it_asserted_on() {
    let p = Project::with_session("predicate");
    let run = p.gbcli(&[
        "measure",
        "Main.loop..Main.loop",
        "--max",
        "100",
        "--session",
        "demo",
    ]);
    run.fails(1, "predicate_false", "raise --max");
    assert!(
        run.stdout.starts_with("measure from=Main.loop "),
        "{}",
        run.stdout
    );
    assert!(
        run.stdout.contains("\nt min=112 max=112 "),
        "{}",
        run.stdout
    );
}

#[test]
fn rev_mismatch_says_to_re_read() {
    let p = Project::with_session("rev");
    run1(&p);
    p.gbcli(&["run", "--session", "demo", "--frames", "1", "--rev", "s0"])
        .fails(3, "rev_mismatch", "re-read the session");
}

#[test]
#[ignore = "pre-release: waits out the 5 s session lock budget"]
fn session_busy_says_to_wait() {
    let p = Project::with_session("busy");
    let lock = fs::File::open(p.path(".gbcli/locks/demo.lock")).unwrap();
    lock.lock().unwrap();
    p.gbcli(&["show", "--session", "demo"])
        .fails(3, "session_busy", "wait for the other writer");
}

#[test]
fn rom_changed_offers_the_override() {
    let p = Project::with_session("rom-changed");
    run1(&p);
    fs::write(p.path("demo.gb"), V2).unwrap();
    fs::write(p.path("demo.sym"), V2_SYM).unwrap();
    p.gbcli(&["run", "--session", "demo", "--frames", "1"])
        .fails(3, "rom_changed", "--allow-rom-change");
    let forced = p.gbcli(&[
        "run",
        "--session",
        "demo",
        "--frames",
        "1",
        "--allow-rom-change",
    ]);
    assert_eq!(forced.code, 0, "{}", forced.stderr);
}

#[test]
fn uninitialized_read_offers_allow_uninit() {
    let p = Project::with_session("uninit");
    run1(&p);
    p.gbcli(&["read", "[wNever]", "--session", "demo"]).fails(
        3,
        "uninitialized_read",
        "--allow-uninit",
    );
}

#[test]
fn rom_unreadable_names_the_path() {
    let p = Project::new("rom-unreadable");
    p.gbcli(&["run", "nosuch.gb", "--session", "demo"])
        .fails(4, "rom_unreadable", "nosuch.gb");
}

fn checkpoint_file(p: &Project) -> std::path::PathBuf {
    run1(p);
    let made = p.gbcli(&["checkpoint", "one", "--session", "demo"]);
    assert_eq!(made.code, 0, "{}", made.stderr);
    p.path(".gbcli/sessions/demo/checkpoints/one.gbck")
}

/// Replaces `from` with `to` (the same length) in a checkpoint's header.
fn edit_header(path: &std::path::Path, from: &str, to: &str) {
    assert_eq!(from.len(), to.len());
    let mut bytes = fs::read(path).unwrap();
    let at = bytes
        .windows(from.len())
        .position(|w| w == from.as_bytes())
        .unwrap_or_else(|| panic!("{from} not in the header"));
    bytes[at..at + to.len()].copy_from_slice(to.as_bytes());
    fs::write(path, bytes).unwrap();
}

#[test]
fn checkpoint_unreadable_offers_a_way_back() {
    let p = Project::with_session("cp-unreadable");
    fs::write(checkpoint_file(&p), b"GBCK").unwrap();
    p.gbcli(&["restore", "one", "--session", "demo"]).fails(
        4,
        "checkpoint_unreadable",
        "checkpoint",
    );
}

#[test]
fn checkpoint_mismatch_names_both_cores() {
    let p = Project::with_session("cp-mismatch");
    edit_header(
        &checkpoint_file(&p),
        "\"backend\":\"sameboy\"",
        "\"backend\":\"sameboz\"",
    );
    p.gbcli(&["restore", "one", "--session", "demo"])
        .fails(4, "checkpoint_mismatch", "sameboz");
}

#[test]
fn boot_rom_missing_names_the_hash() {
    let p = Project::with_session("boot-missing");
    let path = checkpoint_file(&p);
    let text = String::from_utf8_lossy(&fs::read(&path).unwrap()).into_owned();
    let key = "\"boot_rom_sha256\":\"";
    let at = text.find(key).expect("the header names its boot ROM") + key.len();
    let hash = &text[at..at + 64];
    let other: String = std::iter::repeat_n('0', 64).collect();
    edit_header(&path, hash, &other);
    p.gbcli(&["restore", "one", "--session", "demo"]).fails(
        5,
        "boot_rom_missing",
        "re-create it from the ROM",
    );
}

#[test]
fn symbols_unreadable_names_the_flag() {
    let p = Project::new("sym-unreadable");
    p.gbcli(&[
        "show",
        "--session",
        "demo",
        "--rom",
        "demo.gb",
        "--sym",
        "nosuch.sym",
    ])
    .fails(4, "symbols_unreadable", "--sym");
}

#[test]
fn session_unreadable_offers_removal() {
    let p = Project::with_session("session-unreadable");
    fs::write(p.path(".gbcli/sessions/demo/session.json"), "{").unwrap();
    p.gbcli(&["show", "--session", "demo"]).fails(
        4,
        "session_unreadable",
        "gbcli sessions rm demo",
    );
}

#[test]
fn unsupported_is_counted_in_the_gap_ledger() {
    let p = Project::with_session("unsupported");
    p.gbcli(&["read", "[$FEA0]", "--session", "demo"])
        .fails(5, "unsupported", "gbcli");
    let gaps = p.gbcli(&["gaps"]);
    assert_eq!(gaps.stdout, "gap feature=extra_oam count=1\n");
}

#[test]
fn no_session_says_how_to_make_one() {
    let p = Project::with_session("no-session");
    p.gbcli(&["show", "--session", "nosuch"])
        .fails(5, "no_session", "known sessions: demo");
    p.gbcli(&["show"]).fails(5, "no_session", "--session");
    let fresh = Project::new("no-session-fresh");
    fresh
        .gbcli(&["run", "demo.gb", "--frames", "1"])
        .fails(5, "no_session", "add --session NAME");
    assert!(!fresh.path(".gbcli").exists());
}

#[test]
fn io_failed_names_the_path() {
    use std::os::unix::fs::PermissionsExt;
    let p = Project::with_session("io-failed");
    let history = p.path(".gbcli/sessions/demo/history");
    fs::set_permissions(&history, fs::Permissions::from_mode(0o555)).unwrap();
    let run = p.gbcli(&["run", "--session", "demo", "--frames", "1"]);
    fs::set_permissions(&history, fs::Permissions::from_mode(0o755)).unwrap();
    run.fails(5, "io_failed", "check permissions");
}
