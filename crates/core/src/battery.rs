//! Battery-backed cart state as a `.sav` file (design §6, the 2026-11-07
//! battery-save ruling).

/// The longest a cart clock is moved on at once
/// ([`Core::advance_rtc`](crate::Core::advance_rtc)): 1024 days. An MBC3
/// clock counts days in 9 bits with a carry that stays set once the count
/// wraps, so past 512 days the carry is always set and an advance reads the
/// same as one 512 days shorter: every reading an advance can give is
/// reached by one of at most 1024 days. The advance also costs host time per
/// emulated day (per minute on HuC3), so an unbounded one could hold the
/// session for minutes.
pub const MAX_RTC_ADVANCE_S: u64 = 1024 * 24 * 60 * 60;

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
