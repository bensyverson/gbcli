//! gbcli's rules over a YAFF document: what makes it a gbcli font. The
//! first rule broken, in file order, is the error.

use std::collections::BTreeMap;

use super::syntax::{Document, GlyphDef};
use crate::error::FontError;
use crate::key::Key;
use crate::model::{Font, Glyph, HEIGHT, MAX_WIDTH, Property};

const SPACING: &str = "spacing is drawn into the glyph in gbcli: draw the space as columns of \
                       `.` inside each glyph's width, and remove the property";

/// The font `doc` is, or the first rule it breaks.
pub(super) fn font(doc: Document) -> Result<Font, FontError> {
    let mut seen: BTreeMap<String, u32> = BTreeMap::new();
    for p in &doc.properties {
        let norm = p.key.to_ascii_lowercase().replace('_', "-");
        if let Some(first) = seen.insert(norm.clone(), p.line) {
            return Err(FontError::new(
                p.line,
                1,
                format!("`{}` is set twice (first at line {first})", p.key),
                "keep one of them",
            ));
        }
        match norm.as_str() {
            "yaff" if !(p.value == "1" || p.value.starts_with("1.")) => {
                return Err(FontError::new(
                    p.line,
                    1,
                    format!("this file is YAFF {}; gbcli reads YAFF 1", p.value),
                    "write it as YAFF 1.0 (`yaff: 1.0`), the version monobit's YAFF.md specifies",
                ));
            }
            "left-bearing" | "right-bearing" | "tracking" if p.value.trim() != "0" => {
                return Err(FontError::new(
                    p.line,
                    1,
                    format!(
                        "`{}: {}` moves every glyph, and gbcli's layout has no bearings",
                        p.key, p.value
                    ),
                    SPACING,
                ));
            }
            _ => {}
        }
    }
    let mut keys: BTreeMap<Key, u32> = BTreeMap::new();
    let mut glyphs = Vec::with_capacity(doc.glyphs.len());
    for def in doc.glyphs {
        let glyph = glyph(def, &mut keys)?;
        glyphs.push(glyph);
    }
    Ok(Font {
        comment: doc.comment,
        properties: doc
            .properties
            .into_iter()
            .map(|p| Property {
                comment: p.comment,
                key: p.key,
                value: p.value,
            })
            .collect(),
        glyphs,
        tail: doc.tail,
    })
}

fn glyph(def: GlyphDef, keys: &mut BTreeMap<Key, u32>) -> Result<Glyph, FontError> {
    let first = def
        .labels
        .first()
        .expect("the grammar gives every glyph a label line");
    let key = Key::from_label(&first.text)
        .map_err(|e| FontError::new(first.line, 1, e.message, e.remedy))?;
    if let Some(second) = def.labels.get(1) {
        return Err(FontError::new(
            second.line,
            1,
            format!(
                "`{}:` is a second label for {key}'s glyph; a gbcli glyph has one key",
                second.text
            ),
            "give the other key a glyph of its own: copy the rows under its own label",
        ));
    }
    if let Some(at) = keys.insert(key.clone(), first.line) {
        return Err(FontError::new(
            first.line,
            1,
            format!("{key} already has a glyph, at line {at}"),
            "remove one of them, or key this one differently",
        ));
    }
    let line = def.rows_line;
    if def.rows.is_empty() {
        return Err(FontError::new(
            line,
            1,
            "`-` is YAFF's empty glyph; a gbcli glyph is 8 rows of 1 to 8 pixels",
            "draw it: a space is 8 rows of `.`, as wide as the pen should move",
        ));
    }
    if def.rows.len() != HEIGHT {
        return Err(FontError::new(
            line,
            1,
            format!(
                "{key}'s glyph has {} rows; a gbcli glyph is 8 rows tall",
                def.rows.len()
            ),
            "draw it in 8 rows: rows of `.` above and below the art",
        ));
    }
    let width = def.rows[0].len();
    if width > usize::from(MAX_WIDTH) {
        return Err(FontError::new(
            line,
            1,
            format!("{key}'s glyph is {width} pixels wide; a gbcli glyph is 1 to 8"),
            "narrow it to 8 pixels or fewer: a glyph is one tile at most",
        ));
    }
    if let Some(p) = def.properties.first() {
        let norm = p.key.to_ascii_lowercase().replace('_', "-");
        let remedy = if norm.contains("kerning") || norm == "kern-to" {
            "gbcli's layout has no kerning: the pen moves by each glyph's width; remove it"
        } else if norm.contains("bearing") || norm == "tracking" || norm == "offset" {
            SPACING
        } else {
            "remove it: a gbcli glyph is its key and its 8 rows"
        };
        return Err(FontError::new(
            p.line,
            1,
            format!(
                "`{}` is a per-glyph property, which a gbcli font doesn't have",
                p.key
            ),
            remedy,
        ));
    }
    let mut rows = [0u8; HEIGHT];
    for (row, text) in rows.iter_mut().zip(&def.rows) {
        for (i, c) in text.chars().enumerate() {
            if c == '@' {
                *row |= 0x80 >> i;
            }
        }
    }
    Ok(Glyph {
        comment: def.comment,
        key,
        width: width as u8,
        rows,
    })
}
