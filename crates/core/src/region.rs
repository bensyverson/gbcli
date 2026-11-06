//! Memory regions and frames, read without side effects.

use crate::time::Ticks;

/// A memory a core exposes by direct access (design §2.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Region {
    Rom,
    Wram,
    /// Cartridge RAM.
    Sram,
    Vram,
    Hram,
    Oam,
    /// Raw IO storage, which is not the register values: see `Core::io_register`.
    Io,
    Ie,
    BootRom,
    /// CGB background palette RAM.
    BgPalette,
    /// CGB object palette RAM.
    ObjPalette,
}

/// A region's whole backing store (every bank) and the bank mapped now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionView<'a> {
    pub bytes: &'a [u8],
    pub bank: u16,
}

/// What kind of vblank produced a frame (mirrors SameBoy's vblank types).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VblankKind {
    /// A real vblank: the frame is complete.
    Normal,
    /// A synthetic frame while the LCD is off.
    LcdOff,
    /// A synthetic frame for another reason (e.g. the DMG PPU in STOP mode).
    Artificial,
}

/// Which frame a framebuffer holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameTag {
    /// Vblanks since power-on, counting this one (the first frame is 1).
    pub number: u64,
    pub kind: VblankKind,
    pub at: Ticks,
}

/// Pixel data of a frame, row-major.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pixels<'a> {
    /// DMG/MGB: shade 0 (lightest) to 3 (darkest) per pixel, after BGP/OBP mapping.
    Shades(&'a [u8]),
    /// CGB, AGB and SGB: 0x00RRGGBB per pixel.
    Rgb(&'a [u32]),
}

/// The last complete frame, separate from the live, partial buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameRef<'a> {
    pub tag: FrameTag,
    pub width: u32,
    pub height: u32,
    pub pixels: Pixels<'a>,
}

/// Which bytes of a region have been written, one bit per byte (bit `i % 64`
/// of word `i / 64`), since power-on. Saved and loaded with the state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WrittenView<'a> {
    /// Bytes in the region.
    pub len: usize,
    /// Set by CPU writes (the write hook), including bus writes.
    pub cpu: &'a [u64],
    /// Set by pokes.
    pub poked: &'a [u64],
}

impl WrittenView<'_> {
    pub fn cpu_wrote(&self, offset: usize) -> bool {
        bit(self.cpu, offset)
    }
    pub fn was_poked(&self, offset: usize) -> bool {
        bit(self.poked, offset)
    }
    /// Neither written by the CPU nor poked: the byte holds its power-on value.
    pub fn never_written(&self, offset: usize) -> bool {
        !self.cpu_wrote(offset) && !self.was_poked(offset)
    }
}

fn bit(words: &[u64], offset: usize) -> bool {
    words
        .get(offset / 64)
        .is_some_and(|word| word & (1 << (offset % 64)) != 0)
}

/// The layer that drew a pixel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    Background,
    Window,
    Object { index: u8 },
}

/// Which layer drew each pixel of the last complete frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayerMap<'a> {
    pub width: u32,
    pub height: u32,
    pub layers: &'a [Layer],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn written_bits_read_per_byte() {
        let cpu = [0b10, 1 << 63];
        let poked = [0b100, 0];
        let view = WrittenView {
            len: 128,
            cpu: &cpu,
            poked: &poked,
        };
        assert!(view.cpu_wrote(1) && view.cpu_wrote(127));
        assert!(!view.cpu_wrote(0) && !view.cpu_wrote(64));
        assert!(view.was_poked(2) && !view.cpu_wrote(2));
        assert!(view.never_written(0) && !view.never_written(2));
        assert!(!view.cpu_wrote(500), "past the end reads as unwritten");
    }
}
