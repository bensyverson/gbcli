//! Each font's part of the generated files: in `images.asm` its glyphs
//! (`<Name>FontGlyphs`, 8 bytes each) and widths (`<Name>FontWidths`), in
//! `images.inc` its constants and a charmap macro. The macro, not a
//! charmap, because NEWCHARMAP switches to the map it makes, so a charmap
//! defined on include would re-encode every string of every file that
//! includes it; and because the base differs by use (glyph numbers for a
//! renderer, `$80 + n` for a font loaded as tiles).

use std::fmt::Write;

use super::super::fonts::Emitted;

/// The constants a font defines, with their values and comments.
pub(super) fn constants(f: &Emitted) -> Vec<(String, String, String)> {
    let c = &f.constant;
    let n = f.glyphs.len();
    let numbers = match n {
        0 => "none".to_string(),
        1 => "glyph number 0".to_string(),
        _ => format!("glyph numbers 0-{}", n - 1),
    };
    vec![
        (
            format!("{c}_FONT_GLYPHS"),
            n.to_string(),
            format!("{numbers}, in the font file's order"),
        ),
        (
            format!("{c}_FONT_FIXED"),
            u8::from(f.fixed).to_string(),
            "1 when every glyph is 8 wide".into(),
        ),
        (
            format!("{c}_FONT_NEWLINE"),
            "$FE".into(),
            "the charmap's \"\\n\"; no glyph takes it".into(),
        ),
        (
            format!("{c}_FONT_END"),
            "$FF".into(),
            "ends a string; no glyph takes it".into(),
        ),
    ]
}

/// A charmap key as an rgbasm string's body.
fn escaped(key: &str) -> String {
    let mut out = String::new();
    for c in key.chars() {
        match c {
            '"' | '\\' | '{' | '}' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// The font's section in `images.asm`.
pub(super) fn asm(out: &mut String, f: &Emitted) {
    let label = &f.label;
    let _ = write!(
        out,
        "\nSECTION \"font {label}\", ROMX\n\n\
         ; {}: {} glyph(s), 8 bytes each: 1bpp, row 0 first, bit 7 the leftmost\n\
         ; pixel, a set bit ink. Glyph n is at {label}FontGlyphs + 8 * n.\n\
         {label}FontGlyphs::\n",
        f.source,
        f.glyphs.len()
    );
    for (i, g) in f.glyphs.iter().enumerate() {
        let bytes: Vec<String> = g.rows.iter().map(|b| format!("${b:02X}")).collect();
        let _ = writeln!(out, "\tdb {} ; {i} {}", bytes.join(","), g.key.label());
    }
    let _ = write!(
        out,
        "{label}FontGlyphsEnd::\n\n\
         ; A byte a glyph: its width in pixels, 1-8, how far the pen moves after it.\n\
         {label}FontWidths::\n"
    );
    let widths: Vec<String> = f.glyphs.iter().map(|g| g.width.to_string()).collect();
    for chunk in widths.chunks(16) {
        let _ = writeln!(out, "\tdb {}", chunk.join(", "));
    }
    let _ = writeln!(out, "{label}FontWidthsEnd::");
}

/// The font's constants and charmap macro in `images.inc`.
pub(super) fn inc(out: &mut String, f: &Emitted) {
    let c = &f.constant;
    let width = match f
        .glyphs
        .iter()
        .map(|g| g.width)
        .min()
        .zip(f.glyphs.iter().map(|g| g.width).max())
    {
        Some((lo, hi)) if lo == hi => format!("every glyph {lo} wide"),
        Some((lo, hi)) => format!("{lo}-{hi} wide"),
        None => "no glyphs".into(),
    };
    let _ = write!(out, "\n; {}: a font, {width}.\n", f.source);
    for (name, value, comment) in constants(f) {
        let _ = writeln!(out, "DEF {name} EQU {value} ; {comment}");
    }
    let _ = write!(
        out,
        "; {c}_FONT_CHARMAP name, base defines the charmap `name`, each glyph as base +\n\
         ; its number, and leaves the active charmap as it was. Then: PUSHC name,\n\
         ; db \"text\", {c}_FONT_END, POPC. A named glyph is written <name>.\n\
         MACRO {c}_FONT_CHARMAP\n\
         \tPUSHC\n\
         \tNEWCHARMAP \\1\n"
    );
    for (i, g) in f.glyphs.iter().enumerate() {
        let _ = writeln!(
            out,
            "\tCHARMAP \"{}\", (\\2) + {i}",
            escaped(&g.key.charmap())
        );
    }
    let _ = write!(
        out,
        "\tCHARMAP \"\\n\", {c}_FONT_NEWLINE\n\
         \tPOPC\n\
         ENDM\n"
    );
}
