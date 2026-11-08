//! Where each source tile went: one [`TileEntry`] per 8x8 tile of a
//! background or tileset, or per 8x16 unit of a sheet, re-exported from
//! `images`.

use std::fmt;

use serde::{Deserialize, Serialize};

/// How the hardware flips an object: OAM attribute bits 5 (X) and 6 (Y).
/// A sheet's units are deduplicated with flips, so a unit drawn mirrored
/// reuses another's tiles; backgrounds never flip (dual mode: the DMG's
/// tilemap has no flip bits).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Flip {
    #[default]
    None,
    X,
    Y,
    /// Both ways: X and Y.
    Xy,
}

impl Flip {
    pub fn is_none(&self) -> bool {
        *self == Flip::None
    }

    /// The OAM attribute bits: $20 flips X, $40 flips Y.
    pub fn oam_bits(self) -> u8 {
        match self {
            Flip::None => 0,
            Flip::X => 0x20,
            Flip::Y => 0x40,
            Flip::Xy => 0x60,
        }
    }

    /// Whether it mirrors left to right.
    pub fn x(self) -> bool {
        matches!(self, Flip::X | Flip::Xy)
    }

    /// Whether it mirrors top to bottom.
    pub fn y(self) -> bool {
        matches!(self, Flip::Y | Flip::Xy)
    }
}

/// A GBC palette slot, 0-7: attribute bits 0-2 of a background map entry
/// (BG palettes) or of an object (OBJ palettes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct PaletteSlot(u8);

impl PaletteSlot {
    /// Slots of each kind: 8 BG and 8 OBJ palettes.
    pub const COUNT: usize = 8;

    /// `None` past 7.
    pub fn new(slot: u8) -> Option<PaletteSlot> {
        (usize::from(slot) < Self::COUNT).then_some(PaletteSlot(slot))
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

/// A number past 7 isn't a palette slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotPaletteSlot(pub u8);

impl fmt::Display for NotPaletteSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} isn't a GBC palette slot: they are 0 to 7", self.0)
    }
}

impl TryFrom<u8> for PaletteSlot {
    type Error = NotPaletteSlot;

    fn try_from(slot: u8) -> Result<PaletteSlot, NotPaletteSlot> {
        PaletteSlot::new(slot).ok_or(NotPaletteSlot(slot))
    }
}

impl From<PaletteSlot> for u8 {
    fn from(s: PaletteSlot) -> u8 {
        s.0
    }
}

/// Where one source tile (a sheet's 8x16 unit) went.
///
/// `index` is a tile number in the image's own tile data, its first tile
/// 0 (`TownTiles`, `HeroTiles`): a background's or tileset's 8x8 tile, or
/// a sheet unit's top tile, always even, its bottom tile `index + 1` (an
/// 8x16 object's tile number, from `HeroTiles`' first). The index is the
/// same on both machines: tiles count as one only when both machines'
/// versions match, and a GBC image's tile data is in the same order as
/// its DMG data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TileEntry {
    pub index: u32,
    /// How the kept unit is flipped to draw this one; always none for a
    /// background or tileset.
    #[serde(default, skip_serializing_if = "Flip::is_none")]
    pub flip: Flip,
    /// The GBC palette slot: a background's attribute map entry (palette
    /// bits only), a sheet unit's OBJ palette. Present exactly when the
    /// image has a GBC version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette: Option<PaletteSlot>,
}
