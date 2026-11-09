//! Font authoring (project/2026-11-09-pixel-fonts-design.md, Ben's
//! decisions, the second leaf): the ops behind `gbcli font edit`, `save`,
//! `import`, `export`, `new` and `diff`. The `.yaff` file is the one
//! source; a PNG grid with its `.chars` sidecar, or a BDF file, is a
//! working copy for an editing tool. `gbcli-build` answers these; none
//! needs a session. Paths are relative to the server's working directory.

use serde::{Deserialize, Serialize};

use crate::font::FontPreview;

/// A working copy's form.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FontForm {
    /// A PNG grid of 8x8 cells with a `.chars` sidecar and guides.
    #[default]
    Png,
    /// BDF 2.1, for bitmap font editors.
    Bdf,
}

/// `op: "font_edit"`: write a working copy of `file` and say where.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontEditArgs {
    /// The font, a `.yaff` file.
    pub file: String,
    #[serde(default)]
    pub form: FontForm,
}

/// `op: "font_save"`: convert the working copy back into its `.yaff` and
/// remove it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontSaveArgs {
    /// The `.yaff` file, or the working copy's own path.
    pub file: String,
}

/// `op: "font_import"`: a BDF font as a `.yaff`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontImportArgs {
    /// The BDF file.
    pub file: String,
    /// Only these characters: a named set (`ascii`, `latin1`,
    /// `latin-ext-a`) or a character list file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chars: Option<String>,
    /// The `.yaff` to write; default the BDF's name with `.yaff`, beside it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub out: Option<String>,
}

/// `op: "font_export"`: a font written as a PNG grid (with its `.chars`)
/// or BDF, where the caller says, outside the working-copy cycle.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontExportArgs {
    pub file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub png: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bdf: Option<String>,
    /// Draw guides in the PNG's clear pixels.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub guides: bool,
}

/// `op: "font_new"`: a working copy for a font that doesn't exist yet: an
/// empty, guided grid of the characters asked for.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontNewArgs {
    /// The font to make: a `.yaff` path, or a bare name for
    /// `assets/fonts/NAME.yaff`.
    pub name: String,
    /// A named set (`ascii`, `latin1`, `latin-ext-a`) or a character list
    /// file.
    pub chars: String,
    /// Also write a 4x sheet labeling every cell with its character.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guide_sheet: Option<String>,
}

/// `op: "font_diff"`: two fonts compared glyph by glyph.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontDiffArgs {
    pub a: String,
    pub b: String,
    /// Draw each changed glyph before and after, at 4x.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub png: Option<String>,
}

/// What `font_edit`, `font_save`, `font_import`, `font_export` or
/// `font_new` did.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontWritten {
    /// The `.yaff` file it is about, as given or found.
    pub font: String,
    /// The glyphs written.
    pub glyphs: u32,
    /// Every file written, in order.
    pub files: Vec<FontFile>,
    /// The working copy's files removed (`font_save`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed: Vec<String>,
    /// Characters a character list asked for that the source lacks
    /// (`font_import`): a warning each.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<String>,
    /// Glyphs a BDF had that a gbcli font can't key, by name, skipped
    /// (`font_import`): a warning each.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<String>,
}

/// One file written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontFile {
    pub path: String,
    pub kind: FontFileKind,
    pub bytes: u64,
}

/// What a written file is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontFileKind {
    Yaff,
    Png,
    Chars,
    Bdf,
    GuideSheet,
}

/// What differs between two fonts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontDiffMade {
    pub a: String,
    pub b: String,
    /// Changed and removed glyphs in A's order, then added in B's.
    pub glyphs: Vec<GlyphChange>,
    /// The glyphs both hold are in another order, which renumbers them.
    pub reordered: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub png: Option<FontPreview>,
}

/// One glyph that differs, with its width on each side that has it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlyphChange {
    pub key: String,
    pub change: GlyphChangeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width_a: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width_b: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GlyphChangeKind {
    Changed,
    Added,
    Removed,
}

/// A composed glyph whose mark's ink lands on its base's (a warning, in
/// `ImagesMade.font_overlaps`): the letter is legible only if drawn by
/// hand.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontOverlap {
    /// The font file, as `FontAsset.source` gives it.
    pub source: String,
    /// The composed letter.
    pub key: String,
    pub base: String,
    pub mark: String,
    /// The rows where they overlap, top first.
    pub rows: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Op, parse_request};

    #[test]
    fn authoring_requests_parse_need_no_session_and_refuse_unknown_fields() {
        let cases = [
            (
                r#"{"id":"1","op":"font_edit","args":{"file":"a.yaff","form":"bdf"}}"#,
                Op::FontEdit(FontEditArgs {
                    file: "a.yaff".into(),
                    form: FontForm::Bdf,
                }),
                "font_edit",
            ),
            (
                r#"{"id":"1","op":"font_save","args":{"file":"a.yaff"}}"#,
                Op::FontSave(FontSaveArgs {
                    file: "a.yaff".into(),
                }),
                "font_save",
            ),
            (
                r#"{"id":"1","op":"font_import","args":{"file":"m.bdf","chars":"latin1"}}"#,
                Op::FontImport(FontImportArgs {
                    file: "m.bdf".into(),
                    chars: Some("latin1".into()),
                    out: None,
                }),
                "font_import",
            ),
            (
                r#"{"id":"1","op":"font_export","args":{"file":"a.yaff","png":"a.png","guides":true}}"#,
                Op::FontExport(FontExportArgs {
                    file: "a.yaff".into(),
                    png: Some("a.png".into()),
                    bdf: None,
                    guides: true,
                }),
                "font_export",
            ),
            (
                r#"{"id":"1","op":"font_new","args":{"name":"talk","chars":"ascii"}}"#,
                Op::FontNew(FontNewArgs {
                    name: "talk".into(),
                    chars: "ascii".into(),
                    guide_sheet: None,
                }),
                "font_new",
            ),
            (
                r#"{"id":"1","op":"font_diff","args":{"a":"a.yaff","b":"b.yaff"}}"#,
                Op::FontDiff(FontDiffArgs {
                    a: "a.yaff".into(),
                    b: "b.yaff".into(),
                    png: None,
                }),
                "font_diff",
            ),
        ];
        for (line, op, name) in cases {
            assert_eq!(parse_request(line).expect(line).op, op);
            assert_eq!(op.name(), name);
            assert!(!op.needs_bundle() && !op.changes_session() && !op.mutates_head());
            let bad = line
                .replace("{\"", "{\"extra\":1,\"")
                .replacen("{\"extra\":1,\"", "{\"", 1);
            assert!(parse_request(&bad).is_err(), "{bad}");
        }
        // The form defaults to png.
        let line = r#"{"id":"1","op":"font_edit","args":{"file":"a.yaff"}}"#;
        assert_eq!(
            parse_request(line).unwrap().op,
            Op::FontEdit(FontEditArgs {
                file: "a.yaff".into(),
                form: FontForm::Png,
            })
        );
    }
}
