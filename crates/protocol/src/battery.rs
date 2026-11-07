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
    /// cycle went into the ROM on disk carrying only the battery.
    pub rom_rebuilt: bool,
}

/// A `.sav` that was written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavMade {
    pub path: String,
    pub bytes: u64,
    /// The cart RAM's share of `bytes`; the rest is the clock footer.
    pub ram_bytes: u64,
}
