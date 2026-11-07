//! A flash-save cart on SameBoy's memory callbacks (`gbcli_flash`).
//!
//! SameBoy's write callback fires before its mapper sees a write, and lets it
//! through, so every CPU write to $0000-$7FFF reaches both the chip and the
//! MBC, as on the board. Its read callback replaces the ROM byte while the
//! chip answers with status or IDs. The chip's array is SameBoy's own ROM
//! buffer, so programming and erasing change what the CPU and inspection
//! read. Unlike a filter's, these callbacks stay installed for the core's
//! life (`Hooks::read_hook_wanted`).

use gbcli_core::flash::{FlashChip, FlashNotes};
use gbcli_core::{CoreError, Feature};
use sameboy_sys as sb;

use crate::SameBoy;
use crate::hooks::Hooks;

/// Emulated time at the access being made: the step's start plus the
/// cycles SameBoy has run of it.
fn now(gb: *mut sb::GB_gameboy_t, hooks: &Hooks) -> u64 {
    if hooks.in_run {
        // SAFETY: a plain read of the live instance, inside its callback.
        hooks.step_ticks + u64::from(unsafe { sb::gbcli_cycles_since_run(gb) })
    } else {
        hooks.step_ticks
    }
}

/// SameBoy's ROM buffer: the chip's array, in the bytes the CPU reads.
fn rom<'a>(gb: *mut sb::GB_gameboy_t) -> &'a mut [u8] {
    let (mut size, mut bank) = (0usize, 0u16);
    // SAFETY: GB_get_direct_access only reads the instance and fills the
    // outputs; the buffer lives as long as the instance, and nothing else
    // holds a reference into it while a callback or `&mut SameBoy` runs.
    unsafe {
        let ptr = sb::GB_get_direct_access(gb, sb::GB_DIRECT_ACCESS_ROM, &mut size, &mut bank);
        if ptr.is_null() {
            return &mut [];
        }
        std::slice::from_raw_parts_mut(ptr.cast::<u8>(), size)
    }
}

/// The chip address of a CPU access: the mapped bank's base plus the offset.
fn chip_addr(gb: *mut sb::GB_gameboy_t, addr: u16) -> u32 {
    // SAFETY: a side-effect-free accessor on the live instance.
    let bank = u32::from(unsafe { sb::gbcli_bank_for(gb, addr) });
    bank * 0x4000 + u32::from(addr & 0x3FFF)
}

/// A CPU write that landed at `addr`; the caller lets it on to the MBC.
pub(crate) fn on_write(gb: *mut sb::GB_gameboy_t, hooks: &mut Hooks, addr: u16, data: u8) {
    if addr >= 0x8000 {
        return;
    }
    let at = now(gb, hooks);
    if let Some(chip) = &mut hooks.flash {
        chip.write(at, addr, chip_addr(gb, addr), data, rom(gb));
    }
}

/// A CPU read of `addr` that returned `data` from the ROM.
pub(crate) fn on_read(gb: *mut sb::GB_gameboy_t, hooks: &mut Hooks, addr: u16, data: u8) -> u8 {
    // While the boot ROM is mapped, its bytes are not the chip's.
    // SAFETY: a plain read of the live instance, inside its callback.
    if addr >= 0x8000 || unsafe { sb::gbcli_reads_boot_rom(gb, addr) } {
        return data;
    }
    let at = now(gb, hooks);
    match &mut hooks.flash {
        Some(chip) => chip.read(at, chip_addr(gb, addr), data),
        None => data,
    }
}

impl SameBoy {
    /// Puts a flash chip on the cart and installs both callbacks for good.
    pub(crate) fn attach_flash(&mut self, chip: FlashChip) {
        let gb = self.raw();
        self.hooks_mut().flash = Some(chip);
        // SAFETY: outside GB_run; the callbacks reach the hooks through user data.
        unsafe {
            sb::GB_set_read_memory_callback(gb, Some(crate::watch::on_read));
            sb::GB_set_write_memory_callback(gb, Some(crate::watch::on_write));
        }
    }

    pub(crate) fn flash_save(&self) -> Option<Vec<u8>> {
        let rom = rom(self.raw());
        self.hooks().flash.as_ref().map(|chip| chip.save(rom))
    }

    /// Loads a blob's flash section, or refuses a blob whose cart differs.
    pub(crate) fn flash_load(&mut self, state: Option<&[u8]>) -> Result<(), CoreError> {
        let gb = self.raw();
        match (&mut self.hooks_mut().flash, state) {
            (None, None) => Ok(()),
            (Some(chip), Some(state)) => chip.load(state, rom(gb)).map_err(invalid),
            (Some(_), None) => Err(CoreError::InvalidState {
                reason: "the state is from a mask-ROM cart, and this one has a flash chip".into(),
            }),
            (None, Some(_)) => Err(CoreError::InvalidState {
                reason: "the state is from a flash-save cart, and this one is mask ROM".into(),
            }),
        }
    }

    pub(crate) fn export_flash(&mut self) -> Option<Vec<u8>> {
        let (gb, ticks) = (self.raw(), self.ticks);
        let rom = rom(gb);
        self.hooks()
            .flash
            .as_ref()
            .map(|chip| chip.contents(ticks, rom))
    }

    pub(crate) fn import_flash(&mut self, contents: &[u8]) -> Result<(), CoreError> {
        let gb = self.raw();
        match &mut self.hooks_mut().flash {
            Some(chip) => chip.power_on(contents, rom(gb)).map_err(invalid),
            None => Err(CoreError::Unsupported {
                feature: Feature::FlashCart,
                reason: "this cart is mask ROM: it was built without a flash profile",
            }),
        }
    }

    pub(crate) fn flash_notes(&mut self) -> FlashNotes {
        self.hooks_mut()
            .flash
            .as_mut()
            .map(FlashChip::take_notes)
            .unwrap_or_default()
    }
}

fn invalid(err: gbcli_core::flash::FlashStateError) -> CoreError {
    CoreError::InvalidState {
        reason: err.to_string(),
    }
}
