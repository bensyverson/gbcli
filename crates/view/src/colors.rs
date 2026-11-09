//! Where every color in an image comes from. Frames use the core's own
//! output; debug views color tiles the way the PPU does, from the palette
//! registers and palette RAM, through the core's own RGB555 conversion.
//! Only the chrome (outlines, gaps, transparency, diff marks) is ours.

use gbcli_core::{ColorMode, Core, CoreError, IoReg};
use gbcli_engine::inspect::video::{PaletteSet, cgb_palettes};

use crate::ViewError;

/// DMG shades 0 (lightest) to 3 as gray: SameBoy's `GB_PALETTE_GREY`, and the
/// grays the corpus judge compares against.
pub const DMG_GRAYS: [u32; 4] = [0xFF_FF_FF, 0xAA_AA_AA, 0x55_55_55, 0x00_00_00];
/// The viewport rectangle on a tilemap.
pub const VIEWPORT: u32 = 0xFF_00_00;
/// Color 0 of an object, which the PPU never draws.
pub const TRANSPARENT: u32 = 0xFF_00_FF;
/// Between the cells of a sheet or an OAM view.
pub const GAP: u32 = 0x30_30_60;
/// A pixel that differs between two frames.
pub const CHANGED: u32 = 0xFF_00_00;

/// Which palette a tileset is drawn with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TilePalette {
    /// Background palette `n`: BGP on a DMG, SGB or a CGB running a DMG
    /// cartridge (only 0 exists there), palette RAM 0–7 in CGB mode.
    Background(u8),
    /// Object palette `n`: OBP0 or OBP1, or palette RAM 0–7 in CGB mode.
    Object(u8),
    /// Color index `i` as gray shade `i`, through no palette: for when the
    /// palettes hide the tiles (BGP = 0 draws everything white).
    Indices,
}

/// The palette registers and palette RAM, read once, colored by the core.
pub(crate) struct Colors {
    mode: ColorMode,
    bgp: u8,
    obp: [u8; 2],
    /// Palette RAM through the core's conversion, where the PPU uses it.
    bg: [[u32; 4]; 8],
    obj: [[u32; 4]; 8],
}

/// A display register (LCDC, SCX, SCY, BGP, OBP0, OBP1) as the CPU last wrote
/// it, readable at any instant (see the engine's `display_register`).
pub(crate) fn display_register(core: &dyn Core, reg: IoReg) -> Result<u8, ViewError> {
    Ok(gbcli_engine::inspect::video::display_register(core, reg)?)
}

fn dmg_shade(register: u8, index: u8) -> usize {
    usize::from((register >> (2 * (index & 3))) & 3)
}

fn out_of_range(what: &'static str, value: u8, limit: u8) -> ViewError {
    ViewError::Core(CoreError::OutOfRange {
        what,
        value: value.into(),
        limit: limit.into(),
    })
}

impl Colors {
    pub fn read(core: &dyn Core) -> Result<Colors, ViewError> {
        let mode = core.color_mode()?;
        let mut colors = Colors {
            mode,
            bgp: display_register(core, IoReg::BGP)?,
            obp: [
                display_register(core, IoReg::OBP0)?,
                display_register(core, IoReg::OBP1)?,
            ],
            bg: [[0; 4]; 8],
            obj: [[0; 4]; 8],
        };
        if matches!(mode, ColorMode::CgbPalettes | ColorMode::CgbDmgCompat) {
            for (set, out) in [
                (PaletteSet::Background, &mut colors.bg),
                (PaletteSet::Object, &mut colors.obj),
            ] {
                for (p, palette) in cgb_palettes(core, set)?.iter().enumerate() {
                    for (c, &color) in palette.colors.iter().enumerate() {
                        out[p][c] = core.rgb555_color(color)?;
                    }
                }
            }
        }
        Ok(colors)
    }

    /// Whether map attributes and OAM's CGB bits pick palettes and banks.
    pub fn cgb_attributes(&self) -> bool {
        self.mode == ColorMode::CgbPalettes
    }

    /// Where the colors come from, for the image's label.
    pub fn describe(&self) -> &'static str {
        match self.mode {
            ColorMode::DmgShades => "dmg shades through BGP/OBP0/OBP1 as SameBoy grays",
            ColorMode::CgbPalettes => "cgb palette RAM through the core's color conversion",
            ColorMode::CgbDmgCompat => {
                "cgb compatibility: BGP/OBPx shades through palette RAM 0/0-1, the core's conversion"
            }
            ColorMode::Sgb => {
                "sgb: shades through BGP/OBP0/OBP1 as grays (SGB palettes not exposed)"
            }
        }
    }

    /// A background or window pixel of color `index`, under CGB attribute
    /// palette `palette` (ignored outside CGB mode).
    pub fn bg(&self, palette: u8, index: u8) -> u32 {
        let shade = dmg_shade(self.bgp, index);
        match self.mode {
            ColorMode::DmgShades | ColorMode::Sgb => DMG_GRAYS[shade],
            ColorMode::CgbDmgCompat => self.bg[0][shade],
            ColorMode::CgbPalettes => self.bg[usize::from(palette & 7)][usize::from(index & 3)],
        }
    }

    /// An object pixel of color `index` (not 0, which is transparent) under
    /// DMG palette `dmg` (OBP0/1) or CGB palette `cgb`, as the mode selects.
    pub fn obj(&self, dmg: u8, cgb: u8, index: u8) -> u32 {
        let dmg = usize::from(dmg & 1);
        let shade = dmg_shade(self.obp[dmg], index);
        match self.mode {
            ColorMode::DmgShades | ColorMode::Sgb => DMG_GRAYS[shade],
            ColorMode::CgbDmgCompat => self.obj[dmg][shade],
            ColorMode::CgbPalettes => self.obj[usize::from(cgb & 7)][usize::from(index & 3)],
        }
    }

    /// The four colors a tileset is drawn with.
    pub fn tile_palette(&self, palette: TilePalette) -> Result<[u32; 4], ViewError> {
        let limit = |n: u8, cgb: u8, dmg: u8, what| {
            let last = if self.cgb_attributes() { cgb } else { dmg };
            if n > last {
                Err(out_of_range(what, n, last))
            } else {
                Ok(n)
            }
        };
        Ok(match palette {
            TilePalette::Indices => DMG_GRAYS,
            TilePalette::Background(n) => {
                let n = limit(n, 7, 0, "background palette")?;
                std::array::from_fn(|i| self.bg(n, i as u8))
            }
            TilePalette::Object(n) => {
                let n = limit(n, 7, 1, "object palette")?;
                std::array::from_fn(|i| self.obj(n, n, i as u8))
            }
        })
    }
}
