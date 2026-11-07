//! The `view` verb on the wire (design §9, §12): VRAM and OAM as data first,
//! the PNG only when asked for, per "every read has a cheaper sibling".

use serde::{Deserialize, Serialize};

/// What to view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewKind {
    /// Every tile in VRAM.
    Tiles,
    /// One 32×32 tilemap.
    Map,
    /// The 40 OAM entries.
    Oam,
}

/// Which tilemap: as LCDC selects it for the background (the default) or
/// the window, or one by address.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapSel {
    #[default]
    Background,
    Window,
    #[serde(rename = "9800")]
    Map9800,
    #[serde(rename = "9c00")]
    Map9C00,
}

/// A tilemap's address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MapAreaView {
    #[serde(rename = "9800")]
    Map9800,
    #[serde(rename = "9c00")]
    Map9C00,
}

/// How a map's tile IDs pick tiles (LCDC bit 4): unsigned from $8000, or
/// signed from $9000 (the $8800 method).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AddressingView {
    #[serde(rename = "8000")]
    Unsigned8000,
    #[serde(rename = "8800")]
    Signed8800,
}

/// A PNG to write beside the data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageOut {
    pub path: String,
    /// The integer scale; the view's default (4) when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewArgs {
    pub what: ViewKind,
    /// For `map` only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map: Option<MapSel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub png: Option<ImageOut>,
}

/// One tile that isn't blank: eight rows of eight color indices (0–3).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TileView {
    pub bank: u8,
    /// 0–383 in the bank: tile `n` is at $8000 + 16n.
    pub index: u16,
    /// Each row as eight digits, left to right: `"00111100"`.
    pub rows: Vec<String>,
}

/// One OAM entry: the raw bytes, then what they mean now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OamView {
    pub index: u8,
    pub y: u8,
    pub x: u8,
    pub tile: u8,
    pub flags: u8,
    pub screen_x: i16,
    pub screen_y: i16,
    pub on_screen: bool,
    pub dropped_by_line_limit: bool,
}

/// What the `view` verb read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VideoData {
    /// The tiles that aren't all color 0; `blank` counts the rest.
    Tiles { tiles: Vec<TileView>, blank: u32 },
    /// A map. `background` says whether it is the map the background shows.
    /// `ids` is 32 rows of 32 hex tile IDs; `attributes` (CGB only) is the
    /// same for the attribute bytes.
    Map {
        area: MapAreaView,
        addressing: AddressingView,
        background: bool,
        ids: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attributes: Option<Vec<String>>,
    },
    /// The 40 entries in OAM order; `height` is 8 or 16 (LCDC bit 2).
    Oam { height: u8, objects: Vec<OamView> },
}

/// The `view` result: the data, and the PNG when one was asked for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewMade {
    pub data: VideoData,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub png: Option<ImageMade>,
}

/// A PNG that was written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageMade {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub scale: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_request;
    use crate::request::Op;

    #[test]
    fn the_view_verb_names_what_and_takes_a_png_on_request() {
        let line = r#"{"id":"1","op":"view","args":{"what":"map","map":"9c00","png":{"path":"m.png","scale":2}}}"#;
        let request = parse_request(line).expect("parses");
        let Op::View(args) = &request.op else {
            panic!("{request:?}")
        };
        assert_eq!(args.what, ViewKind::Map);
        assert_eq!(args.map, Some(MapSel::Map9C00));
        assert_eq!(args.png.as_ref().map(|p| p.scale), Some(Some(2)));
        assert!(!request.op.mutates_head());
        assert_eq!(serde_json::to_string(&request).unwrap(), line);
        assert!(parse_request(r#"{"id":"1","op":"view","args":{"what":"sprites"}}"#).is_err());
    }
}
