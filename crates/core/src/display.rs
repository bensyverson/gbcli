//! The live framebuffer and the core's own color output, for image views.

/// Pixel data a core hands over by value, row-major: the owned twin of
/// [`Pixels`](crate::Pixels).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PixelBuf {
    /// DMG/MGB: shade 0 (lightest) to 3 (darkest) per pixel, after BGP/OBP mapping.
    Shades(Vec<u8>),
    /// CGB, AGB and SGB: 0x00RRGGBB per pixel.
    Rgb(Vec<u32>),
}

/// The framebuffer as it is now, mid-frame: lines the PPU has drawn of the
/// frame in progress and, below them, what the buffer held before (the
/// previous frame's lines). Never a complete frame; see
/// [`Core::last_complete_frame`](crate::Core::last_complete_frame).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveFrame {
    pub width: u32,
    pub height: u32,
    pub pixels: PixelBuf,
}

/// How the PPU turns a tile's color indices into the colors a frame holds now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMode {
    /// DMG and MGB: BGP, OBP0 and OBP1 pick shades 0–3; frames hold shades.
    DmgShades,
    /// A CGB or AGB in CGB mode: the attribute or OAM palette picks a color
    /// from palette RAM.
    CgbPalettes,
    /// A CGB or AGB running a DMG cartridge: BGP picks a shade, and the shade
    /// picks a color from background palette 0 (OBP0 and OBP1 from object
    /// palettes 0 and 1).
    CgbDmgCompat,
    /// A Super Game Boy: BGP and OBPx pick shades, which the SGB's own
    /// palettes color (not exposed by the core).
    Sgb,
}
