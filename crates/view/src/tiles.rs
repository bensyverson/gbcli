//! VRAM views: the tileset, a full 256×256 tilemap with the viewport drawn
//! on it, and the 40 OAM objects. The data behind each is in
//! `gbcli_engine::inspect::video`; these draw it.

use gbcli_core::{Core, IoReg, Region};
use gbcli_engine::inspect::video::{
    MapArea, ObjectHeight, TILES_PER_BANK, Tile, TileAddressing, oam, tile, tilemap as map_data,
};

use crate::ViewError;
use crate::colors::{Colors, GAP, TRANSPARENT, TilePalette, VIEWPORT, display_register};
use crate::image::{Image, LabelKey};

const VRAM_BANK: usize = 0x2000;
/// Tiles per row of a tileset bank, as VRAM viewers lay them out.
const TILESET_COLUMNS: u32 = 16;
const SCREEN: (u32, u32) = (160, 144);
const OAM_COLUMNS: u32 = 8;
const OBJECTS: u32 = 40;

/// Which map a tilemap view shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapChoice {
    /// The map LCDC bit 3 selects for the background.
    Background,
    /// The map LCDC bit 6 selects for the window.
    Window,
    Area(MapArea),
}

fn area_name(area: MapArea) -> &'static str {
    match area {
        MapArea::Map9800 => "9800",
        MapArea::Map9C00 => "9C00",
    }
}

/// Draws `tile` with `colors[index]` at (`x`, `y`), flipped as asked.
fn draw_tile(image: &mut Image, tile: &Tile, x: u32, y: u32, flip: (bool, bool), colors: [u32; 4]) {
    for ty in 0..8 {
        for tx in 0..8 {
            let sx = if flip.0 { 7 - tx } else { tx };
            let sy = if flip.1 { 7 - ty } else { ty };
            let index = tile.pixels[sy as usize][sx as usize];
            image.set(x + tx, y + ty, colors[usize::from(index)]);
        }
    }
}

/// Every tile in VRAM, 16 to a row, one 128×192 block per VRAM bank side by
/// side, drawn with `palette`.
pub fn tileset(core: &dyn Core, palette: TilePalette) -> Result<Image, ViewError> {
    let colors = Colors::read(core)?;
    let four = colors.tile_palette(palette)?;
    let banks = (core.region(Region::Vram)?.bytes.len() / VRAM_BANK) as u32;
    let bank_width = TILESET_COLUMNS * 8;
    let rows = u32::from(TILES_PER_BANK) / TILESET_COLUMNS;
    let mut image = Image::filled(bank_width * banks, rows * 8, GAP);
    for bank in 0..banks {
        for index in 0..TILES_PER_BANK {
            let t = tile(core, bank as u8, index)?;
            let (col, row) = (
                u32::from(index) % TILESET_COLUMNS,
                u32::from(index) / TILESET_COLUMNS,
            );
            draw_tile(
                &mut image,
                &t,
                bank * bank_width + col * 8,
                row * 8,
                (false, false),
                four,
            );
        }
    }
    Ok(image
        .label(LabelKey::View, "tileset")
        .label(LabelKey::Colors, colors.describe())
        .label(
            LabelKey::Detail,
            format!("{palette:?}, {banks} VRAM bank(s), tiles from 8000"),
        ))
}

