//! The canonical writer: rows by frame or band, tiles and a map, and the
//! author's legend kept.

use gbcli_pixels::{EXAMPLE, Layout, Palettes, Raster, from_raster, parse_text, to_text};

use super::parse;

const HERO_HEAD: &str = "\
format gbcli-pixels 1
size 32 16
color . clear
color k #201018
color s #f0c090
color r #d03030
";

fn example_rows(lines: std::ops::Range<usize>) -> String {
    let all: Vec<&str> = EXAMPLE.lines().collect();
    all[lines].iter().map(|l| format!("{l}\n")).collect()
}

#[test]
fn a_sheet_is_written_a_block_a_frame_in_the_authors_legend() {
    let text = to_text(
        &parse(EXAMPLE),
        Layout::Rows {
            frame: Some((16, 16)),
        },
    )
    .unwrap();
    let want = format!(
        "{HERO_HEAD}\nrows 0 0 # frame 0\n{}\nrows 16 0 # frame 1\n{}",
        example_rows(10..26),
        example_rows(28..44)
    );
    assert_eq!(text, want);
}

#[test]
fn with_no_frame_size_the_rows_are_written_in_8_row_bands() {
    let text = to_text(&parse(EXAMPLE), Layout::Rows { frame: None }).unwrap();
    let headers: Vec<&str> = text.lines().filter(|l| l.starts_with("rows")).collect();
    assert_eq!(headers, ["rows 0 0", "rows 0 8"]);
    assert!(text.contains("rows 0 0\n................................\n"));
    // A frame size the image isn't whole frames of falls back to bands.
    let odd = to_text(
        &parse(EXAMPLE),
        Layout::Rows {
            frame: Some((24, 16)),
        },
    )
    .unwrap();
    assert_eq!(odd, text);
}

const MEADOW: &str = "\
format gbcli-pixels 1
size 32 16
color 0 dmg 0   # cloud
color 1 dmg 1   # sky
color 2 dmg 2   # grass
color 9 #123456 # never drawn

tile sk
11111111
11111111
11111111
11111111
11111111
11111111
11111111
11111111

tile cl
11111111
11100111
11000011
10000001
00000000
10000001
11111111
11111111

tile gt
11111111
11111111
11111111
21112111
22122112
22222222
22222222
22222222

map 0 0
sk cl sk cl
gt gt gt gt
";

#[test]
fn tiles_are_keyed_in_hex_by_first_use_and_the_map_reads_as_tile_numbers() {
    let text = to_text(&parse(MEADOW), Layout::Tiles).unwrap();
    let headers: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("tile") || l.starts_with("map"))
        .collect();
    assert_eq!(headers, ["tile 00", "tile 01", "tile 02", "map 0 0"]);
    assert!(
        text.ends_with("map 0 0\n00 01 00 01\n02 02 02 02\n"),
        "{text}"
    );
    // The legend keeps its references unresolved, and drops what nothing draws.
    assert!(text.starts_with(
        "format gbcli-pixels 1\nsize 32 16\ncolor 0 dmg 0\ncolor 1 dmg 1\ncolor 2 dmg 2\n\ntile 00\n"
    ));
    let grays = Palettes {
        dmg: Some([[0xFF; 3], [0xAA; 3], [0x55; 3], [0x00; 3]]),
        named: Vec::new(),
    };
    assert_eq!(
        parse(&text).raster(&grays).unwrap(),
        parse(MEADOW).raster(&grays).unwrap()
    );
}

#[test]
fn fill_keep_and_overlaps_are_flattened() {
    let text = "\
format gbcli-pixels 1
size 3 1
color a #ff0000
color - keep
color b #0000ff
fill a

rows 1 0
-b
";
    let canonical = to_text(&parse(text), Layout::Rows { frame: None }).unwrap();
    assert_eq!(
        canonical,
        "format gbcli-pixels 1\nsize 3 1\ncolor a #ff0000\ncolor b #0000ff\n\nrows 0 0\naab\n"
    );
}

#[test]
fn canonical_text_is_a_fixed_point() {
    for layout in [
        Layout::Rows {
            frame: Some((16, 16)),
        },
        Layout::Tiles,
    ] {
        let once = to_text(&parse(EXAMPLE), layout).unwrap();
        let twice = to_text(&parse(&once), layout).unwrap();
        assert_eq!(once, twice, "{layout:?}");
    }
}

#[test]
fn tiles_need_a_size_of_whole_tiles() {
    let text = "format gbcli-pixels 1\nsize 10 8\ncolor x #000\nfill x\n";
    let e = to_text(&parse(text), Layout::Tiles).expect_err("10 isn't whole tiles");
    assert_eq!(
        e.message,
        "the image is 10x8, which isn't whole 8x8 tiles, so it can't be written as tiles"
    );
    assert_eq!(e.remedy, "write it as rows: drop --tiles");
}

#[test]
fn past_256_tiles_the_keys_grow_a_digit() {
    // 17x16 tiles, each drawing its own number in its pixels.
    let (w, h) = (17 * 8, 16 * 8);
    let mut rgba = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let n = (y / 8) * 17 + x / 8;
            let bit = (y % 8) * 8 + x % 8;
            let on = bit < 16 && (n >> bit) & 1 == 1;
            rgba.push(if on { [0, 0, 0, 255] } else { [255; 4] });
        }
    }
    let raster = Raster {
        width: w,
        height: h,
        rgba,
    };
    let picture = from_raster(&raster).unwrap();
    assert_eq!(picture.tiles(), Some(272));
    let text = to_text(&picture, Layout::Tiles).unwrap();
    let map = text.split("map 0 0\n").nth(1).unwrap();
    assert!(map.starts_with("000 001 002"), "{}", &map[..40]);
    assert!(text.contains("tile 10f\n"));
    let back = parse_text(&text).unwrap();
    assert_eq!(back.raster(&Palettes::default()).unwrap(), raster);
}
