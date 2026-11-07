//! The chip's command state machine.

use std::collections::BTreeMap;

use crate::note::{FlashNote, FlashNotes, NoteKind, Operation};
use crate::profile::{DataLines, FlashProfile, WritePin};

/// Notes kept between takes; past this, they are counted, not kept.
const NOTE_CAP: usize = 64;

/// Bus addresses other boards unlock at (FlashGBX's DMG database), for the
/// note that names a program's target board.
const OTHER_UNLOCKS: [u16; 6] = [0x0AAA, 0x0555, 0x5555, 0x4AAA, 0x4555, 0x7AAA];

/// Where the chip is in a command sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Read,
    /// After the first unlock cycle.
    Unlocked1,
    /// After both: the next write at the first unlock address is a command.
    Unlocked2,
    /// After $A0: the next write is the byte to program.
    Program,
    /// After $80, then the erase's own unlock cycles.
    Erase0,
    Erase1,
    Erase2,
    Autoselect,
    /// Running an operation until `until`; reads answer with status.
    Busy {
        op: Operation,
        until: u64,
        /// The programmed byte as the chip holds it (for DQ7), or 0 for an erase.
        data: u8,
    },
}

/// One flash chip on one board.
#[derive(Clone, Debug)]
pub struct FlashChip {
    pub(crate) profile: FlashProfile,
    pub(crate) mode: Mode,
    /// DQ6, which flips on every status read.
    pub(crate) toggle: bool,
    /// The ROM bytes of every sector the chip has changed, as they were
    /// before the first change: what a load restores when the state it loads
    /// left that sector alone.
    pub(crate) pristine: BTreeMap<u32, Vec<u8>>,
    notes: FlashNotes,
}

impl FlashChip {
    pub fn new(profile: FlashProfile) -> FlashChip {
        FlashChip {
            profile,
            mode: Mode::Read,
            toggle: false,
            pristine: BTreeMap::new(),
            notes: FlashNotes::default(),
        }
    }

    pub fn profile(&self) -> &FlashProfile {
        &self.profile
    }

    /// Whether an operation is still running at `now`.
    pub fn busy(&self, now: u64) -> bool {
        matches!(self.mode, Mode::Busy { until, .. } if now < until)
    }

    /// A CPU write to $0000-$7FFF at `now`. `chip_addr` is the bank's base
    /// plus the offset; `rom` is the array in the bytes the CPU reads. The
    /// mapper sees the write too: that is the caller's job.
    pub fn write(&mut self, now: u64, bus_addr: u16, chip_addr: u32, value: u8, rom: &mut [u8]) {
        self.settle(now);
        let seen = self.profile.data_lines.cross(value);
        if self.profile.write_pin == WritePin::Audio {
            if self.looks_like_unlock(bus_addr, chip_addr, value) {
                self.note(now, bus_addr, value, NoteKind::WritePinAudio);
            }
            return;
        }
        if let Mode::Busy { op, .. } = self.mode {
            self.note(now, bus_addr, value, NoteKind::WriteWhileBusy { op });
            return;
        }
        let first = self.profile.is_cycle(chip_addr, 0);
        let second = self.profile.is_cycle(chip_addr, 1);
        // A reset ends any sequence and autoselect, at any address.
        if seen == 0xF0 && self.mode != Mode::Program {
            self.mode = Mode::Read;
            return;
        }
        self.mode = match self.mode {
            Mode::Read | Mode::Autoselect if first && seen == 0xAA => Mode::Unlocked1,
            Mode::Read => {
                self.diagnose(now, bus_addr, chip_addr, value);
                Mode::Read
            }
            Mode::Autoselect => Mode::Autoselect,
            Mode::Unlocked1 if second && seen == 0x55 => Mode::Unlocked2,
            Mode::Unlocked2 if first && seen == 0xA0 => Mode::Program,
            Mode::Unlocked2 if first && seen == 0x80 => Mode::Erase0,
            Mode::Unlocked2 if first && seen == 0x90 => Mode::Autoselect,
            Mode::Erase0 if first && seen == 0xAA => Mode::Erase1,
            Mode::Erase1 if second && seen == 0x55 => Mode::Erase2,
            Mode::Erase2 if seen == 0x30 => self.erase_sector(now, chip_addr, rom),
            Mode::Erase2 if first && seen == 0x10 => self.erase_chip(now, rom),
            Mode::Program => self.program(now, bus_addr, chip_addr, value, rom),
            broken => {
                let after = match broken {
                    Mode::Unlocked1 => "the first unlock cycle",
                    Mode::Unlocked2 => "both unlock cycles",
                    Mode::Erase0 => "the erase command ($80)",
                    _ => "the erase's unlock cycles",
                };
                self.note(now, bus_addr, value, NoteKind::BrokenSequence { after });
                self.diagnose(now, bus_addr, chip_addr, value);
                Mode::Read
            }
        };
    }

