//! Battery saves on the wire (design §6, the 2026-11-07 battery-save
//! ruling): `power_cycle`, which may import a `.sav`, and `sav`, which
//! exports one.

use serde::{Deserialize, Serialize};

/// `{"op":"power_cycle","args":{…}}`: the machine at power-on again, keeping
/// the cart's battery-backed RAM and clock. It saves a new head.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PowerCycleArgs {
    /// A `.sav` file to carry in instead of the cart's own battery.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sav: Option<String>,
    /// Moves the cart's clock on while the power is off: a whole number and
    /// a unit, `45s`, `30m`, `12h` or `7d` (the `prune` grammar).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rtc_advance: Option<String>,
}

/// `{"op":"sav","args":{"path":…}}`: writes the head's battery save. A read
/// op: the head is unchanged.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavArgs {
    pub path: String,
}

/// Where a power cycle's battery came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatteryFrom {
    /// The head's own cart.
    Head,
    /// The `.sav` named in the request.
    Sav,
    /// Nowhere: the cart has no battery, so its RAM was lost.
    None,
}

/// What a battery save did to the cart's clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SavClockState {
    /// The cart has no clock.
    NotClocked,
    /// The clock is the save's, with no host time added.
    Restored,
    /// The save had no clock footer: the clock starts at zero, with the day
    /// counter's carry bit set to tell the game to set it.
    FooterMissing,
}

/// A power cycle's result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerCycleMade {
    pub battery: BatteryFrom,
    /// The size of the battery save carried in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    pub clock: SavClockState,
    /// Seconds the clock was moved on.
    pub rtc_advanced_s: u64,
    /// Whether the head was saved over another build of the ROM, so the
    /// cycle went into the ROM on disk carrying only the battery (and, on a
    /// flash session, the flash sectors the program changed).
    pub rom_rebuilt: bool,
    /// On a flash session, where the chip's contents came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flash: Option<FlashFrom>,
    /// The size of the chip image imported, with `flash: image`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flash_bytes: Option<u64>,
}

/// A `.sav` that was written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavMade {
    /// The `.sav`'s path; absent when the cart has no battery RAM, so only
    /// its flash image was written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub bytes: u64,
    /// The cart RAM's share of `bytes`; the rest is the clock footer.
    pub ram_bytes: u64,
    /// On a flash session, the chip image written beside the `.sav`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flash: Option<FlashFileMade>,
}

/// A flash chip image file: the whole chip as the CPU reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlashFileMade {
    pub path: String,
    pub bytes: u64,
}

/// Where a power cycle's flash chip contents came from, on a flash session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlashFrom {
    /// The head's own chip (across a rebuild, the sectors it changed).
    Head,
    /// The chip image beside the `.sav` named in the request.
    Image,
}

/// Why the flash model took a note (`gbcli_flash::NoteKind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlashNoteKind {
    DataLines,
    UnlockAddress,
    WritePinAudio,
    BrokenSequence,
    ProgramSetsBits,
    WriteWhileBusy,
    CutOff,
    /// A mask-ROM cart saw a board's unlock cycle; `board` names it.
    NoChip,
}

/// One flash note, as a run reports it: when, the write, why, and the
/// sentence its warning row carries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlashNoteRow {
    /// Emulated time, in ticks of 2^23 Hz since power-on.
    pub ticks: u64,
    pub addr: u16,
    pub value: u8,
    pub kind: FlashNoteKind,
    /// The board the write unlocks, for `no_chip`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<String>,
    pub text: String,
}
