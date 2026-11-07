//! Tiles, tilemaps, OAM and CGB palettes as data, decoded from VRAM, OAM and
//! palette RAM (images are stage 4).

use gbcli_core::{ColorMode, Core, CoreError, IoReg, Region};

const TILE_BYTES: usize = 16;
/// Tiles in one VRAM bank (8000-97FF).
pub const TILES_PER_BANK: u16 = 384;
const VRAM_BANK: usize = 0x2000;
const OBJECTS: usize = 40;
const SCREEN_LINES: i16 = 144;
const SCREEN_WIDTH: i16 = 160;
/// Objects the PPU draws on one line; later ones in OAM order are dropped.
const OBJECTS_PER_LINE: usize = 10;

/// One 8×8 tile as color indices 0–3, `pixels[y][x]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tile {
    pub bank: u8,
    /// 0–383, counted from 8000.
    pub index: u16,
    pub pixels: [[u8; 8]; 8],
}

/// Decodes tile `index` (0–383, from 8000) of VRAM `bank`.
pub fn tile(core: &dyn Core, bank: u8, index: u16) -> Result<Tile, CoreError> {
    let vram = core.region(Region::Vram)?.bytes;
    let banks = vram.len() / VRAM_BANK;
    if usize::from(bank) >= banks {
        return Err(CoreError::OutOfRange {
            what: "VRAM bank",
            value: bank.into(),
            limit: banks.saturating_sub(1) as u64,
        });
    }
    if index >= TILES_PER_BANK {
        return Err(CoreError::OutOfRange {
            what: "tile index",
            value: index.into(),
            limit: u64::from(TILES_PER_BANK - 1),
        });
    }
    let start = usize::from(bank) * VRAM_BANK + usize::from(index) * TILE_BYTES;
    let bytes = &vram[start..start + TILE_BYTES];
    let mut pixels = [[0u8; 8]; 8];
    for (y, row) in pixels.iter_mut().enumerate() {
        let (lo, hi) = (bytes[2 * y], bytes[2 * y + 1]);
        for (x, px) in row.iter_mut().enumerate() {
            let bit = 7 - x;
            *px = ((lo >> bit) & 1) | (((hi >> bit) & 1) << 1);
        }
    }
    Ok(Tile {
        bank,
        index,
        pixels,
    })
}

/// Which of the two 32×32 maps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapArea {
    Map9800,
    Map9C00,
}

/// How a map's tile IDs pick tiles (LCDC bit 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileAddressing {
    /// IDs 0–255 from 8000 (LCDC.4 = 1).
    Unsigned8000,
    /// IDs as signed offsets from 9000 (LCDC.4 = 0).
    Signed8800,
}

/// A map entry's CGB attribute byte (VRAM bank 1), decoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BgAttributes {
    /// Background over objects.
    pub priority: bool,
    pub y_flip: bool,
    pub x_flip: bool,
    /// VRAM bank of the tile.
    pub bank: u8,
    pub palette: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapEntry {
    /// The byte in the map.
    pub tile_id: u8,
    /// The tile it selects, 0–383 from 8000, under the map's addressing.
    pub tile_index: u16,
    /// Present wherever VRAM bank 1 exists; the PPU uses it only in CGB mode.
    pub attributes: Option<BgAttributes>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tilemap {
    pub area: MapArea,
    pub addressing: TileAddressing,
    /// 32×32, row-major.
    pub entries: Vec<MapEntry>,
}

pub fn tilemap(
    core: &dyn Core,
    area: MapArea,
    addressing: TileAddressing,
) -> Result<Tilemap, CoreError> {
    let vram = core.region(Region::Vram)?.bytes;
    let base = match area {
        MapArea::Map9800 => 0x1800,
        MapArea::Map9C00 => 0x1C00,
    };
    let entries = (0..32 * 32)
        .map(|i| {
            let tile_id = vram[base + i];
            let tile_index = match addressing {
                TileAddressing::Unsigned8000 => u16::from(tile_id),
                TileAddressing::Signed8800 => (256 + i16::from(tile_id as i8)) as u16,
            };
            let attributes = vram.get(VRAM_BANK + base + i).map(|&a| BgAttributes {
                priority: a & 0x80 != 0,
                y_flip: a & 0x40 != 0,
                x_flip: a & 0x20 != 0,
                bank: (a >> 3) & 1,
                palette: a & 7,
            });
            MapEntry {
                tile_id,
                tile_index,
                attributes,
            }
        })
        .collect();
    Ok(Tilemap {
        area,
        addressing,
        entries,
    })
}

/// Object height (LCDC bit 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectHeight {
    Eight,
    Sixteen,
}

impl ObjectHeight {
    fn lines(self) -> i16 {
        match self {
            ObjectHeight::Eight => 8,
            ObjectHeight::Sixteen => 16,
        }
    }
}

/// The object height LCDC selects now; fails where LCDC doesn't decode.
pub fn object_height(core: &dyn Core) -> Result<ObjectHeight, CoreError> {
    let lcdc = core.io_register(IoReg::LCDC)?;
    Ok(if lcdc & 0x04 != 0 {
        ObjectHeight::Sixteen
    } else {
        ObjectHeight::Eight
    })
}

