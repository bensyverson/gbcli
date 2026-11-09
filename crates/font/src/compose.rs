//! Composition (Ben's ruling 3): a precomposed letter the font doesn't
//! draw, made from its base and combining mark, both drawn, through the
//! mark table. The mark's pixels are laid over the base's at the base's
//! width; ink of the mark landing on ink of the base is an overlap, which
//! the caller reports as a warning naming the glyph. Only requested
//! characters are composed, and a drawn glyph always wins: the caller asks
//! only for what the font lacks.

use crate::key::Key;
use crate::marks;
use crate::model::{Font, Glyph};

/// A letter composed from two drawn glyphs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Composed {
    /// Keyed by the letter, as wide as its base.
    pub glyph: Glyph,
    pub base: char,
    pub mark: char,
    /// The rows where the mark's ink lands on the base's, top first.
    pub overlap: Vec<usize>,
}

/// `letter` composed in `font`, when the table decomposes it and the font
/// draws both its base and its mark. `None` too when the font draws the
/// letter itself.
pub fn compose(font: &Font, letter: char) -> Option<Composed> {
    if font.find(&Key::Char(letter)).is_some() {
        return None;
    }
    let (base, mark) = marks::decompose(letter)?;
    let b = &font.glyphs[font.find(&Key::Char(base))?];
    let m = &font.glyphs[font.find(&Key::Char(mark))?];
    let keep = (0xFF00u16 >> b.width) as u8;
    let mut rows = b.rows;
    let mut overlap = Vec::new();
    for (r, row) in rows.iter_mut().enumerate() {
        let ink = m.rows[r] & keep;
        if *row & ink != 0 {
            overlap.push(r);
        }
        *row |= ink;
    }
    Some(Composed {
        glyph: Glyph {
            comment: Vec::new(),
            key: Key::Char(letter),
            width: b.width,
            rows,
        },
        base,
        mark,
        overlap,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::HEIGHT;

    fn glyph(key: Key, width: u8, rows: [u8; HEIGHT]) -> Glyph {
        Glyph {
            comment: vec!["kept".into()],
            key,
            width,
            rows,
        }
    }

    fn font() -> Font {
        Font {
            glyphs: vec![
                glyph(Key::Char('e'), 5, [0, 0, 0x70, 0x88, 0xF8, 0x80, 0x70, 0]),
                glyph(
                    Key::Char('E'),
                    5,
                    [0xF8, 0x80, 0xF0, 0x80, 0x80, 0xF8, 0, 0],
                ),
                // The acute: rows 0-1, wider than e so its last column is cut.
                glyph(Key::Char('\u{301}'), 8, [0x30, 0x61, 0, 0, 0, 0, 0, 0]),
                glyph(Key::Char('ü'), 5, [0x50; 8]),
            ],
            ..Font::default()
        }
    }

    #[test]
    fn a_missing_letter_is_its_mark_over_its_base_at_the_bases_width() {
        let c = compose(&font(), 'é').expect("composed");
        assert_eq!(c.glyph.key, Key::Char('é'));
        assert_eq!(c.glyph.width, 5);
        assert_eq!(c.glyph.rows, [0x30, 0x60, 0x70, 0x88, 0xF8, 0x80, 0x70, 0]);
        assert!(c.glyph.comment.is_empty());
        assert_eq!((c.base, c.mark), ('e', '\u{301}'));
        assert!(c.overlap.is_empty());
    }

    #[test]
    fn ink_on_ink_is_an_overlap_by_row() {
        let c = compose(&font(), 'É').expect("composed");
        assert_eq!(c.overlap, vec![0]);
    }

    #[test]
    fn nothing_is_composed_without_both_parts_or_over_a_drawn_letter() {
        assert_eq!(compose(&font(), 'ë'), None, "no diaeresis drawn");
        assert_eq!(compose(&font(), 'ü'), None, "drawn");
        assert_eq!(compose(&font(), 'x'), None, "no decomposition");
        assert_eq!(compose(&font(), 'á'), None, "no base");
    }
}
