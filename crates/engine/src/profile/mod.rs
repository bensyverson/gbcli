//! The cycle profiler: exact emulated cycle costs between two points
//! (`measure`), per routine by `.sym` (`profile`), per frame, and the raster
//! timeline (`raster`). Design §11.

mod frames;
mod measure;
mod raster;
mod routines;

pub use frames::{FrameBudget, FrameSplit};
pub use measure::{Leg, MeasureError, MeasureSpec, Measurement, Stats};
pub use raster::{ModeChange, Raster, RasterLine, RasterTimeline, RasterWrite};
pub use routines::{Entry, Profile, RoutineCost, Routines, ShadowStats};

use gbcli_core::{Core, Ticks};

/// The CPU clock a span ran at; T-cycles are counted at that clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CpuSpeed {
    Single,
    /// CGB double speed: a T-cycle is half as long.
    Double,
}

impl CpuSpeed {
    pub fn of(core: &dyn Core) -> CpuSpeed {
        if core.registers().double_speed {
            CpuSpeed::Double
        } else {
            CpuSpeed::Single
        }
    }

    /// Ticks (2^23 Hz) per CPU T-cycle.
    fn ticks_per_t(self) -> u64 {
        match self {
            CpuSpeed::Single => 2,
            CpuSpeed::Double => 1,
        }
    }
}

/// A stretch of emulated time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub ticks: Ticks,
}

impl Span {
    /// CPU T-cycles at `speed`.
    pub fn t(&self, speed: CpuSpeed) -> u64 {
        self.ticks.0 / speed.ticks_per_t()
    }

    /// CPU M-cycles (four T) at `speed`.
    pub fn m(&self, speed: CpuSpeed) -> u64 {
        self.t(speed) / 4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_t_cycle_is_two_ticks_at_single_speed_and_one_at_double() {
        let span = Span { ticks: Ticks(48) };
        assert_eq!(span.t(CpuSpeed::Single), 24);
        assert_eq!(span.m(CpuSpeed::Single), 6);
        assert_eq!(span.t(CpuSpeed::Double), 48);
        assert_eq!(span.m(CpuSpeed::Double), 12);
    }

    #[test]
    fn a_frame_of_ticks_is_70224_t_at_single_speed() {
        let frame = Span {
            ticks: Ticks(Ticks::PER_FRAME),
        };
        assert_eq!(frame.t(CpuSpeed::Single), 70_224);
        assert_eq!(frame.m(CpuSpeed::Single), 17_556);
    }
}
