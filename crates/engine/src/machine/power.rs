//! The power cycle (design §6, the 2026-11-07 battery-save ruling): the
//! machine at power-on again, keeping what a cart keeps with the power off.

use gbcli_core::{CoreError, SavClock};

use super::{Machine, core_config};

/// Where a power cycle's battery-backed RAM and clock come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Battery<'a> {
    /// The cart's own, as the power went off. A cart without a battery keeps
    /// nothing, as on hardware.
    Keep,
    /// A `.sav` in [`Core::battery_sav`](gbcli_core::Core::battery_sav)'s
    /// format, in place of the cart's own.
    Sav(&'a [u8]),
}

/// What a power cycle carried across.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PowerCycled {
    /// The size of the battery save carried in, or `None` when the cart has
    /// no battery.
    pub battery_bytes: Option<usize>,
    pub clock: SavClock,
}

impl Machine {
    /// Turns the machine off and on: a core built afresh from the same ROM,
    /// patches, setup and seed, at power-on (time 0, no boot handoff yet),
    /// with the battery's RAM and clock carried in, the clock then moved on
    /// by `rtc_advance` seconds. Held keys stay held. The clock's count
    /// within its current second restarts, as a `.sav` doesn't hold it. On
    /// any failure the machine is unchanged.
    pub fn power_cycle(
        &mut self,
        battery: Battery<'_>,
        rtc_advance: u64,
    ) -> Result<PowerCycled, CoreError> {
        let sav = match battery {
            Battery::Keep => self.core.battery_sav()?,
            Battery::Sav(bytes) => Some(bytes.to_vec()),
        };
        let mut core = (self.backend.build)(core_config(&self.setup, &self.rom))?;
        for patch in &self.patches {
            core.patch_rom(patch.offset, &patch.bytes)?;
        }
        let clock = match &sav {
            Some(bytes) => core.load_battery_sav(bytes)?,
            None => SavClock::NotClocked,
        };
        if rtc_advance > 0 {
            core.advance_rtc(rtc_advance)?;
        }
        core.set_keys(self.keys);
        self.core = core;
        self.boot = None;
        Ok(PowerCycled {
            battery_bytes: sav.as_ref().map(Vec::len),
            clock,
        })
    }
}
