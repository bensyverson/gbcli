//! Battery-backed cart state as a `.sav` file (design §6, the 2026-11-07
//! battery-save ruling).

/// What an imported `.sav` did to the cart's clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SavClock {
    /// The cart has no clock.
    NotClocked,
    /// The clock holds the file's clock exactly: the footer's wall-clock
    /// stamp is ignored, so no host time passes on import.
    Restored,
    /// The cart has a clock but the file has no clock footer (a RAM-only
    /// `.sav`): the clock is the fresh core's, zero, with its day-counter carry
    /// bit set, the hint SameBoy gives a game that its clock needs setting.
    FooterMissing,
}