    /// A CPU read of $0000-$7FFF at `now`: `data` is the ROM byte, returned
    /// unless the chip is answering with status or IDs. Status reads flip
    /// DQ6, so only a real bus read may call this.
    pub fn read(&mut self, now: u64, chip_addr: u32, data: u8) -> u8 {
        self.settle(now);
        match self.mode {
            Mode::Busy { op, data: held, .. } => {
                self.toggle = !self.toggle;
                let dq6 = if self.toggle { 0x40 } else { 0 };
                let status = match op {
                    Operation::Program => (!held & 0x80) | dq6,
                    // DQ3: the erase has begun.
                    Operation::SectorErase | Operation::ChipErase => dq6 | 0x08,
                };
                self.profile.data_lines.cross(status)
            }
            Mode::Autoselect => match &self.profile.ids {
                Some(ids) if !ids.is_empty() => ids[chip_addr as usize % ids.len()],
                _ => 0xFF,
            },
            _ => data,
        }
    }

    /// Ends an operation whose time is up.
    fn settle(&mut self, now: u64) {
        if let Mode::Busy { until, .. } = self.mode
            && now >= until
        {
            self.mode = Mode::Read;
        }
    }

    fn program(
        &mut self,
        now: u64,
        bus_addr: u16,
        chip_addr: u32,
        value: u8,
        rom: &mut [u8],
    ) -> Mode {
        let Some(at) = index(chip_addr, rom) else {
            return Mode::Read;
        };
        let sector = self.profile.sector_of(at as u32, rom.len() as u32);
        self.touch(sector, rom);
        let was = rom[at];
        if value & !was != 0 {
            let kind = NoteKind::ProgramSetsBits { was, wanted: value };
            self.note(now, bus_addr, value, kind);
        }
        rom[at] = was & value;
        Mode::Busy {
            op: Operation::Program,
            until: now + self.profile.busy.program,
            data: self.profile.data_lines.cross(value),
        }
    }

    fn erase_sector(&mut self, now: u64, chip_addr: u32, rom: &mut [u8]) -> Mode {
        if let Some(at) = index(chip_addr, rom) {
            let sector = self.profile.sector_of(at as u32, rom.len() as u32);
            self.touch(sector.clone(), rom);
            rom[sector.start as usize..sector.end as usize].fill(0xFF);
        }
        Mode::Busy {
            op: Operation::SectorErase,
            until: now + self.profile.busy.sector_erase,
            data: 0,
        }
    }

    fn erase_chip(&mut self, now: u64, rom: &mut [u8]) -> Mode {
        let size = rom.len() as u32;
        let mut start = 0;
        while start < size {
            let sector = self.profile.sector_of(start, size);
            start = sector.end;
            self.touch(sector, rom);
        }
        rom.fill(0xFF);
        Mode::Busy {
            op: Operation::ChipErase,
            until: now + self.profile.busy.chip_erase,
            data: 0,
        }
    }

    /// Whether a write would be the first unlock cycle under either data
    /// wiring: what a program trying to command the chip sends.
    fn looks_like_unlock(&self, bus_addr: u16, chip_addr: u32, value: u8) -> bool {
        let unlock_byte = value == 0xAA || value == DataLines::SwappedD0D1.cross(0xAA);
        unlock_byte && (self.profile.is_cycle(chip_addr, 0) || OTHER_UNLOCKS.contains(&bus_addr))
    }

    /// Notes a write in read mode that a program meant as an unlock cycle
    /// for another board. Ordinary mapper writes match neither case.
    fn diagnose(&mut self, now: u64, bus_addr: u16, chip_addr: u32, value: u8) {
        let lines = self.profile.data_lines;
        let seen = lines.cross(value);
        if self.profile.is_cycle(chip_addr, 0) && lines.other().cross(value) == 0xAA {
            let kind = NoteKind::DataLines {
                seen,
                expected: 0xAA,
                written_for: lines.other(),
            };
            self.note(now, bus_addr, value, kind);
        } else if seen == 0xAA && OTHER_UNLOCKS.contains(&bus_addr) {
            let expected = self.profile.unlock[0];
            self.note(now, bus_addr, value, NoteKind::UnlockAddress { expected });
        }
    }

    /// The notes since the last take.
    pub fn take_notes(&mut self) -> FlashNotes {
        std::mem::take(&mut self.notes)
    }

    pub(crate) fn note(&mut self, at: u64, bus_addr: u16, value: u8, kind: NoteKind) {
        if self.notes.notes.len() < NOTE_CAP {
            self.notes.notes.push(FlashNote {
                at,
                bus_addr,
                value,
                kind,
            });
        } else {
            self.notes.dropped += 1;
        }
    }

    /// Records a sector's bytes before its first change.
    pub(crate) fn touch(&mut self, sector: std::ops::Range<u32>, rom: &[u8]) {
        self.pristine
            .entry(sector.start)
            .or_insert_with(|| rom[sector.start as usize..sector.end as usize].to_vec());
    }
}

/// A chip address's index into the array, wrapping as the address lines do
/// on a chip smaller than the mapper reaches.
fn index(chip_addr: u32, rom: &[u8]) -> Option<usize> {
    (!rom.is_empty()).then(|| chip_addr as usize % rom.len())
}
