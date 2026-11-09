//! Rows for `font`: the problem that stopped it, or the font read, its
//! sample and its preview; and for the authoring verbs, the files written
//! and removed, every problem in a working copy, or a diff.

use gbcli_protocol::{FontFileKind, GlyphChangeKind, ResultBody};

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
    for p in &r.font_problems {
        out += &Row::new("problem")
            .field("path", &p.path)
            .opt("cell", p.cell.map(|c| format!("{},{}", c.x, c.y)))
            .opt("line", p.line)
            .opt("column", p.column)
            .opt("glyph", p.glyph.as_deref())
            .field("message", &p.message)
            .field("remedy", &p.remedy)
            .line();
    }
    if let Some(w) = &r.font_written {
        for f in &w.files {
            let kind = match f.kind {
                FontFileKind::Yaff => "yaff",
                FontFileKind::Png => "png",
                FontFileKind::Chars => "chars",
                FontFileKind::Bdf => "bdf",
                FontFileKind::GuideSheet => "guide_sheet",
                FontFileKind::Base => "base",
            };
            out += &Row::new("wrote")
                .field("kind", kind)
                .field("path", &f.path)
                .field("bytes", f.bytes)
                .line();
        }
        for path in &w.removed {
            out += &Row::new("removed").field("path", path).line();
        }
        if !w.missing.is_empty() {
            out += &Row::new("missing")
                .field("count", w.missing.len())
                .field("chars", w.missing.concat())
                .line();
        }
        if !w.skipped.is_empty() {
            out += &Row::new("skipped")
                .field("count", w.skipped.len())
                .field("names", w.skipped.join(","))
                .line();
        }
        out += &Row::new("font")
            .field("path", &w.font)
            .field("glyphs", w.glyphs)
            .line();
    }
    if let Some(d) = &r.font_diff {
        for g in &d.glyphs {
            let change = match g.change {
                GlyphChangeKind::Changed => "changed",
                GlyphChangeKind::Added => "added",
                GlyphChangeKind::Removed => "removed",
            };
            out += &Row::new("glyph")
                .field("key", &g.key)
                .field("change", change)
                .opt("width_a", g.width_a)
                .opt("width_b", g.width_b)
                .line();
        }
        let count = |k: GlyphChangeKind| d.glyphs.iter().filter(|g| g.change == k).count();
        out += &Row::new("diff")
            .field("a", &d.a)
            .field("b", &d.b)
            .field("changed", count(GlyphChangeKind::Changed))
            .field("added", count(GlyphChangeKind::Added))
            .field("removed", count(GlyphChangeKind::Removed))
            .flag("reordered", d.reordered)
            .line();
        if let Some(p) = &d.png {
            out += &Row::new("preview")
                .field("path", &p.path)
                .field("bytes", p.bytes)
                .line();
        }
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
