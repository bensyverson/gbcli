//! Rows for `pixels`: the problem that stopped it, or the image read and
//! each file written.

use gbcli_protocol::{PixelsForm, PixelsOutputKind, ResultBody};

use super::Row;

pub fn rows(r: &ResultBody) -> String {
    let mut out = String::new();
    if let Some(p) = &r.pixels_problem {
        out += &Row::new("problem")
            .field("path", &p.path)
            .opt("line", p.line)
            .opt("column", p.column)
            .opt("x", p.x)
            .opt("y", p.y)
            .field("message", &p.message)
            .field("remedy", &p.remedy)
            .line();
    }
    if let Some(m) = &r.pixels {
        let form = match m.form {
            PixelsForm::Png => "png",
            PixelsForm::Text => "text",
        };
        out += &Row::new("pixels")
            .field("path", &m.path)
            .field("form", form)
            .field("size", format!("{}x{}", m.width, m.height))
            .field("colors", m.colors)
            .flag("clear", m.clear)
            .opt("frames", m.frames)
            .opt("tiles", m.tiles)
            .line();
        for o in &m.wrote {
            let kind = match o.kind {
                PixelsOutputKind::Text => "text",
                PixelsOutputKind::Png => "png",
                PixelsOutputKind::Preview => "preview",
            };
            out += &Row::new(kind)
                .field("path", &o.path)
                .field("bytes", o.bytes)
                .line();
        }
    }
    out
}
