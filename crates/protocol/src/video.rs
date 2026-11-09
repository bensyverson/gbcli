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
    /// BGP, OBP0 and OBP1, and on a CGB its 8 background and 8 object
    /// palettes from palette RAM.
    Palettes,
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
    /// The integer scale when present; otherwise the view's default: 4 for the
    /// VRAM views, 1 for a run's audio images.
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
    /// The DMG palette registers, then palette RAM where the model has it
    /// (a CGB, in CGB mode or running a DMG cartridge); none on a DMG.
    Palettes {
        bgp: DmgPaletteView,
        obp0: DmgPaletteView,
        obp1: DmgPaletteView,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        bg: Vec<CgbPaletteView>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        obj: Vec<CgbPaletteView>,
    },
}

/// A DMG palette register: the byte, and the shade (0 lightest to 3) it
/// gives color indices 0 to 3.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DmgPaletteView {
    pub value: u8,
    pub shades: [u8; 4],
}

impl DmgPaletteView {
    pub fn of(value: u8) -> DmgPaletteView {
        DmgPaletteView {
            value,
            shades: std::array::from_fn(|i| (value >> (2 * i)) & 3),
        }
    }
}

/// One CGB palette from palette RAM: colors 0 to 3.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CgbPaletteView {
    /// 0–7.
    pub index: u8,
    pub colors: Vec<CgbColorView>,
}

/// One palette RAM color.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CgbColorView {
    /// As stored: bits 0–4 red, 5–9 green, 10–14 blue.
    pub rgb555: u16,
    /// `#RRGGBB`, each 5-bit channel widened to 8 bits as `v << 3 | v >> 2`
    /// (no color correction: what an asset's color was, not what a screen shows).
    pub hex: String,
}

impl CgbColorView {
    pub fn of(rgb555: u16) -> CgbColorView {
        let wide = |shift: u16| {
            let v = ((rgb555 >> shift) & 0x1F) as u8;
            v << 3 | v >> 2
        };
        CgbColorView {
            rgb555,
            hex: format!("#{:02X}{:02X}{:02X}", wide(0), wide(5), wide(10)),
        }
    }
}

/// `run --oam-log`: OAM at every vblank the run passed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OamLogMade {
    /// What each frame's `frame` counts from.
    pub since: crate::stop::TimeBase,
    pub frames: Vec<OamFrameView>,
}

/// OAM as the frame that just ended drew it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OamFrameView {
    /// The frame's number from the time base, as a stop's `frame_shown`.
    pub frame: i64,
    /// The LCD was off, so nothing was drawn: the objects are what OAM held.
    #[serde(default, skip_serializing_if = "crate::request::is_false")]
    pub lcd_off: bool,
    /// 8 or 16 (LCDC bit 2).
    pub height: u8,
    /// The objects on screen, in OAM order; the others are left out.
    pub objects: Vec<OamView>,
}

impl OamFrameView {
    /// The OAM indices of the objects the 10-per-line limit dropped.
    pub fn dropped(&self) -> Vec<u8> {
        self.objects
            .iter()
            .filter(|o| o.dropped_by_line_limit)
            .map(|o| o.index)
            .collect()
    }
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

    #[test]
    fn palettes_is_a_view() {
        let line = r#"{"id":"1","op":"view","args":{"what":"palettes"}}"#;
        let request = parse_request(line).expect("parses");
        let Op::View(args) = &request.op else {
            panic!("{request:?}")
        };
        assert_eq!(args.what, ViewKind::Palettes);
    }

    #[test]
    fn a_dmg_register_gives_each_index_its_shade() {
        let bgp = DmgPaletteView::of(0b11_10_01_00);
        assert_eq!(bgp.shades, [0, 1, 2, 3]);
        assert_eq!(DmgPaletteView::of(0xFC).shades, [0, 3, 3, 3]);
    }

    #[test]
    fn a_color_widens_each_channel_to_eight_bits() {
        assert_eq!(CgbColorView::of(0x7FFF).hex, "#FFFFFF");
        assert_eq!(CgbColorView::of(0x0000).hex, "#000000");
        // red 31, green 16, blue 0
        assert_eq!(CgbColorView::of(0x021F).hex, "#FF8400");
    }
}
