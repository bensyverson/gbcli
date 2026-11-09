//! BDF out: every glyph a full-height box at the pen, as wide as it
//! advances, so an editor shows it in its cell; the font's ascent (its
//! `ascent` property, else row 7's top) as `FONT_ASCENT`; its other
//! properties as string properties named in capitals, which an import
//! reads back under their own names.

use crate::key::Key;
use crate::model::{Font, HEIGHT};

/// `font` as BDF, its `FONT` named `name`.
pub fn write_bdf(font: &Font, name: &str) -> String {
    let ascent = u32::from(font.ascent());
    let descent = HEIGHT as u32 - ascent;
    let mut out = String::from("STARTFONT 2.1\n");
    for line in &font.comment {
        out += &format!("COMMENT {line}\n").replace("COMMENT \n", "COMMENT\n");
    }
    out += &format!("FONT {name}\nSIZE 8 75 75\nFONTBOUNDINGBOX 8 8 0 -{descent}\n");
    let others: Vec<_> = font
        .properties
        .iter()
        .filter(|p| !matches!(p.key.to_ascii_lowercase().as_str(), "ascent" | "descent"))
        .collect();
    out += &format!("STARTPROPERTIES {}\n", others.len() + 4);
    out += &format!("FONT_ASCENT {ascent}\nFONT_DESCENT {descent}\n");
    out += "CHARSET_REGISTRY \"ISO10646\"\nCHARSET_ENCODING \"1\"\n";
    for p in others {
        let key = p.key.to_ascii_uppercase().replace(['-', '.'], "_");
        let value = p.value.replace('\n', " ").replace('"', "\"\"");
        out += &format!("{key} \"{value}\"\n");
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
