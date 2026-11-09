//! `pixels` (project/2026-11-08-pixel-text-design.md): an image, a PNG or
//! pixel text, checked and converted. `gbcli-pixels` reads and writes;
//! `gbcli-build` fills these. Needs no session and no RGBDS.

use serde::{Deserialize, Serialize};

/// `op: "pixels"`. Paths are relative to the server's working directory.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PixelsArgs {
    /// The image: a PNG, or pixel text (`format gbcli-pixels 1`).
    pub file: String,
    /// Write the canonical pixel text here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Lay the text out as tiles and a map, not rows.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tiles: bool,
    /// Write the image at 1:1 as a PNG, the one the build would read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub png: Option<String>,
    /// Write it four times larger with an 8x8 grid, for looking only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    /// The palette file `PALETTE N` and `dmg N` resolve through; default
    /// the `assets/palettes.txt` of the project the image sits in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palettes: Option<String>,
}

/// An image file's form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PixelsForm {
    Png,
    /// Pixel text (`format gbcli-pixels 1`).
    Text,
}

/// What `pixels` wrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PixelsOutputKind {
    Text,
    Png,
    Preview,
}

/// An image read, and what was made from it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PixelsMade {
    /// As given, relative to the working directory.
    pub path: String,
    pub form: PixelsForm,
    pub width: u32,
    pub height: u32,
    /// Opaque colors, by legend entry in pixel text.
    pub colors: u32,
    /// Whether some pixel is clear.
    pub clear: bool,
    /// Frames, when the file name gives a frame size (`hero.16x16.pixels`)
    /// the image is whole frames of.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frames: Option<u32>,
    /// Distinct 8x8 tiles (no flips), when the size is whole tiles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tiles: Option<u32>,
    pub wrote: Vec<PixelsOutput>,
}

/// A file `pixels` wrote.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PixelsOutput {
    pub kind: PixelsOutputKind,
    pub path: String,
    pub bytes: u64,
}

/// Why an image didn't load: in pixel text (or the palette file), a line
/// and column; in a PNG, the first pixel at fault, when one is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PixelsProblem {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<u32>,
    pub message: String,
    pub remedy: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Op, parse_request};

    #[test]
    fn a_pixels_request_parses_and_needs_no_session() {
        let line = r#"{"id":"1","op":"pixels","args":{"file":"hero.png","text":"hero.pixels","tiles":true}}"#;
        let request = parse_request(line).expect("parses");
        let op = Op::Pixels(PixelsArgs {
            file: "hero.png".into(),
            text: Some("hero.pixels".into()),
            tiles: true,
            ..PixelsArgs::default()
        });
        assert_eq!(request.op, op);
        assert_eq!(op.name(), "pixels");
        assert!(!op.needs_bundle() && !op.changes_session() && !op.mutates_head());
        let bare = r#"{"id":"1","op":"pixels","args":{"file":"a.png","grid":true}}"#;
        assert!(parse_request(bare).is_err(), "unknown fields are refused");
        let args = PixelsArgs {
            file: "a.png".into(),
            ..PixelsArgs::default()
        };
        assert_eq!(serde_json::to_string(&args).unwrap(), r#"{"file":"a.png"}"#);
    }

    #[test]
    fn a_text_problem_carries_its_line_and_column_and_no_pixel() {
        let p = PixelsProblem {
            path: "hero.pixels".into(),
            line: Some(13),
            column: Some(9),
            x: None,
            y: None,
            message: "x isn't in the legend: it has ., k, s and r".into(),
            remedy: "add `color x VALUE`, or use a legend character".into(),
        };
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(
            json,
            r#"{"path":"hero.pixels","line":13,"column":9,"message":"x isn't in the legend: it has ., k, s and r","remedy":"add `color x VALUE`, or use a legend character"}"#
        );
        assert_eq!(serde_json::from_str::<PixelsProblem>(&json).unwrap(), p);
    }
}