/// The whole 32×32 map as LCDC addresses its tiles, colored as the PPU colors
/// it, with the 160×144 viewport outlined at (SCX, SCY) when the map shown is
/// the background's.
pub fn tilemap(core: &dyn Core, choice: MapChoice) -> Result<Image, ViewError> {
    let colors = Colors::read(core)?;
    let lcdc = display_register(core, IoReg::LCDC)?;
    let pick = |bit: u8| {
        if lcdc & bit != 0 {
            MapArea::Map9C00
        } else {
            MapArea::Map9800
        }
    };
    let bg_area = pick(0x08);
    let area = match choice {
        MapChoice::Background => bg_area,
        MapChoice::Window => pick(0x40),
        MapChoice::Area(area) => area,
    };
    let addressing = if lcdc & 0x10 != 0 {
        TileAddressing::Unsigned8000
    } else {
        TileAddressing::Signed8800
    };
    let map = map_data(core, area, addressing)?;
    let mut image = Image::filled(256, 256, GAP);
    for (i, entry) in map.entries.iter().enumerate() {
        let attrs = entry.attributes.filter(|_| colors.cgb_attributes());
        let bank = attrs.map_or(0, |a| a.bank);
        let palette = attrs.map_or(0, |a| a.palette);
        let flip = attrs.map_or((false, false), |a| (a.x_flip, a.y_flip));
        let t = tile(core, bank, entry.tile_index)?;
        let four = std::array::from_fn(|c| colors.bg(palette, c as u8));
        draw_tile(
            &mut image,
            &t,
            (i as u32 % 32) * 8,
            (i as u32 / 32) * 8,
            flip,
            four,
        );
    }
    let mut detail = format!(
        "map {}, tiles {}",
        area_name(area),
        match addressing {
            TileAddressing::Unsigned8000 => "8000 unsigned",
            TileAddressing::Signed8800 => "8800 signed",
        }
    );
    if area == bg_area {
        let scx = u32::from(display_register(core, IoReg::SCX)?);
        let scy = u32::from(display_register(core, IoReg::SCY)?);
        image.outline_wrapped(scx, scy, SCREEN.0, SCREEN.1, VIEWPORT);
        detail.push_str(&format!(", viewport SCX={scx} SCY={scy}"));
    }
    Ok(image
        .label(LabelKey::View, "tilemap")
        .label(LabelKey::Colors, colors.describe())
        .label(LabelKey::Detail, detail))
}

/// The 40 objects in OAM order, 8 to a row, each drawn as the PPU would draw
/// it (its tile or tile pair, flips, palette) with color 0 as [`TRANSPARENT`].
pub fn oam_sheet(core: &dyn Core) -> Result<Image, ViewError> {
    let colors = Colors::read(core)?;
    // LCDC bit 2, read as the other display registers are.
    let height = if display_register(core, IoReg::LCDC)? & 0x04 != 0 {
        ObjectHeight::Sixteen
    } else {
        ObjectHeight::Eight
    };
    let cell_h = match height {
        ObjectHeight::Eight => 8,
        ObjectHeight::Sixteen => 16,
    };
    let rows = OBJECTS / OAM_COLUMNS;
    let mut image = Image::filled(OAM_COLUMNS * 9 - 1, rows * (cell_h + 1) - 1, GAP);
    for entry in oam(core, height)? {
        let bank = if colors.cgb_attributes() {
            entry.cgb_bank
        } else {
            0
        };
        let mut four: [u32; 4] =
            std::array::from_fn(|c| colors.obj(entry.dmg_palette, entry.cgb_palette, c as u8));
        four[0] = TRANSPARENT;
        let tiles: Vec<u8> = match height {
            ObjectHeight::Eight => vec![entry.tile],
            ObjectHeight::Sixteen if entry.y_flip => vec![entry.tile | 1, entry.tile & 0xFE],
            ObjectHeight::Sixteen => vec![entry.tile & 0xFE, entry.tile | 1],
        };
        let i = u32::from(entry.index);
        let (x, y) = ((i % OAM_COLUMNS) * 9, (i / OAM_COLUMNS) * (cell_h + 1));
        for (half, &id) in tiles.iter().enumerate() {
            let t = tile(core, bank, u16::from(id))?;
            draw_tile(
                &mut image,
                &t,
                x,
                y + 8 * half as u32,
                (entry.x_flip, entry.y_flip),
                four,
            );
        }
    }
    Ok(image
        .label(LabelKey::View, "oam")
        .label(LabelKey::Colors, colors.describe())
        .label(
            LabelKey::Detail,
            format!("40 objects in OAM order, 8 per row, 8x{cell_h}, color 0 transparent"),
        ))
}
