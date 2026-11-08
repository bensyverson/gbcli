//! What distinguishes one flash board from another.

use std::ops::Range;

use crate::TICKS_PER_SECOND;

/// How the cart's data lines reach the chip's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataLines {
    Straight,
    /// The cart's D0 is the chip's D1 and the other way round, as on many
    /// cheap boards: the CPU must write $A9 for the chip to see $AA.
    SwappedD0D1,
}

impl DataLines {
    /// A byte as it crosses the board, either way (the swap is its own
    /// inverse).
    pub fn cross(self, value: u8) -> u8 {
        match self {
            DataLines::Straight => value,
            DataLines::SwappedD0D1 => (value & 0xFC) | ((value & 1) << 1) | ((value >> 1) & 1),
        }
    }

    /// The other wiring, for a note that names it.
    pub fn other(self) -> DataLines {
        match self {
            DataLines::Straight => DataLines::SwappedD0D1,
            DataLines::SwappedD0D1 => DataLines::Straight,
        }
    }
}

/// Which cart pin drives the chip's /WE.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WritePin {
    /// The bus's /WR: the CPU's writes reach the chip.
    Wr,
    /// The AUDIO (VIN) pin, which only a flasher drives: no CPU write ever
    /// reaches the chip, so a game can't save to it.
    Audio,
}

/// `count` sectors of `size` bytes each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SectorRun {
    pub size: u32,
    pub count: u32,
}

/// How long each operation keeps the chip busy, in ticks of 2^23 Hz.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BusyTicks {
    pub program: u64,
    pub sector_erase: u64,
    pub chip_erase: u64,
}

impl BusyTicks {
    /// Round typical figures for 5 V and 3.3 V AMD-style parts (assumed, not
    /// taken from one datasheet): 10 µs to program a byte, 0.7 s to erase a
    /// sector, 25 s to erase the chip. A program that waits less than a real
    /// chip needs should fail here too, so the figures err long.
    pub const TYPICAL: BusyTicks = BusyTicks {
        program: TICKS_PER_SECOND / 100_000,
        sector_erase: TICKS_PER_SECOND * 7 / 10,
        chip_erase: TICKS_PER_SECOND * 25,
    };

    /// No busy time at all: every operation is done by the next access.
    pub const INSTANT: BusyTicks = BusyTicks {
        program: 0,
        sector_erase: 0,
        chip_erase: 0,
    };
}

/// One flash board: the chip's command decoding and how the cart wires it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlashProfile {
    /// The profile's name, as [`FlashProfile::named`] takes it.
    pub name: String,
    /// Where its facts come from (file and commit).
    pub source: String,
    /// The bus addresses of the two unlock cycles (the chip sees $AA, then
    /// $55): $0AAA/$0555, $0555/$02AA, $5555/$2AAA, $4AAA/$4555...
    pub unlock: [u16; 2],
    /// Which chip-address bits the command decoder compares. A command cycle
    /// matches when `chip_addr & mask == unlock & mask`, where the chip
    /// address is the bank's base plus the offset. AMD-style chips ignore
    /// A11 and up in command cycles, so a mask of $FFF (byte mode) makes
    /// $4AAA the same cycle as $0AAA on every bank.
    pub command_mask: u32,
    pub data_lines: DataLines,
    pub write_pin: WritePin,
    /// The chip's size in bytes. The ROM image fills it from address 0 and
    /// the rest reads erased ($FF); the mapper reaches all of it, whatever
    /// the ROM header says, so a save past the image's end (GB Studio's
    /// banks 12-15 on a 128 KiB game) lands in real flash.
    pub size: u32,
    /// The sector map from chip address 0, as FlashGBX lists it; past its
    /// end (or when empty), sectors are 64 KiB.
    pub sectors: Vec<SectorRun>,
    /// What the CPU reads at chip addresses 0, 1... in autoselect mode,
    /// repeating (as FlashGBX's `flash_ids` lists them, after the data
    /// lines), or `None` when the source doesn't say; the chip then answers
    /// with $FF.
    pub ids: Option<Vec<u8>>,
    pub busy: BusyTicks,
}

/// The sector size past the end of a profile's map.
pub const DEFAULT_SECTOR: u32 = 0x10000;

impl FlashProfile {
    /// The sector holding `chip_addr`, in a chip of `size` bytes.
    pub fn sector_of(&self, chip_addr: u32, size: u32) -> Range<u32> {
        let mut start = 0u32;
        for run in &self.sectors {
            for _ in 0..run.count {
                let end = start.saturating_add(run.size);
                if chip_addr < end {
                    return start..end.min(size);
                }
                start = end;
            }
        }
        let past = (chip_addr - start) / DEFAULT_SECTOR;
        let lo = start + past * DEFAULT_SECTOR;
        lo..lo.saturating_add(DEFAULT_SECTOR).min(size)
    }

    /// Whether a write's chip address is the given unlock cycle's.
    pub(crate) fn is_cycle(&self, chip_addr: u32, cycle: usize) -> bool {
        is_cycle(self.unlock[cycle], self.command_mask, chip_addr)
    }
}

/// Whether a write's chip address matches an unlock address under a decode
/// mask: the one test every command cycle, and the mask-ROM hint, use.
pub(crate) fn is_cycle(unlock: u16, mask: u32, chip_addr: u32) -> bool {
    chip_addr & mask == u32::from(unlock) & mask
}
