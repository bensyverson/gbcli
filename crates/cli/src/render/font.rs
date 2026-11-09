//! Rows for `font`: the problem that stopped it, or the font read, its
//! sample and its preview.

use gbcli_protocol::ResultBody;

use super::Row;

pub fn rows(r: &ResultBody) -> String {
    let mut out = String::new();
    if let Some(p) = &r.font_problem {
        out += &Row::new("problem")
            .field("path", &p.path)
            .opt("line", p.line)
            .opt("column", p.column)
            .field("message", &p.message)
            .field("remedy", &p.remedy)
            .line();
    }
    if let Some(m) = &r.font {
        let widths = m
            .min_width
            .zip(m.max_width)
            .map(|(lo, hi)| format!("{lo}..{hi}"));
        out += &Row::new("font")
            .field("path", &m.path)
            .field("glyphs", m.glyphs)
            .flag("fixed", m.fixed)
            .opt("widths", widths)
            .line();
        if let Some(s) = &m.sample {
            out += &Row::new("sample")
                .field("width", s.width)
                .field("tiles", s.tiles)
                .opt("fit", s.fit)
                .opt(
                    "missing",
                    (!s.missing.is_empty()).then(|| s.missing.concat()),
                )
                .line();
        }
        if let Some(p) = &m.preview {
            out += &Row::new("preview")
                .field("path", &p.path)
                .field("bytes", p.bytes)
                .line();
        }
    }
    out
}