/// One OAM entry, decoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OamEntry {
    pub index: u8,
    /// Raw OAM bytes.
    pub y: u8,
    pub x: u8,
    pub tile: u8,
    pub flags: u8,
    /// Top-left corner on screen: y - 16, x - 8.
    pub screen_y: i16,
    pub screen_x: i16,
    pub behind_background: bool,
    pub y_flip: bool,
    pub x_flip: bool,
    /// OBP0 or OBP1 (DMG).
    pub dmg_palette: u8,
    /// VRAM bank of the tile (CGB mode).
    pub cgb_bank: u8,
    /// Palette 0–7 (CGB mode).
    pub cgb_palette: u8,
    /// Covers a visible line and a visible column.
    pub on_screen: bool,
    /// Eleventh or later on one of its lines, so the PPU skips it there.
    pub dropped_by_line_limit: bool,
}

/// The 40 OAM entries, with visibility and the 10-per-line limit worked
/// out for `height` (see [`object_height`]).
pub fn oam(core: &dyn Core, height: ObjectHeight) -> Result<Vec<OamEntry>, CoreError> {
    let bytes = core.region(Region::Oam)?.bytes;
    let mut entries: Vec<OamEntry> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .take(OBJECTS)
        .enumerate()
        .map(|(i, &[y, x, tile, flags])| {
            let screen_y = i16::from(y) - 16;
            let screen_x = i16::from(x) - 8;
            let covers_line = screen_y < SCREEN_LINES && screen_y + height.lines() > 0;
            OamEntry {
                index: i as u8,
                y,
                x,
                tile,
                flags,
                screen_y,
                screen_x,
                behind_background: flags & 0x80 != 0,
                y_flip: flags & 0x40 != 0,
                x_flip: flags & 0x20 != 0,
                dmg_palette: (flags >> 4) & 1,
                cgb_bank: (flags >> 3) & 1,
                cgb_palette: flags & 7,
                on_screen: covers_line && screen_x > -8 && screen_x < SCREEN_WIDTH,
                dropped_by_line_limit: false,
            }
        })
        .collect();
    for line in 0..SCREEN_LINES {
        entries
            .iter_mut()
            .filter(|e| e.screen_y <= line && line < e.screen_y + height.lines())
            .skip(OBJECTS_PER_LINE)
            .for_each(|e| e.dropped_by_line_limit = true);
    }
    Ok(entries)
}

/// One OAM entry as `oam[N]` reads it (design §7): its four bytes and what
/// the attribute byte says, without the screen geometry [`oam`] works out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectAttributes {
    pub index: u8,
    pub y: u8,
    pub x: u8,
    pub tile: u8,
    pub attr: u8,
    /// Bit 7: background and window colors 1–3 are drawn over the object.
    pub priority: bool,
    pub y_flip: bool,
    pub x_flip: bool,
    /// Bit 4: OBP0 or OBP1, the palette outside CGB mode.
    pub palette: u8,
    /// Bits 3 and 0–2, which only CGB mode reads.
    pub cgb: Option<CgbObjectAttributes>,
}

/// The attribute bits CGB mode adds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CgbObjectAttributes {
    /// The VRAM bank of the tile.
    pub bank: u8,
    /// Object palette 0–7.
    pub palette: u8,
}

/// OAM entry `index` (0–39), decoded; the CGB bits only when the PPU is in
/// CGB mode, since a DMG, and a CGB running a DMG cartridge, ignores them.
pub fn object_attributes(core: &dyn Core, index: u8) -> Result<ObjectAttributes, CoreError> {
    let i = usize::from(index);
    if i >= OBJECTS {
        return Err(CoreError::InvalidState {
            reason: format!(
                "OAM holds {OBJECTS} entries, 0 to {}; not {index}",
                OBJECTS - 1
            ),
        });
    }
    let bytes = core.region(Region::Oam)?.bytes;
    let [y, x, tile, attr] = bytes[4 * i..4 * i + 4] else {
        unreachable!("OAM holds 40 four-byte entries");
    };
    let cgb_mode = matches!(core.color_mode(), Ok(ColorMode::CgbPalettes));
    Ok(ObjectAttributes {
        index,
        y,
        x,
        tile,
        attr,
        priority: attr & 0x80 != 0,
        y_flip: attr & 0x40 != 0,
        x_flip: attr & 0x20 != 0,
        palette: (attr >> 4) & 1,
        cgb: cgb_mode.then_some(CgbObjectAttributes {
            bank: (attr >> 3) & 1,
            palette: attr & 7,
        }),
    })
}

/// Which CGB palette RAM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteSet {
    Background,
    Object,
}

/// Four RGB555 colors (bits 0–4 red, 5–9 green, 10–14 blue).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CgbPalette {
    pub colors: [u16; 4],
}

/// The eight palettes of a CGB palette RAM; fails on a model without one.
pub fn cgb_palettes(core: &dyn Core, set: PaletteSet) -> Result<[CgbPalette; 8], CoreError> {
    let region = match set {
        PaletteSet::Background => Region::BgPalette,
        PaletteSet::Object => Region::ObjPalette,
    };
    let bytes = core.region(region)?.bytes;
    if bytes.len() < 64 {
        return Err(CoreError::InvalidState {
            reason: format!("{set:?} palette RAM has {} bytes, not 64", bytes.len()),
        });
    }
    Ok(std::array::from_fn(|p| CgbPalette {
        colors: std::array::from_fn(|c| {
            let at = 8 * p + 2 * c;
            u16::from_le_bytes([bytes[at], bytes[at + 1]])
        }),
    }))
}

/// An RGB555 color's 5-bit red, green and blue.
pub fn rgb555(color: u16) -> (u8, u8, u8) {
    let channel = |shift: u16| ((color >> shift) & 0x1F) as u8;
    (channel(0), channel(5), channel(10))
}
