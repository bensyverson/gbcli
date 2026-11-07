//! An AMD/JEDEC flash-chip model for flash-save Game Boy carts: GB Studio's
//! batteryless saves, and any program that writes its own cart's flash.
//!
//! The model is backend-free. A core routes every CPU write to $0000-$7FFF
//! through [`FlashChip::write`] *and* on to its mapper (the chip and the MBC
//! share the bus, so a command at $0AAA is also an MBC RAM-enable write), and
//! every CPU read of $0000-$7FFF through [`FlashChip::read`], which replaces
//! the ROM byte while the chip answers with status or IDs. The chip's array
//! is the core's ROM buffer, in the bytes the CPU reads: programming and
//! erasing change it in place, so inspection sees the save.
//!
//! Boards differ, and the differences decide whether a save works, so every
//! one is a [`FlashProfile`] parameter: the unlock addresses, a D0/D1 data-line
//! swap, the sector map, whether /WE is on /WR or the AUDIO pin (which the
//! Game Boy can't drive), and how long the chip stays busy. Named profiles
//! ([`FlashProfile::named`]) are written from FlashGBX's cart database. When
//! a program's commands miss this board, the chip records why as a
//! [`FlashNote`] ("$A9 is $AA with D0 and D1 swapped"), rather than failing
//! silently as hardware does.
//!
//! Not modeled: Intel command sets, CPLD multicart mappers, CFI queries,
//! erase suspend, read-while-write banks, and what an operation cut off by
//! the power leaves behind (it counts as done, with a note).

mod chip;
mod note;
mod profile;
mod profiles;
mod state;

pub use chip::FlashChip;
pub use note::{FlashNote, FlashNotes, NoteKind, Operation};
pub use profile::{BusyTicks, DataLines, FlashProfile, SectorRun, WritePin};
pub use state::FlashStateError;

/// The model's clock: ticks of 2^23 Hz, as `gbcli_core::Ticks` counts them.
pub const TICKS_PER_SECOND: u64 = 1 << 23;
