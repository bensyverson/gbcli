use gbcli_pixels::{decode_png, encode_png};

use super::*;
use crate::model::{Glyph, Property};
use crate::yaff;

fn glyph(key: Key, width: u8, rows: [u8; HEIGHT]) -> Glyph {
    Glyph {
        comment: vec![],
        key,
        width,
        rows,
    }
}

/// 20 glyphs, so two rows of cells: a space of paper alone, widths 1 to
/// 8, a named glyph and a mark.
fn font() -> Font {
    let mut glyphs = vec![
        glyph(Key::Char(' '), 3, [0; 8]),
        glyph(Key::Char('!'), 2, [0x80, 0x80, 0x80, 0x80, 0, 0x80, 0, 0]),
        glyph(
            Key::Name("heart".into()),
            7,
            [0x6C, 0xFE, 0xFE, 0x7C, 0x38, 0x10, 0, 0],
        ),
        glyph(Key::Char('\u{301}'), 4, [0x20, 0x40, 0, 0, 0, 0, 0, 0]),
        glyph(Key::Char('<'), 4, [0, 0x20, 0x40, 0x80, 0x40, 0x20, 0, 0]),
        glyph(Key::Char('a'), 8, [0xFF; 8]),
    ];
    for (i, c) in ('b'..='o').enumerate() {
        let w = (i % 8) as u8 + 1;
        let row = (0xFF00u16 >> w) as u8 & 0xAA;
        glyphs.push(glyph(Key::Char(c), w, [row, 0, row, 0, row, 0, row, 0x80]));
    }
    Font {
        comment: vec!["dropped by a grid".into()],
        properties: vec![Property {
            comment: vec![],
            key: "ascent".into(),
            value: "6".into(),
        }],
        glyphs,
        tail: vec![],
    }
}

fn pixel(image: &Raster, x: u32, y: u32) -> Rgba {
    image.rgba[(y * image.width + x) as usize]
}

fn set(image: &mut Raster, x: u32, y: u32, p: Rgba) {
    let w = image.width;
    image.rgba[(y * w + x) as usize] = p;
}

#[test]
fn a_font_round_trips_through_a_guided_grid_and_its_png() {
    let font = font();
    let grid = to_grid(&font, true);
    assert_eq!((grid.image.width, grid.image.height), (128, 16));
    assert_eq!(grid.chars.lines().count(), 2);
    let png = encode_png(&grid.image);
    let back = from_grid(&decode_png(&png).unwrap(), &grid.chars).unwrap();
    assert_eq!(back, font.glyphs);
}

#[test]
fn guides_are_half_transparent_and_only_where_no_glyph_draws() {
    let grid = to_grid(&font(), true);
    let img = &grid.image;
    // The space (cell 0,0) is 3 columns of paper; column 3 on is clear.
    assert_eq!(pixel(img, 0, 0), PAPER);
    assert_eq!(pixel(img, 2, 7), PAPER);
    // Cell 0 is untinted above the baseline (ascent 6), tinted below it.
    assert_eq!(pixel(img, 3, 0)[3], 0);
    assert_eq!(pixel(img, 3, 6), BASELINE_GUIDE);
    // Cell 1 (every other cell) is tinted past its width.
    assert_eq!(pixel(img, 8 + 2, 0), CELL_GUIDE);
    // A full-width glyph has no guide pixels at all.
    assert!((40..48).all(|x| (0..8).all(|y| pixel(img, x, y)[3] == 255)));
    // Without guides, clear is alpha 0.
    let plain = to_grid(&font(), false);
    assert!(plain.image.rgba.iter().all(|p| p[3] == 0 || p[3] == 255));
    assert_eq!(
        from_grid(&plain.image, &plain.chars).unwrap(),
        font().glyphs
    );
}

