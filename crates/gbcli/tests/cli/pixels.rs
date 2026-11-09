//! `gbcli pixels` through the binary: pixel text checked, written as a PNG,
//! a preview and canonical text, a PNG read back as tiles, and a typo's
//! structured error.

use std::fs;

use crate::common::{Project, golden};

fn project(name: &str) -> Project {
    let p = Project::new(name);
    fs::write(p.path("hero.16x16.pixels"), gbcli_pixels::EXAMPLE).unwrap();
    p
}

#[test]
fn pixels_checks_converts_and_previews() {
    let p = project("pixels");
    let mut transcript = String::new();
    let steps: &[&[&str]] = &[
        &["pixels", "hero.16x16.pixels"],
        &[
            "pixels",
            "hero.16x16.pixels",
            "--png",
            "hero.png",
            "--preview",
            "big.png",
            "--text",
            "canon.pixels",
        ],
        &["pixels", "hero.png", "--text", "tiles.pixels", "--tiles"],
        &["pixels", "hero.png", "--json"],
    ];
    for args in steps {
        let run = p.gbcli(args);
        assert_eq!(run.code, 0, "{args:?}: {}", run.stderr);
        transcript += &run.transcript(args, &p.dir);
    }
    golden("pixels.txt", &transcript);
    let tiles = fs::read_to_string(p.path("tiles.pixels")).unwrap();
    assert!(
        tiles.contains("\nmap 0 0\n00 01 00 01\n02 03 04 05\n"),
        "{tiles}"
    );
    let canon = fs::read_to_string(p.path("canon.pixels")).unwrap();
    assert!(canon.contains("rows 16 0 # frame 1\n"), "{canon}");
}

#[test]
fn a_typo_is_pixels_unreadable_located_to_its_line_and_column() {
    let p = project("pixels-bad");
    let typo = gbcli_pixels::EXAMPLE.replacen("....kssssssk....", "....kssssxsk....", 1);
    fs::write(p.path("bad.pixels"), typo).unwrap();
    let run = p.gbcli(&["pixels", "bad.pixels", "--png", "bad.png"]);
    run.fails(4, "pixels_unreadable", "add `color x VALUE`");
    assert!(
        run.stderr
            .contains("bad.pixels:13:10: x isn't in the legend: it has ., k, s and r"),
        "{}",
        run.stderr
    );
    assert!(
        run.stdout
            .starts_with("problem path=bad.pixels line=13 column=10 "),
        "{}",
        run.stdout
    );
    assert!(!p.path("bad.png").exists(), "nothing is written");
    let json = p.gbcli(&["pixels", "bad.pixels", "--json"]);
    assert_eq!(json.code, 4);
    let line: serde_json::Value = serde_json::from_str(json.stdout.trim()).unwrap();
    let problem = &line["error"]["result"]["pixels_problem"];
    assert_eq!(
        (problem["line"].as_u64(), problem["column"].as_u64()),
        (Some(13), Some(10))
    );
}

#[test]
fn tiles_without_text_is_a_bad_request_and_a_missing_file_is_unreadable() {
    let p = project("pixels-flags");
    p.gbcli(&["pixels", "hero.16x16.pixels", "--tiles"]).fails(
        2,
        "bad_request",
        "add --text PATH, or drop --tiles",
    );
    p.gbcli(&["pixels", "nope.pixels"])
        .fails(4, "pixels_unreadable", "check the path");
}

#[test]
fn help_pixels_teaches_the_format_and_names_no_drawing_tool() {
    let p = Project::new("pixels-help");
    let run = p.gbcli(&["help", "pixels"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    for needle in [
        "format gbcli-pixels 1",
        "color . clear",
        "tile KEY",
        "map",
        "keep",
        "dmg 2",
        "--tiles",
        "pixels_unreadable",
        "a PNG from any tool, or pixel text",
    ] {
        assert!(run.stdout.contains(needle), "{needle}:\n{}", run.stdout);
    }
    assert!(!run.stdout.to_lowercase().contains("woodcase"));
    // The example in the help parses.
    let tail = run.stdout.rsplit("format gbcli-pixels 1").next().unwrap();
    let example = format!("format gbcli-pixels 1{tail}");
    let picture = gbcli_pixels::parse_text(&example).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((picture.width, picture.height), (32, 16));
}
