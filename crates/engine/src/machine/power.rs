//! The power cycle (design §6, the 2026-11-07 battery-save ruling): the
//! machine at power-on again, keeping what a cart keeps with the power off.

use gbcli_core::flash::contents_past;
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

/// Where a power cycle's flash chip contents come from, on a flash-save
/// cart; ignored on mask ROM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flash<'a> {
    /// The chip's own: a flash chip keeps its array with the power off.
    Keep,
    /// A [`Core::flash_contents`](gbcli_core::Core::flash_contents) from
    /// another core on the same board (the head's, across a rebuild). The
    /// sectors that overlap this machine's ROM file are left out: the image
    /// is this build's, and they are named in
    /// [`PowerCycled::flash_sectors_dropped`].
    Carried(&'a [u8]),
    /// A whole chip image, as [`Machine::flash_image`] exports it: every
    /// sector where it differs from the image as built becomes the chip's.
    Image(&'a [u8]),
}

/// What a power cycle carried across.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PowerCycled {
    /// The size of the battery save carried in, or `None` when the cart has
    /// no battery.
    pub battery_bytes: Option<usize>,
    pub clock: SavClock,
    /// The size of the flash image imported ([`Flash::Image`]).
    pub flash_bytes: Option<usize>,
    /// How many of the imported image's differing sectors overlap the ROM
    /// file's extent: the save is normally past it, and a difference there
    /// may be another build's code.
    pub flash_sectors_in_image: usize,
    /// The chip-address ranges of the carried sectors ([`Flash::Carried`])
    /// that overlap the ROM file's extent, which were not carried: the new
    /// build's code lies there, so the save no longer fits beside it.
    pub flash_sectors_dropped: Vec<std::ops::Range<u32>>,
}

impl Machine {
    /// Turns the machine off and on: a core built afresh from the same ROM,
    /// patches, setup and seed, at power-on (time 0, no boot handoff yet),
    /// with the battery's RAM and clock carried in, the clock then moved on
    /// by `rtc_advance` seconds. Held keys stay held. The clock's count
    /// within its current second restarts, as neither a `.sav` nor a battery
    /// state holds it. A flash-save cart's chip comes from `flash`: its own
    /// array by default (an operation the power cuts off counts as done,
    /// with a note). On any failure the machine is unchanged.
    pub fn power_cycle(
        &mut self,
        battery: Battery<'_>,
        flash: Flash<'_>,
        rtc_advance: u64,
    ) -> Result<PowerCycled, CoreError> {
        // Keeping the cart's own battery carries the core's state, not a
        // `.sav`: the import refuses HuC3 and TPP1 footers.
        let kept = match battery {
            Battery::Keep => self.core.battery_state()?,
            Battery::Carried(_) | Battery::Sav(_) => None,
        };
        // A flash chip keeps its array whatever happens to the battery.
        let own = match flash {
            Flash::Keep => self.core.flash_contents()?,
            Flash::Carried(_) | Flash::Image(_) => None,
        };
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
        let (mut flash_bytes, mut flash_sectors_in_image) = (None, 0);
        let mut flash_sectors_dropped = Vec::new();
        match (flash, own.as_deref()) {
            (Flash::Keep, Some(contents)) => core.load_flash_contents(contents)?,
            (Flash::Carried(contents), _) => {
                // Where the image lies is this build's code, however the
                // other build's program left those sectors.
                let split = contents_past(contents, self.rom.len() as u32).map_err(|e| {
                    CoreError::InvalidState {
                        reason: e.to_string(),
                    }
                })?;
                core.load_flash_contents(&split.kept)?;
                flash_sectors_dropped = split.dropped;
            }
            (Flash::Image(image), _) => {
                let changed = core.load_flash_image(image)?;
                let end = self.rom.len() as u32;
                flash_sectors_in_image = changed.iter().filter(|s| s.start < end).count();
                flash_bytes = Some(image.len());
            }
            (Flash::Keep, None) => {}
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
            flash_bytes,
            flash_sectors_in_image,
            flash_sectors_dropped,
        })
    }
}