#[test]
fn every_disagreement_between_the_chars_and_the_art_names_its_cell() {
    let grid = to_grid(&font(), true);
    let mut img = grid.image.clone();
    // Ink drawn in cell (7,1), past the last character of row 1.
    set(&mut img, 7 * 8, 9, INK);
    // A hole inside '!' (cell 1,0): a clear pixel left of its last column.
    set(&mut img, 8, 7, [0, 0, 0, 0]);
    // A third color in 'a' (cell 5,0).
    set(&mut img, 5 * 8 + 3, 3, [0x80, 0x10, 0x10, 0xFF]);
    let mut chars: Vec<String> = grid.chars.lines().map(str::to_string).collect();
    // Row 1 gains a character over the empty cell 4, and lists 'b' twice.
    chars[1].push_str("pb");
    let problems = from_grid(&img, &chars.join("\n")).unwrap_err();
    let cells: Vec<Option<(u32, u32)>> = problems.iter().map(|p| p.cell).collect();
    for want in [(7, 1), (1, 0), (5, 0), (4, 1), (5, 1)] {
        assert!(cells.contains(&Some(want)), "{want:?} in {problems:#?}");
    }
    let said = |cell| {
        problems
            .iter()
            .find(|p| p.cell == Some(cell))
            .map(|p| p.message.clone())
            .unwrap()
    };
    assert!(said((7, 1)).contains("no character"), "{}", said((7, 1)));
    assert!(said((4, 1)).contains("empty"), "{}", said((4, 1)));
    assert!(said((5, 1)).contains("twice"), "{}", said((5, 1)));
    assert!(said((1, 0)).contains("clear"), "{}", said((1, 0)));
}

#[test]
fn a_line_past_the_grid_and_a_wrong_size_are_problems() {
    let grid = to_grid(&font(), false);
    let long = format!(
        "{}\nqrstuvwxyzQRSTUVWXYZ\nZ",
        grid.chars.lines().next().unwrap()
    );
    let problems = from_grid(&grid.image, &long).unwrap_err();
    assert!(
        problems
            .iter()
            .any(|p| p.line == Some(2) && p.message.contains("16")),
        "{problems:#?}"
    );
    assert!(problems.iter().any(|p| p.line == Some(3)), "{problems:#?}");
    let narrow = Raster {
        width: 120,
        height: 8,
        rgba: vec![[0, 0, 0, 0]; 960],
    };
    let problems = from_grid(&narrow, "").unwrap_err();
    assert!(problems[0].message.contains("128"), "{problems:#?}");
}

#[test]
fn a_new_grid_is_all_guide_and_asks_for_every_cell_to_be_drawn() {
    let keys: Vec<Key> = "ABC".chars().map(Key::Char).collect();
    let grid = new_grid(&keys);
    assert_eq!(grid.chars, "ABC\n");
    assert!(grid.image.rgba.iter().all(|p| p[3] < 255));
    let problems = from_grid(&grid.image, &grid.chars).unwrap_err();
    assert_eq!(problems.len(), 3);
    assert!(problems[0].remedy.contains("<>"), "{}", problems[0].remedy);
    // Drawn as paper alone, A reads as a space-like glyph 8 wide.
    let mut img = grid.image.clone();
    for y in 0..8 {
        for x in 0..8 {
            set(&mut img, x, y, PAPER);
        }
    }
    let glyphs = from_grid(&img, "A<><>").unwrap();
    assert_eq!(glyphs, vec![glyph(Key::Char('A'), 8, [0; 8])]);
}

#[test]
fn the_guide_sheet_is_the_grid_at_four_times_with_a_label_strip() {
    let grid = to_grid(&font(), true);
    let png = guide_sheet(&grid.image, &grid.chars);
    let sheet = decode_png(&png).unwrap();
    assert_eq!(sheet.width, 128 * 4);
    assert_eq!(sheet.height, 2 * (32 + 8));
    assert!(sheet.rgba.iter().all(|p| p[3] == 255));
}

#[test]
fn yaff_survives_a_round_trip_through_the_grid_where_the_grid_holds_it() {
    let text = yaff::write(&font());
    let font = yaff::read(&text).unwrap();
    let grid = to_grid(&font, true);
    let glyphs = from_grid(&grid.image, &grid.chars).unwrap();
    assert_eq!(glyphs, font.glyphs);
}
