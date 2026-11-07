//! Battery-backed cart state as a `.sav` file (design §6, the 2026-11-07
//! battery-save ruling).

const DAY_S: u64 = 24 * 60 * 60;

/// The kind of clock a cart carries, which decides how far one advance
/// ([`Core::advance_rtc`](crate::Core::advance_rtc)) can move it: to
/// [`max_advance_s`](CartClock::max_advance_s), the shortest advance after
/// which the clock shows nothing new, so no reading is out of reach of one
/// advance. The advance also costs host time per emulated day (per minute on
/// HuC3), so an unbounded one could hold the session for minutes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CartClock {
    /// MBC3's clock: days in 9 bits, then a carry flag that stays set.
    Mbc3,
    /// HuC3's clock: minutes of the day, and days in 16 bits with no flag.
    Huc3,
    /// TPP1's clock: weeks in 8 bits, then MR4's overflow flag, which stays
    /// set.
    Tpp1,
}

impl CartClock {
    /// The longest advance in days: past it a reading repeats one a shorter
    /// advance gives.
    pub const fn max_advance_days(self) -> u64 {
        match self {
            // From day 0, 512 days set the carry and 512 more reach every
            // count again with it set.
            CartClock::Mbc3 => 2 * 512,
            // The same for 256 weeks and MR4's flag.
            CartClock::Tpp1 => 2 * 256 * 7,
            // No flag: 65536 days come back to the same reading.
            CartClock::Huc3 => 65_536,
        }
    }

    /// [`max_advance_days`](CartClock::max_advance_days) in seconds.
    pub const fn max_advance_s(self) -> u64 {
        self.max_advance_days() * DAY_S
    }

    /// Why the bound is where it is, in this cart's own terms.
    pub const fn bound_reason(self) -> &'static str {
        match self {
            CartClock::Mbc3 => {
                "an MBC3 clock counts 512 days and then only sets its day counter's carry flag, \
                 so every reading it can show is reached within 1024 days"
            }
            CartClock::Tpp1 => {
                "a TPP1 clock counts 256 weeks (1792 days) and then only sets MR4's overflow \
                 flag, so every reading it can show is reached within 3584 days"
            }
            CartClock::Huc3 => {
                "a HuC3 clock counts days in 16 bits and wraps with no flag, so every reading it \
                 can show is reached within 65536 days"
            }
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::CartClock;

    const ALL: [(CartClock, &str); 3] = [
        (CartClock::Mbc3, "MBC3"),
        (CartClock::Huc3, "HuC3"),
        (CartClock::Tpp1, "TPP1"),
    ];

    /// Review of e65e509: the bound was 1024 days for every cart, explained
    /// in MBC3's terms, though TPP1's weeks and HuC3's days count further.
    #[test]
    fn each_clock_is_bounded_where_its_readings_start_to_repeat() {
        assert_eq!(CartClock::Mbc3.max_advance_days(), 2 * 512);
        assert_eq!(CartClock::Tpp1.max_advance_days(), 2 * 256 * 7);
        assert_eq!(CartClock::Huc3.max_advance_days(), 65_536);
        assert_eq!(CartClock::Tpp1.max_advance_s(), 3584 * 86_400);
    }

    #[test]
    fn each_bound_is_explained_in_its_own_carts_terms() {
        for (clock, name) in ALL {
            let reason = clock.bound_reason();
            assert!(reason.contains(name), "{clock:?}: {reason}");
            let days = format!("{} days", clock.max_advance_days());
            assert!(reason.contains(&days), "{clock:?}: {reason}");
            for (_, other) in ALL.iter().filter(|(c, _)| *c != clock) {
                assert!(!reason.contains(other), "{clock:?}: {reason}");
            }
        }
    }
}
