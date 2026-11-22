//! `gbcli song` through the binary: a hUGETracker sample to text and back,
//! a piano roll from a song file with no build, and a malformed song's
//! structured error.

use std::fs;

use crate::common::{Project, corpus_file, golden};

/// A v5 hUGETracker sample song from the corpus's `hugetracker-samples` suite.
fn peril() -> Vec<u8> {
    corpus_file("hugetracker-samples/Coffee Bat - Pilgrim's Peril Menu.uge")
}
/// gbcli-music's pinned roll of its example song.
const EXAMPLE_PIANO: &[u8] = include_bytes!("../../../music/tests/golden/example_piano.png");

fn project(name: &str) -> Project {
    let p = Project::new(name);
    fs::write(p.path("peril.uge"), peril()).unwrap();
    fs::write(p.path("example.song"), gbcli_music::EXAMPLE).unwrap();
    p
}

#[test]
fn song_converts_both_ways_and_draws_a_piano_roll() {
    let p = project("song");
    let mut transcript = String::new();
    let steps: &[&[&str]] = &[
        &["song", "peril.uge"],
        &["song", "peril.uge", "--text", "peril.song"],
        &[
            "song",
            "peril.song",
            "--uge",
            "back.uge",
            "--piano",
            "peril.png",
        ],
        &["song", "example.song", "--piano", "example.png", "--json"],
    ];
    for args in steps {
        let run = p.gbcli(args);
        assert_eq!(run.code, 0, "{args:?}: {}", run.stderr);
        transcript += &run.transcript(args, &p.dir);
    }
    golden("song.txt", &transcript);

    let text = fs::read_to_string(p.path("peril.song")).unwrap();
    assert!(
        text.starts_with(gbcli_music::FORMAT_LINE),
        "{}",
        &text[..80]
    );
    // The v5 file comes back as v6 with the same song.
    let back = gbcli_music::read_uge(&fs::read(p.path("back.uge")).unwrap()).unwrap();
    assert_eq!(back.version, 6);
    assert!(back.song == gbcli_music::read_uge(&peril()).unwrap().song);
    // The CLI draws exactly the library's pinned roll.
    assert!(fs::read(p.path("example.png")).unwrap() == EXAMPLE_PIANO);
}

#[test]
fn a_malformed_song_is_song_unreadable_located_to_its_line_and_column() {
    let p = project("song-bad");
    let bad = gbcli_music::EXAMPLE.replace("04 E-5 01", "04 H-5 01");
    fs::write(p.path("bad.song"), bad).unwrap();
    let run = p.gbcli(&["song", "bad.song", "--uge", "bad.uge"]);
    run.fails(4, "song_unreadable", "C-3 to B-8");
    assert!(
        run.stderr.contains("bad.song:32:4: `H-5` is not a note"),
        "{}",
        run.stderr
    );
    assert!(
        run.stdout
            .starts_with("problem path=bad.song line=32 column=4 "),
        "{}",
        run.stdout
    );
    assert!(!p.path("bad.uge").exists(), "nothing is written");

    let json = p.gbcli(&["song", "bad.song", "--json"]);
    assert_eq!(json.code, 4);
    let line: serde_json::Value = serde_json::from_str(json.stdout.trim()).unwrap();
    let problem = &line["error"]["result"]["song_problem"];
    assert_eq!(
        (problem["line"].as_u64(), problem["column"].as_u64()),
        (Some(32), Some(4))
    );
}

#[test]
fn a_damaged_uge_names_the_byte_it_ran_out_at() {
    let p = project("song-cut");
    fs::write(p.path("cut.uge"), &peril()[..5000]).unwrap();
    let run = p.gbcli(&["song", "cut.uge"]);
    run.fails(4, "song_unreadable", "hUGETracker");
    assert!(run.stderr.contains("cut.uge: byte 5000:"), "{}", run.stderr);
}

#[test]
fn a_missing_song_file_is_song_unreadable() {
    let p = project("song-missing");
    p.gbcli(&["song", "nope.uge"])
        .fails(4, "song_unreadable", "check the path");
}

#[test]
fn song_warns_of_subpattern_rows_and_jumps_hugedriver_never_plays() {
    let p = project("song-subpattern");
    let instrument = "instrument square 3 \"Arp\"\n  subpattern on\n  05 +00 .. J40 ...\n  \
                      40 +12 .. ... ...\n";
    let text = gbcli_music::EXAMPLE.replacen("\norders", &format!("\n{instrument}\norders"), 1);
    fs::write(p.path("arp.song"), text).unwrap();
    let run = p.gbcli(&["song", "arp.song"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let warnings: Vec<&str> = run
        .stderr
        .lines()
        .filter(|l| l.starts_with("warning: "))
        .collect();
    assert_eq!(warnings.len(), 2, "{}", run.stderr);
    assert!(warnings[0].contains("J40"), "{}", warnings[0]);
    assert!(warnings[1].contains("row 40"), "{}", warnings[1]);
    let json = p.gbcli(&["song", "arp.song", "--json"]);
    let line: serde_json::Value = serde_json::from_str(json.stdout.trim()).unwrap();
    assert_eq!(line["warnings"].as_array().map(Vec::len), Some(2));
    // The example song plays every row it has.
    assert!(p.gbcli(&["song", "example.song"]).stderr.is_empty());
}

#[test]
fn song_needs_its_file() {
    let p = project("song-usage");
    p.gbcli(&["song"]).fails(2, "bad_request", "gbcli song");
}

/// `--asm` writes exactly what the `song` op answers `serve` for the same
/// file and descriptor: by default `song_` and the file's stem, or the one
/// `--descriptor` names.
#[test]
fn song_asm_writes_the_source_the_song_op_answers() {
    let p = project("song-asm");
    let run = p.gbcli(&["song", "example.song", "--asm", "example.asm"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(
        run.stdout.contains("asm path=example.asm bytes=")
            && run.stdout.contains(" descriptor=song_example"),
        "{}",
        run.stdout
    );
    let named = p.gbcli(&[
        "song",
        "peril.uge",
        "--asm",
        "peril.asm",
        "--descriptor",
        "song_menu",
    ]);
    assert_eq!(named.code, 0, "{}", named.stderr);
    let lines = "{\"id\":\"1\",\"op\":\"song\",\"args\":{\"path\":\"example.song\",\"descriptor\":\"song_example\"}}\n\
                 {\"id\":\"2\",\"op\":\"song\",\"args\":{\"path\":\"peril.uge\",\"descriptor\":\"song_menu\"}}\n";
    let serve = p.gbcli_with(&["serve"], &[], Some(lines));
    assert_eq!(serve.code, 0, "{}", serve.stderr);
    let answers: Vec<gbcli_protocol::Response> = serve
        .stdout
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    for (answer, file) in answers.iter().zip(["example.asm", "peril.asm"]) {
        let made = answer
            .outcome
            .as_ref()
            .expect("ok")
            .song_asm
            .clone()
            .unwrap();
        assert!(
            made.asm.as_bytes() == fs::read(p.path(file)).unwrap(),
            "{file} differs from the op's answer"
        );
    }
}

#[test]
fn a_descriptor_that_is_no_symbol_is_refused_and_nothing_is_written() {
    let p = project("song-asm-bad");
    let run = p.gbcli(&[
        "song",
        "example.song",
        "--asm",
        "x.asm",
        "--descriptor",
        "song.x",
    ]);
    run.fails(2, "bad_request", "letters");
    assert!(!p.path("x.asm").exists());
}
