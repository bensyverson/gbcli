//! Battery saves in SameBoy's `.sav` format, read without host time (design
//! §6, the 2026-11-07 battery-save ruling).

use gbcli_core::{CartClock, CoreError, Feature, SavClock};
use sameboy_sys as sb;

use crate::SameBoy;

/// The MBC3 clock footers SameBoy reads: five 4-byte registers for the real
/// clock, five for the latched one, then a stamp of 8 bytes (BGB, newer VBA)
/// or 4 (older VBA).
const MBC3_FOOTERS: [usize; 2] = [48, 44];
const CLOCK_REGISTERS: usize = 5;
const OTHER_CLOCK_REASON: &str = "HuC3 and TPP1 clock footers aren't read yet; only MBC3's are";

impl SameBoy {
    pub(crate) fn export_battery(&mut self) -> Result<Option<Vec<u8>>, CoreError> {
        let gb = self.raw();
        // SAFETY: plain queries on a live instance outside GB_run.
        let size = unsafe { sb::GB_save_battery_size(gb) };
        if size <= 0 {
            return Ok(None);
        }
        let mut sav = vec![0u8; size as usize];
        // SAFETY: the buffer holds the size SameBoy asked for.
        let status = unsafe { sb::GB_save_battery_to_buffer(gb, sav.as_mut_ptr(), sav.len()) };
        if status != 0 {
            return Err(CoreError::Backend {
                message: format!("SameBoy couldn't write the battery save (error {status})"),
            });
        }
        Ok(Some(sav))
    }

    pub(crate) fn import_battery(&mut self, sav: &[u8]) -> Result<SavClock, CoreError> {
        let gb = self.raw();
        // SAFETY: plain queries on a live instance.
        let (kind, ram) = unsafe {
            (
                sb::gbcli_battery_kind(gb),
                sb::gbcli_cart_ram_size(gb) as usize,
            )
        };
        let footers: &[usize] = match kind {
            sb::GBCLI_BATTERY_NONE => {
                return Err(CoreError::InvalidState {
                    reason: "this cart has no battery, so it keeps nothing to import; give the \
                             ROM a cart type with a battery (rgbfix -m, e.g. MBC5+RAM+BATTERY)"
                        .into(),
                });
            }
            sb::GBCLI_BATTERY_OTHER_CLOCK => {
                return Err(CoreError::Unsupported {
                    feature: Feature::BatterySave,
                    reason: OTHER_CLOCK_REASON,
                });
            }
            sb::GBCLI_BATTERY_RAM_AND_MBC3_CLOCK => &MBC3_FOOTERS,
            _ => &[],
        };
        let footer = sav.len().checked_sub(ram);
        let clock = match footer {
            Some(0) if footers.is_empty() => SavClock::NotClocked,
            Some(0) => SavClock::FooterMissing,
            Some(n) if footers.contains(&n) => SavClock::Restored,
            _ => return Err(size_refused(sav.len(), ram, footers)),
        };
        let (real, latched) = match clock {
            SavClock::Restored => {
                let footer = &sav[ram..];
                let register = |i: usize| footer[4 * i];
                let real: [u8; CLOCK_REGISTERS] = std::array::from_fn(register);
                let latched: [u8; CLOCK_REGISTERS] =
                    std::array::from_fn(|i| register(CLOCK_REGISTERS + i));
                (Some(real), Some(latched))
            }
            _ => (None, None),
        };
        let ptr = |regs: &Option<[u8; CLOCK_REGISTERS]>| {
            regs.as_ref().map_or(std::ptr::null(), |r| r.as_ptr())
        };
        // SAFETY: `sav` holds at least `ram` bytes; the clock arrays are five
        // bytes each or both null, as the shim expects; outside GB_run.
        unsafe { sb::gbcli_load_battery(gb, sav.as_ptr(), ram, ptr(&real), ptr(&latched)) };
        // The file set every byte from outside the CPU, as a poke does.
        if let Some(tracking) = &mut self.hooks_mut().tracking {
            tracking.mark_poke(gbcli_core::Region::Sram, 0, ram);
        }
        Ok(clock)
    }

    pub(crate) fn advance_clock(&mut self, seconds: u64) -> Result<(), CoreError> {
        let Some(clock) = self.clock_kind() else {
            return Err(CoreError::InvalidState {
                reason: "this cart has no clock to advance (only MBC3 with a timer, HuC3 and \
                         TPP1 have one)"
                    .into(),
            });
        };
        if seconds > clock.max_advance_s() {
            return Err(CoreError::InvalidState {
                reason: format!(
                    "an advance of {seconds} s is past {} days ({} s), the most this cart's clock \
                     moves at once: {}",
                    clock.max_advance_days(),
                    clock.max_advance_s(),
                    clock.bound_reason()
                ),
            });
        }
        // SAFETY: a clock update outside GB_run.
        if !unsafe { sb::gbcli_advance_rtc(self.raw(), seconds) } {
            return Err(CoreError::InvalidState {
                reason: format!("the clock's count can't move another {seconds} s"),
            });
        }
        Ok(())
    }

