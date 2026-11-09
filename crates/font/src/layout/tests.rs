use super::*;
use crate::key::Key;

fn glyph(key: Key, width: u8, rows: [u8; 8]) -> Glyph {
    Glyph {
        comment: vec![],
        key,
        width,
        rows,
    }
}

/// 'A' 5 wide and all ink; 'i' 2 wide, ink in column 0; '<' 3 wide;
/// "heart" 7 wide.
fn font() -> Font {
    Font {
        glyphs: vec![
            glyph(Key::Char('A'), 5, [0xF8; 8]),
            glyph(Key::Char('i'), 2, [0x80; 8]),
            glyph(Key::Char('<'), 3, [0x20; 8]),
            glyph(Key::Name("heart".into()), 7, [0x6C; 8]),
        ],
        ..Font::default()
    }
}

#[test]
fn the_pen_advances_by_each_glyphs_width() {
    let line = set(&font(), "AiA");
    assert_eq!(line.glyphs, [0, 1, 0]);
    assert_eq!((line.width, line.tiles), (12, 2));
    assert!(line.missing.is_empty());
}

#[test]
fn a_glyph_crossing_a_tile_boundary_splits_its_rows_across_both() {
    // A at 0-4, A at 5-9: tile 0 is full, tile 1 holds columns 8-9.
    let line = set(&font(), "AA");
    assert_eq!((line.width, line.tiles), (10, 2));
    assert_eq!(&line.data[..8], &[0xFF; 8]);
    assert_eq!(&line.data[8..], &[0xC0; 8]);
}

#[test]
fn a_glyph_replaces_its_own_columns_and_keeps_the_rest() {
    // Over ink everywhere, a 3-wide glyph with ink in its column 1 only, at
    // x = 6: tile 0 keeps columns 0-5, takes paper at 6 and ink at 7; tile
    // 1 takes paper at its column 0 and keeps 1-7.
    let mut data = vec![0xFF; 16];
    draw(&mut data, 6, &glyph(Key::Char('x'), 3, [0x40; 8]));
    assert_eq!(&data[..8], &[0xFD; 8]);
    assert_eq!(&data[8..], &[0x7F; 8]);
    // Bits right of a glyph's width are never drawn, set or not.
    let mut data = vec![0x00; 8];
    let stray = Glyph {
        rows: [0xFF; 8],
        ..glyph(Key::Char('y'), 2, [0; 8])
    };
    draw(&mut data, 0, &stray);
    assert_eq!(&data[..], &[0xC0; 8]);
}

#[test]
fn a_named_glyph_is_written_in_angle_brackets_longest_match_first() {
    let line = set(&font(), "<heart><i");
    assert_eq!(line.glyphs, [3, 2, 1]);
    assert_eq!(line.width, 7 + 3 + 2);
}

#[test]
fn missing_characters_are_listed_once_and_take_no_room() {
    let line = set(&font(), "AzAz?");
    assert_eq!(line.glyphs, [0, 0]);
    assert_eq!(line.missing, ["z", "?"]);
    assert_eq!(line.width, 10);
    let empty = set(&font(), "");
    assert_eq!((empty.width, empty.tiles, empty.data.len()), (0, 0, 0));
}
