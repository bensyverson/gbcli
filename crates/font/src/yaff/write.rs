//! The canonical writer: the global comment and a blank line, the
//! properties and a blank line, then each glyph (its comment, its label,
//! 8 rows indented by four spaces) with a blank line between, then the
//! closing comment after a blank line.

use crate::model::{Font, HEIGHT};

const INDENT: &str = "    ";

pub(super) fn write(font: &Font) -> String {
    let mut out = String::new();
    let comment = |out: &mut String, lines: &[String]| {
        for l in lines {
            if l.is_empty() {
                out.push_str("#\n");
            } else {
                out.push_str("# ");
                out.push_str(l);
                out.push('\n');
            }
        }
    };
    comment(&mut out, &font.comment);
    let more = !font.properties.is_empty() || !font.glyphs.is_empty() || !font.tail.is_empty();
    if !font.comment.is_empty() && more {
        out.push('\n');
    }
    for p in &font.properties {
        comment(&mut out, &p.comment);
        // A one-line value ending in `:` would read as a label: write it
        // indented under its key, as a value of several lines is.
        if p.value.contains('\n') || p.value.ends_with(':') {
            out.push_str(&p.key);
            out.push_str(":\n");
            for l in p.value.lines() {
                out.push_str(INDENT);
                out.push_str(l);
                out.push('\n');
            }
        } else {
            out.push_str(&format!("{}: {}\n", p.key, p.value));
        }
    }
    if !font.properties.is_empty() && !font.glyphs.is_empty() {
        out.push('\n');
    }
    for (i, g) in font.glyphs.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        comment(&mut out, &g.comment);
        out.push_str(&g.key.label());
        out.push_str(":\n");
        for row in 0..HEIGHT {
            out.push_str(INDENT);
            out.extend((0..g.width).map(|c| if g.ink(c, row) { '@' } else { '.' }));
            out.push('\n');
        }
    }
    if !font.tail.is_empty() {
        if !out.is_empty() {
            out.push('\n');
        }
        comment(&mut out, &font.tail);
    }
    out
}
