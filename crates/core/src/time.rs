//! Emulated time.

/// Emulated time in ticks of 2^23 Hz (8.388608 MHz), counted since power-on.
///
/// The finest clock that counts both CPU speeds as integers: one T-cycle is
/// two ticks at single speed and one at double speed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ticks(pub u64);

impl Ticks {
    /// Ticks per emulated second.
    pub const PER_SECOND: u64 = 1 << 23;
    /// Ticks in one 70,224 T-cycle frame at single speed.
    pub const PER_FRAME: u64 = 70_224 * 2;

    /// Emulated seconds, for reporting.
    pub fn as_secs_f64(self) -> f64 {
        self.0 as f64 / Self::PER_SECOND as f64
    }
}

impl std::ops::Add for Ticks {
    type Output = Ticks;
    fn add(self, rhs: Ticks) -> Ticks {
        Ticks(self.0 + rhs.0)
    }
}

impl std::ops::Sub for Ticks {
    type Output = Ticks;
    fn sub(self, rhs: Ticks) -> Ticks {
        Ticks(self.0 - rhs.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_convert_from_ticks() {
        assert_eq!(Ticks(Ticks::PER_SECOND * 3).as_secs_f64(), 3.0);
    }
}
