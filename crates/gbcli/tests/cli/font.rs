//! `gbcli font` through the binary: a font read, a sample set and
//! previewed, `--fit` failing past its width, a bad font located, and the
//! sample's tiles checked against the layout rules as `gbcli help fonts`
//! states them, computed here independently of gbcli-font.

use std::fs;

use crate::common::{Project, golden};

/// ' ' 3 wide, 'H' 5, 'i' 2 (ink in column 0, paper in column 1), and a
/// named heart 7 wide.
const FONT: &str = "\
name: Test

u+0020:
    ...
    ...
    ...
    ...
    ...
    ...
    ...
    ...

'H':
    @...@
    @...@
    @@@@@
    @...@
    @...@
    @...@
    .....
    .....

'i':
    @.
    ..
    @.
    @.
    @.
    @.
    ..
    ..

\"heart\":
    .@@.@@.
    @@@@@@@
    @@@@@@@
    .@@@@@.
    ..@@@..
    ...@...
    .......
    .......
";

fn project(name: &str) -> Project {
    let p = Project::new(name);
    fs::write(p.path("main.yaff"), FONT).unwrap();
    p
}

#[test]
fn font_reads_sets_a_sample_previews_and_fits() {
    let p = project("font");
    let mut transcript = String::new();
    let steps: &[(&[&str], i32)] = &[
        (&["font", "main.yaff"], 0),
        (
            &[
                "font",
                "main.yaff",
                "--sample",
                "Hi <heart>?",
                "--preview",
                "line.png",
            ],
            0,
        ),
        (&["font", "main.yaff", "--preview", "all.png"], 0),
        (&["font", "main.yaff", "--sample", "HiHi", "--fit", "14"], 0),
        (&["font", "main.yaff", "--sample", "HiHi", "--fit", "13"], 1),
    ];
    for (args, code) in steps {
        let run = p.gbcli(args);
        assert_eq!(run.code, *code, "{args:?}: {}", run.stderr);
        transcript += &run.transcript(args, &p.dir);
    }
    golden("font.txt", &transcript);
    assert!(p.path("line.png").is_file() && p.path("all.png").is_file());
}

#[test]
fn a_bad_font_is_font_unreadable_at_its_line_and_column() {
    let p = project("font-bad");
    fs::write(p.path("bad.yaff"), "0x41:\n    @\n").unwrap();
    let run = p.gbcli(&["font", "bad.yaff"]);
    run.fails(4, "font_unreadable", "u+0041");
    assert!(run.stderr.contains("bad.yaff:1:1: "), "{}", run.stderr);
    assert!(
        run.stdout.contains("problem path=bad.yaff line=1 column=1"),
        "{}",
        run.stdout
    );
}

/// Rule 4 of `gbcli help fonts`, as written there: each glyph replaces its
/// own columns, split across two tiles when it crosses a boundary.
fn by_the_rules(glyphs: &[(u8, [u8; 8])]) -> Vec<u8> {
    let width: u32 = glyphs.iter().map(|g| u32::from(g.0)).sum();
    let mut tiles = vec![0u8; width.div_ceil(8) as usize * 8];
    let mut x = 0u32;
    for &(w, rows) in glyphs {
        let (t, o) = ((x / 8) as usize, x % 8);
        let m = (0xFFu16 << (8 - u32::from(w))) as u8;
        for (r, g) in rows.iter().enumerate() {
            let row = &mut tiles[t * 8 + r];
            *row = (*row & !(m >> o)) | ((g & m) >> o);
            if o + u32::from(w) > 8 {
                let row = &mut tiles[(t + 1) * 8 + r];
                *row = (*row & !(m << (8 - o))) | ((g & m) << (8 - o));
            }
        }
        x += u32::from(w);
    }
    tiles
}

#[test]
fn the_sample_is_the_layout_rules_pixel_for_pixel() {
    let help = Project::new("font-rules").gbcli(&["help", "fonts"]);
    for rule in [
        "the pen then moves right",
        "by the glyph's width",
        "there is no kerning",
        "spacing is drawn into it",
        "starts at its cell's left edge",
        "bit 7 - x % 8",
        "row = (row & ~(m >> o)) | ((g & m) >> o)",
        "| ((g & m) << (8 - o))",
    ] {
        assert!(
            help.stdout.contains(rule),
            "{rule} missing:\n{}",
            help.stdout
        );
    }
    let p = project("font-sample");
    let run = p.gbcli(&["font", "main.yaff", "--sample", "HiH <heart>iH", "--json"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let response: gbcli_protocol::Response = serde_json::from_str(&run.stdout).unwrap();
    let sample = response
        .result()
        .unwrap()
        .font
        .clone()
        .unwrap()
        .sample
        .unwrap();
    let h = (5, [0x88, 0x88, 0xF8, 0x88, 0x88, 0x88, 0, 0]);
    let i = (2, [0x80, 0, 0x80, 0x80, 0x80, 0x80, 0, 0]);
    let space = (3, [0; 8]);
    let heart = (7, [0x6C, 0xFE, 0xFE, 0x7C, 0x38, 0x10, 0, 0]);
    let expected = by_the_rules(&[h, i, h, space, heart, i, h]);
    assert_eq!(sample.width, 29);
    assert_eq!(sample.data, expected);
}