    /// The cart's clock, from its mapper.
    pub(crate) fn clock_kind(&self) -> Option<CartClock> {
        // SAFETY: a plain query on a live instance.
        match unsafe { sb::gbcli_clock_kind(self.raw()) } {
            sb::GBCLI_CLOCK_MBC3 => Some(CartClock::Mbc3),
            sb::GBCLI_CLOCK_HUC3 => Some(CartClock::Huc3),
            sb::GBCLI_CLOCK_TPP1 => Some(CartClock::Tpp1),
            _ => None,
        }
    }

    pub(crate) fn export_battery_state(&mut self) -> Result<Option<Vec<u8>>, CoreError> {
        let gb = self.raw();
        // SAFETY: a plain query on a live instance outside GB_run.
        let size = unsafe { sb::GB_save_battery_size(gb) };
        if size <= 0 {
            return Ok(None);
        }
        let mut state = vec![0u8; size as usize];
        // SAFETY: the buffer is `state.len()` bytes; outside GB_run.
        if !unsafe { sb::gbcli_save_battery_state(gb, state.as_mut_ptr(), state.len()) } {
            return Err(CoreError::Backend {
                message: format!("the battery state of this cart doesn't fit {size} bytes"),
            });
        }
        Ok(Some(state))
    }

    pub(crate) fn import_battery_state(&mut self, state: &[u8]) -> Result<SavClock, CoreError> {
        let gb = self.raw();
        // SAFETY: plain queries, then a load of `state.len()` bytes, which
        // the shim checks against the cart; outside GB_run.
        let (kind, size, ram, loaded) = unsafe {
            let kind = sb::gbcli_battery_kind(gb);
            let size = sb::GB_save_battery_size(gb);
            let ram = sb::gbcli_cart_ram_size(gb) as usize;
            let loaded = sb::gbcli_load_battery_state(gb, state.as_ptr(), state.len());
            (kind, size, ram, loaded)
        };
        if !loaded {
            return Err(CoreError::InvalidState {
                reason: format!(
                    "a battery state of {} bytes doesn't fit this cart, whose state is {size}; \
                     carry it only between cores on the same cart",
                    state.len()
                ),
            });
        }
        if let Some(tracking) = &mut self.hooks_mut().tracking {
            tracking.mark_poke(gbcli_core::Region::Sram, 0, ram);
        }
        Ok(match kind {
            sb::GBCLI_BATTERY_RAM => SavClock::NotClocked,
            _ => SavClock::Restored,
        })
    }
}

/// A `.sav` whose size is neither the cart RAM alone nor the RAM and a footer.
fn size_refused(len: usize, ram: usize, footers: &[usize]) -> CoreError {
    let mut sizes = vec![format!("{ram} (the cart RAM alone)")];
    sizes.extend(
        footers
            .iter()
            .map(|f| format!("{} (with a {f}-byte clock footer)", ram + f)),
    );
    CoreError::InvalidState {
        reason: format!(
            "a .sav of {len} bytes fits no layout of this cart; it takes {}",
            sizes.join(" or ")
        ),
    }
}

#[cfg(test)]
mod tests {
    use gbcli_core::{Core, CoreConfig, Model, Region};

    use super::*;

    /// An MBC3 cart with a clock and 8 KiB of battery RAM.
    fn mbc3_clock_cart() -> SameBoy {
        let mut rom = vec![0u8; 0x8000];
        rom[0x147] = 0x10;
        rom[0x149] = 0x02;
        SameBoy::new(CoreConfig::new(Model::DmgB, rom)).unwrap()
    }

    #[test]
    fn an_export_is_what_sameboys_own_save_battery_writes_but_the_stamp() {
        let mut core = mbc3_clock_cart();
        core.poke(Region::Sram, 0x10, &[1, 2, 3, 4]).unwrap();
        let ours = core.battery_sav().unwrap().unwrap();
        let dir = std::env::temp_dir().join(format!("gbcli-sav-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("own.sav");
        let c_path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
        // SAFETY: a live instance outside GB_run and a NUL-terminated path.
        let status = unsafe { sb::GB_save_battery(core.raw(), c_path.as_ptr()) };
        assert_eq!(status, 0);
        let theirs = std::fs::read(&path).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(ours.len(), theirs.len());
        let stamp = ours.len() - 8;
        assert_eq!(ours[..stamp], theirs[..stamp]);
    }
}
