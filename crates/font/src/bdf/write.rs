//! BDF out: every glyph a full-height box at the pen, as wide as it
//! advances, so an editor shows it in its cell; the font's ascent (its
//! `ascent` property, else row 7's top) as `FONT_ASCENT`; its other
//! properties, in its order, as string properties named in capitals with
//! `_` for `-` (a `.` kept), which an import reads back under their own
//! names.

use crate::key::Key;
use crate::model::{Font, HEIGHT, Property};

/// `font` as BDF, its `FONT` named `name`.
pub fn write_bdf(font: &Font, name: &str) -> String {
    let ascent = u32::from(font.ascent());
    let descent = HEIGHT as u32 - ascent;
    let mut out = String::from("STARTFONT 2.1\n");
    for line in &font.comment {
        out += &format!("COMMENT {line}\n").replace("COMMENT \n", "COMMENT\n");
    }
    out += &format!("FONT {name}\nSIZE 8 75 75\nFONTBOUNDINGBOX 8 8 0 -{descent}\n");
    let is = |p: &Property, key: &str| p.key.eq_ignore_ascii_case(key);
    let has = |key: &str| font.properties.iter().any(|p| is(p, key));
    let ascent_line = format!("FONT_ASCENT {ascent}");
    let descent_line = format!("FONT_DESCENT {descent}");
    // Every editor wants both metrics: stated first when the font sets
    // neither, else where the font sets them, in its order.
    let mut props = Vec::new();
    if !has("ascent") {
        props.push(ascent_line.clone());
        if !has("descent") {
            props.push(descent_line.clone());
        }
    }
    for p in &font.properties {
        if is(p, "ascent") {
            props.push(ascent_line.clone());
            if !has("descent") {
                props.push(descent_line.clone());
            }
        } else if is(p, "descent") {
            props.push(descent_line.clone());
        } else {
            let key = p.key.to_ascii_uppercase().replace('-', "_");
            let value = p.value.replace('\n', " ").replace('"', "\"\"");
            props.push(format!("{key} \"{value}\""));
        }
    }
    props.push("CHARSET_REGISTRY \"ISO10646\"".into());
    props.push("CHARSET_ENCODING \"1\"".into());
    out += &format!("STARTPROPERTIES {}\n", props.len());
    for line in props {
        out += &line;
        out.push('\n');
    }
    out += &format!("ENDPROPERTIES\nCHARS {}\n", font.glyphs.len());
    for g in &font.glyphs {
        let (start, encoding) = match &g.key {
            Key::Char(c) => {
                let cp = u32::from(*c);
                (format!("uni{cp:04X}"), cp.to_string())
            }
            Key::Name(n) => (n.clone(), "-1".into()),
        };
        let w = u32::from(g.width);
        out += &format!(
            "STARTCHAR {start}\nENCODING {encoding}\nSWIDTH {} 0\nDWIDTH {w} 0\nBBX {w} 8 0 \
             -{descent}\nBITMAP\n",
            w * 125
        );
        for row in g.rows {
            out += &format!("{row:02X}\n");
        }
        out += "ENDCHAR\n";
    }
    out += "ENDFONT\n";
    out
}
