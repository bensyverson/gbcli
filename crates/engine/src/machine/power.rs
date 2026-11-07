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
    /// A [`Core::battery_state`](gbcli_core::Core::battery_state) from
    /// another core of this backend on the same cart (the head's, across a
    /// rebuild), in place of the cart's own.
    Carried(&'a [u8]),
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
    /// within its current second restarts, as neither a `.sav` nor a battery
    /// state holds it. A flash-save cart keeps its flash chip's array (an
    /// operation the power cuts off counts as done, with a note). On any
    /// failure the machine is unchanged.
    pub fn power_cycle(
        &mut self,
        battery: Battery<'_>,
        rtc_advance: u64,
    ) -> Result<PowerCycled, CoreError> {
        // Keeping the cart's own battery carries the core's state, not a
        // `.sav`: the import refuses HuC3 and TPP1 footers.
        let kept = match battery {
            Battery::Keep => self.core.battery_state()?,
            Battery::Carried(_) | Battery::Sav(_) => None,
        };
        // A flash chip keeps its array whatever happens to the battery.
        let flash = self.core.flash_contents()?;
        let mut core = (self.backend.build)(core_config(&self.setup, &self.rom))?;
        for patch in &self.patches {
            core.patch_rom(patch.offset, &patch.bytes)?;
        }
        let (clock, bytes) = match (battery, kept.as_deref()) {
            (Battery::Keep, None) => (SavClock::NotClocked, None),
            (Battery::Keep, Some(state)) | (Battery::Carried(state), _) => {
                (core.load_battery_state(state)?, Some(state.len()))
            }
            (Battery::Sav(sav), _) => (core.load_battery_sav(sav)?, Some(sav.len())),
        };
        if let Some(contents) = &flash {
            core.load_flash_contents(contents)?;
        }
        if rtc_advance > 0 {
            core.advance_rtc(rtc_advance)?;
        }
        core.set_keys(self.keys);
        self.core = core;
        self.boot = None;
        Ok(PowerCycled {
            battery_bytes: bytes,
            clock,
        })
    }
}
