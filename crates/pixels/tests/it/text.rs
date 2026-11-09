//! What pixel text paints: blocks in file order, fill, keep, tiles and
//! maps, comments, and the counts an image reports.

use gbcli_pixels::{EXAMPLE, Palettes, Value, frame_size};

use super::{draw, parse, raster};

const GRAYS: Palettes = Palettes {
    dmg: Some([[0xFF; 3], [0xAA; 3], [0x55; 3], [0x00; 3]]),
    named: Vec::new(),
};

#[test]
fn the_example_is_a_two_frame_sheet_of_three_colors_and_clear() {
    let p = parse(EXAMPLE);
    assert_eq!((p.width, p.height), (32, 16));
    assert_eq!(p.colors(), 3);
    assert!(p.has_clear());
    // The frames' top halves are alike, so they share two tiles; their
    // bottom halves differ.
    assert_eq!(p.tiles(), Some(6));
    assert_eq!(p.at(5, 1).symbol, 'k');
    assert_eq!(p.at(5, 1).value, Value::Color([0x20, 0x10, 0x18]));
    assert_eq!(p.at(0, 0).value, Value::Clear);
    assert_eq!(frame_size("hero.16x16.pixels"), Some((16, 16)));
}

#[test]
fn blocks_paint_in_file_order_over_fill_and_keep_leaves_what_was_there() {
    let text = "\
format gbcli-pixels 1
size 4 2
color a #ff0000
color b #00f       # #rgb, as in palettes.txt
color - keep
fill a

rows 1 0
bb
  bb  # indented, with a comment

rows 2 0
-b
";
    let r = raster(text, &Palettes::default());
    assert_eq!(
        draw(&r),
        "ff0000 0000ff 0000ff 0000ff\nff0000 0000ff 0000ff ff0000\n"
    );
}

#[test]
fn comments_are_skipped_anywhere_even_inside_a_block() {
    let text = "\
format gbcli-pixels 1 # the format
# a comment line
size 2 3\t#\ttabs
color x #000000 #
\t# indented comment
rows
xx
# between rows
xx
    # indented between rows
xx
";
    let p = parse(text);
    assert_eq!((p.width, p.height, p.colors()), (2, 3, 1));
}

#[test]
fn crlf_lines_and_a_byte_order_mark_read_as_plain_lines() {
    let text = "\u{feff}format gbcli-pixels 1\r\nsize 1 1\r\ncolor x clear\r\nrows\r\nx\r\n";
    let p = parse(text);
    assert!(p.has_clear());
    assert_eq!(p.colors(), 0);
}

#[test]
fn a_map_paints_its_tiles_and_dmg_shades_resolve_through_the_palettes() {
    let text = "\
format gbcli-pixels 1
size 16 8
color 0 dmg 0
color 3 dmg 3

tile w
00000000
00000000
00000000
00000000
00000000
00000000
00000000
00000000

tile k
33333333
33333333
33333333
33333333
33333333
33333333
33333333
33333333

map 0 0
w k
";
    let p = parse(text);
    assert_eq!(p.tiles(), Some(2));
    assert!(p.needs_palettes());
    let r = raster(text, &GRAYS);
    assert_eq!(r.at(7, 3), [0xFF, 0xFF, 0xFF, 0xFF]);
    assert_eq!(r.at(8, 3), [0, 0, 0, 0xFF]);
}

#[test]
fn a_named_palette_entry_is_its_color_and_a_map_may_start_anywhere() {
    let text = "\
format gbcli-pixels 1
size 10 9
color s sea 2
color . clear
fill .

tile t
ssssssss
ssssssss
ssssssss
ssssssss
ssssssss
ssssssss
ssssssss
ssssssss

map 1 1
t
";
    let palettes = Palettes {
        dmg: None,
        named: vec![(
            "sea".into(),
            [[1, 2, 3], [4, 5, 6], [7, 8, 9], [10, 11, 12]],
        )],
    };
    let r = raster(text, &palettes);
    assert_eq!(r.at(0, 0), [0, 0, 0, 0]);
    assert_eq!(r.at(1, 1), [7, 8, 9, 0xFF]);
    assert_eq!(r.at(8, 8), [7, 8, 9, 0xFF]);
    assert_eq!(r.at(9, 8), [0, 0, 0, 0]);
    // A size that isn't whole tiles has no tile count.
    assert_eq!(parse(text).tiles(), None);
}

#[test]
fn a_tile_may_keep_what_the_map_stamps_it_over() {
    let text = "\
format gbcli-pixels 1
size 8 8
color a #111111
color - keep
fill a

tile hole
--------
--------
--------
---aa---
---aa---
--------
--------
--------

map
hole
";
    let r = raster(text, &Palettes::default());
    assert!(r.rgba.iter().all(|&p| p == [0x11, 0x11, 0x11, 0xFF]));
}

#[test]
fn frame_sizes_come_from_the_name_as_the_build_reads_them() {
    assert_eq!(frame_size("rival.24x16.png"), Some((24, 16)));
    assert_eq!(frame_size("hero.8x8.pixels"), Some((8, 8)));
    assert_eq!(frame_size("car.png"), None);
    assert_eq!(frame_size("town.dmg.pixels"), None);
    assert_eq!(frame_size("odd.16x.pixels"), None);
}

#[test]
fn each_pixel_knows_the_line_that_drew_it() {
    let text = "\
format gbcli-pixels 1
size 16 8
color a #000000
color - keep
fill a

tile t
aaaaaaaa
aaaaaaaa
aaaaaaaa
aaaaaaaa
aaaaaaaa
aaaaaaaa
aaaaaaaa
aaaaaaaa

map 8 0
t

rows 0 2
--a
";
    let p = parse(text);
    // fill, then the map row, then the rows block's header; kept pixels
    // stay with what drew them.
    assert_eq!(p.line_at(0, 0), Some(5));
    assert_eq!(p.line_at(8, 0), Some(18));
    assert_eq!(p.line_at(2, 2), Some(20));
    assert_eq!(p.line_at(0, 2), Some(5));
    let png = gbcli_pixels::from_raster(&p.raster(&Palettes::default()).unwrap()).unwrap();
    assert_eq!(png.line_at(0, 0), None, "a PNG has no lines");
}
