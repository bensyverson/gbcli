//! Two fonts compared glyph by glyph, by key (`gbcli font diff A B`), and
//! the changed glyphs drawn before and after, enlarged, for review beside
//! the `.yaff` diff.

use gbcli_pixels::{Raster, Rgba, encode_png};

use crate::key::Key;
use crate::labels::{self, ADVANCE};
use crate::model::{Font, Glyph};

/// One glyph that differs: changed (both sides), added or removed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub key: Key,
    pub before: Option<Glyph>,
    pub after: Option<Glyph>,
}

/// What differs between two fonts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FontDiff {
    /// Changed and removed glyphs in the first font's order, then added
    /// glyphs in the second's.
    pub changes: Vec<Change>,
    /// Whether the glyphs both fonts hold are in a different order, which
    /// renumbers them.
    pub reordered: bool,
}

/// What differs from `a` to `b`. A glyph's comment is not a difference.
pub fn diff(a: &Font, b: &Font) -> FontDiff {
    let same = |x: &Glyph, y: &Glyph| x.width == y.width && x.rows == y.rows;
    let mut changes = Vec::new();
    for g in &a.glyphs {
        match b.find(&g.key).map(|i| &b.glyphs[i]) {
            Some(h) if same(g, h) => {}
            after => changes.push(Change {
                key: g.key.clone(),
                before: Some(g.clone()),
                after: after.cloned(),
            }),
        }
    }
    for h in &b.glyphs {
        if a.find(&h.key).is_none() {
            changes.push(Change {
                key: h.key.clone(),
                before: None,
                after: Some(h.clone()),
            });
        }
    }
    let common = |x: &Font, y: &Font| -> Vec<Key> {
        x.glyphs
            .iter()
            .filter(|g| y.find(&g.key).is_some())
            .map(|g| g.key.clone())
            .collect()
    };
    FontDiff {
        changes,
        reordered: common(a, b) != common(b, a),
    }
}

const SCALE: u32 = 4;
const CELL: u32 = 8 * SCALE;
const LABEL: u32 = 8 * ADVANCE + 4;
const GUTTER: u32 = 4;
const BACK: Rgba = [0xE8, 0xE8, 0xE8, 0xFF];
const PAPER: Rgba = [0xFF, 0xFF, 0xFF, 0xFF];
const INK: Rgba = [0x00, 0x00, 0x00, 0xFF];
const PAST: Rgba = [0xF0, 0xC8, 0xF0, 0xFF];
const NONE: Rgba = [0x30, 0x30, 0x60, 0xFF];
const LABEL_INK: Rgba = [0x20, 0x20, 0x40, 0xFF];

/// `diff`'s changes drawn at 4x, a row each: the key's label, the glyph
/// before and the glyph after (dark where a side has none), each cell's
/// columns past the glyph's width tinted. PNG bytes; a diff with no
/// changes draws one empty row.
pub fn diff_png(diff: &FontDiff) -> Vec<u8> {
    let width = LABEL + 2 * CELL + GUTTER;
    let rows = diff.changes.len().max(1) as u32;
    let height = rows * (CELL + GUTTER);
    let mut r = Raster {
        width,
        height,
        rgba: vec![BACK; (width * height) as usize],
    };
    for (n, change) in diff.changes.iter().enumerate() {
        let top = n as u32 * (CELL + GUTTER);
        labels::draw(&labels::label(&change.key), 8, |x, y| {
            r.rgba[((top + 2 + y) * width + 2 + x) as usize] = LABEL_INK;
        });
        for (side, glyph) in [&change.before, &change.after].into_iter().enumerate() {
            let left = LABEL + side as u32 * (CELL + GUTTER);
            for y in 0..CELL {
                for x in 0..CELL {
                    let (column, row) = ((x / SCALE) as u8, (y / SCALE) as usize);
                    let p = match glyph {
                        None => NONE,
                        Some(g) if column >= g.width => PAST,
                        Some(g) if g.ink(column, row) => INK,
                        Some(_) => PAPER,
                    };
                    r.rgba[((top + y) * width + left + x) as usize] = p;
                }
            }
        }
    }
    encode_png(&r)
}

#[cfg(test)]
mod tests {
    use gbcli_pixels::decode_png;

    use super::*;
    use crate::model::HEIGHT;

    fn glyph(c: char, width: u8, row: u8) -> Glyph {
        Glyph {
            comment: vec![],
            key: Key::Char(c),
            width,
            rows: [row; HEIGHT],
        }
    }

    fn font(glyphs: Vec<Glyph>) -> Font {
        Font {
            glyphs,
            ..Font::default()
        }
    }

    #[test]
    fn glyphs_are_compared_by_key_ignoring_comments() {
        let mut commented = glyph('a', 4, 0x80);
        commented.comment = vec!["only a comment".into()];
        let a = font(vec![commented, glyph('b', 4, 0x80), glyph('c', 4, 0x80)]);
        let b = font(vec![
            glyph('a', 4, 0x80),
            glyph('b', 5, 0x80),
            glyph('d', 3, 0x40),
        ]);
        let d = diff(&a, &b);
        let keys: Vec<(char, bool, bool)> = d
            .changes
            .iter()
            .map(|c| match c.key {
                Key::Char(k) => (k, c.before.is_some(), c.after.is_some()),
                Key::Name(_) => unreachable!(),
            })
            .collect();
        assert_eq!(
            keys,
            vec![('b', true, true), ('c', true, false), ('d', false, true)]
        );
        assert!(!d.reordered);
        assert_eq!(diff(&a, &a), FontDiff::default());
    }

    #[test]
    fn a_new_order_is_reported_since_it_renumbers_glyphs() {
        let a = font(vec![glyph('a', 4, 0x80), glyph('b', 4, 0x80)]);
        let b = font(vec![glyph('b', 4, 0x80), glyph('a', 4, 0x80)]);
        let d = diff(&a, &b);
        assert!(d.changes.is_empty());
        assert!(d.reordered);
    }

    #[test]
    fn the_picture_has_a_row_a_change() {
        let a = font(vec![glyph('a', 4, 0x80), glyph('b', 4, 0x80)]);
        let b = font(vec![glyph('a', 5, 0x80)]);
        let png = diff_png(&diff(&a, &b));
        let r = decode_png(&png).unwrap();
        assert_eq!(r.height, 2 * 36);
        assert!(r.width >= 2 * 32);
    }
}
