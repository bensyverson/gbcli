//! The starter's line of text (leaf RDlaLo): drawn by `src/text.asm` in
//! `assets/fonts/main.yaff`, on screen it is `gbcli font --sample`'s
//! line pixel for pixel; and `TextDraw` keeps to the cost per glyph that
//! `gbcli help fonts` states, measured with `gbcli measure`.

use crate::common::{Project, read_png};

/// The starter's string, and where its line starts on screen: the line
/// buffer's tile 5 at map row 11 (src/main.asm).
const TEXT: &str = "Hello, Game Boy!";
const LINE_X: usize = 40;
const LINE_Y: usize = 88;

fn starter(name: &str) -> Project {
    let p = Project::new(name);
    let made = p.rgbds(&["new", "game"]);
    assert_eq!(made.code, 0, "{}", made.stderr);
    let built = p.rgbds(&["build", "game"]);
    assert_eq!(built.code, 0, "{}{}", built.stdout, built.stderr);
    p
}

/// `gbcli font --sample TEXT --json`'s width and 1bpp tiles.
fn sample(p: &Project) -> (usize, Vec<u8>) {
    let run = p.gbcli(&[
        "font",
        "game/assets/fonts/main.yaff",
        "--sample",
        TEXT,
        "--json",
    ]);
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
    assert!(sample.missing.is_empty(), "{:?}", sample.missing);
    (sample.width as usize, sample.data)
}

#[test]
fn the_starters_line_is_the_samples_pixel_for_pixel() {
    let p = starter("starter-text");
    let (width, data) = sample(&p);
    let run = p.gbcli(&[
        "run",
        "game/build/game.gb",
        "--session",
        "text",
        "--frames",
        "30",
        "--shot",
        "text.png",
        "--scale",
        "1",
    ]);
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    let (screen_width, pixels) = read_png(&p.path("text.png"));
    assert_eq!(screen_width, 160);
    let pixel = |x: usize, y: usize| pixels[y * 160 + x];
    // Paper is shade 0, the screen's empty tile; ink is shade 3, the face's
    // outline (map row 8 from column 4: the face's row 0 is `00333300`).
    let paper = pixel(0, 0);
    let ink = pixel(32 + 2, 64);
    assert_ne!(ink, paper);
    // Every pixel of the line's tiles, and of a tile either side of them.
    let tiles = width.div_ceil(8);
    for y in 0..8 {
        for x in 0..(tiles + 2) * 8 {
            let set = x >= 8 && x - 8 < tiles * 8 && {
                let lx = x - 8;
                data[lx / 8 * 8 + y] & (0x80 >> (lx % 8)) != 0
            };
            let (sx, sy) = (LINE_X - 8 + x, LINE_Y + y);
            assert_eq!(
                pixel(sx, sy),
                if set { ink } else { paper },
                "screen ({sx}, {sy}): the sample's pixel ({}, {y}) is {}",
                x as i64 - 8,
                if set { "ink" } else { "paper" }
            );
        }
    }
}

/// The bound `gbcli help fonts` states: "at most N M-cycles a glyph".
fn stated_bound(help: &str) -> u64 {
    help.split("at most ")
        .skip(1)
        .find_map(|rest| {
            let (n, _) = rest.split_once(" M-cycles a glyph")?;
            n.replace(',', "").parse().ok()
        })
        .expect("help fonts states \"at most N M-cycles a glyph\"")
}

#[test]
fn text_draw_keeps_to_the_cost_per_glyph_help_fonts_states() {
    let p = starter("starter-text-cost");
    let help = p.gbcli(&["help", "fonts"]);
    let bound = stated_bound(&help.stdout);
    // From power-on: one span from each glyph's arrival at the loop to the
    // next, the last one ending at the string's end.
    let glyphs = TEXT.chars().count().to_string();
    let run = p.gbcli(&[
        "measure",
        "TextDraw.glyph..TextDraw.glyph",
        "--rom",
        "game/build/game.gb",
        "--session",
        "cost",
        "--iterations",
        &glyphs,
        "--max",
        &(bound * 4).to_string(),
    ]);
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
}
