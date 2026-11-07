//! Address filters and never-written tracking on SameBoy's memory hooks.
//!
//! Known gaps, documented rather than papered over (`Feature` table):
//! - the write callback misses OAM DMA and HDMA destination writes. It fires
//!   before SameBoy's OAM DMA bus-conflict redirect, so the shim's
//!   `gbcli_write_target` names where a CPU write will land, and hits, store
//!   hits and tracking all use that byte (a write the conflict drops is none);
//! - the read callback also fires for SameBoy's own "safe" reads (we never
//!   make any: inspection uses direct access and the shim's peeks);
//! - execution filters are checked by the run loop, not the execution
//!   callback (which can't veto and skips interrupt dispatch), so a
//!   breakpoint on an interrupt vector works.
//!
//! Banks come from our own shim (`gbcli_bank_for`), not SameBoy's debugger,
//! so its banked SRAM/VRAM watchpoint bug (it treats 8000-CFFF as bank 0)
//! doesn't apply.

use gbcli_core::{Access, AddrFilter, Hit, Hits, Region, StoreFilter, WrittenView, canonical_addr};
use sameboy_sys as sb;

use crate::hooks::{Hooks, Wake, guard};

/// The filters of the run in progress. Raw pointers: they borrow from the
/// `RunRequest`, and the run loop clears them before it returns.
#[derive(Clone, Copy)]
pub(crate) struct Armed {
    pub read: *const AddrFilter,
    pub write: *const AddrFilter,
    pub veto: *const AddrFilter,
    pub stores: *const StoreFilter,
}

/// One region's bits: set by the CPU, set by pokes.
#[derive(Clone, Debug, Default)]
struct Bits {
    len: usize,
    cpu: Vec<u64>,
    poked: Vec<u64>,
}

impl Bits {
    fn new(len: usize) -> Self {
        let words = len.div_ceil(64);
        Bits {
            len,
            cpu: vec![0; words],
            poked: vec![0; words],
        }
    }
}

#[inline(always)]
fn set(words: &mut [u64], offset: usize) {
    if let Some(word) = words.get_mut(offset / 64) {
        *word |= 1 << (offset % 64);
    }
}

/// Never-written tracking for WRAM, HRAM and cart RAM (design §9).
#[derive(Clone, Debug)]
pub(crate) struct Tracking {
    wram: Bits,
    hram: Bits,
    sram: Bits,
}

impl Tracking {
    pub fn new(wram_len: usize, hram_len: usize, sram_len: usize) -> Self {
        Tracking {
            wram: Bits::new(wram_len),
            hram: Bits::new(hram_len),
            sram: Bits::new(sram_len),
        }
    }

    fn bits(&self, region: Region) -> Option<&Bits> {
        match region {
            Region::Wram => Some(&self.wram),
            Region::Hram => Some(&self.hram),
            Region::Sram => Some(&self.sram),
            _ => None,
        }
    }

    pub fn view(&self, region: Region) -> Option<WrittenView<'_>> {
        self.bits(region).map(|bits| WrittenView {
            len: bits.len,
            cpu: &bits.cpu,
            poked: &bits.poked,
        })
    }

    /// Marks a CPU write that landed at `store`.
    #[inline(always)]
    pub fn mark_cpu(&mut self, (region, offset): (Region, usize)) {
        let bits = match region {
            Region::Wram => &mut self.wram,
            Region::Hram => &mut self.hram,
            Region::Sram => &mut self.sram,
            _ => return,
        };
        set(&mut bits.cpu, offset);
    }

    /// Marks poked bytes; regions that aren't tracked are ignored.
    pub fn mark_poke(&mut self, region: Region, offset: usize, len: usize) {
        let bits = match region {
            Region::Wram => &mut self.wram,
            Region::Hram => &mut self.hram,
            Region::Sram => &mut self.sram,
            _ => return,
        };
        for i in offset..offset + len {
            set(&mut bits.poked, i);
        }
    }

    /// Every bitmap, in a fixed order, for the state blob.
    pub fn words(&self) -> Vec<u64> {
        [&self.wram, &self.hram, &self.sram]
            .iter()
            .flat_map(|b| b.cpu.iter().chain(&b.poked).copied())
            .collect()
    }

    /// Restores from `words`; false (and unchanged) if the sizes don't match.
    pub fn restore(&mut self, words: &[u64]) -> bool {
        if words.len() != self.words().len() {
            return false;
        }
        let mut rest = words;
        for bits in [&mut self.wram, &mut self.hram, &mut self.sram] {
            for half in [&mut bits.cpu, &mut bits.poked] {
                let (head, tail) = rest.split_at(half.len());
                half.copy_from_slice(head);
                rest = tail;
            }
        }
        true
    }
}

/// The backing byte a CPU write at a canonical address lands in, as
/// SameBoy's write path computes it: the identity store filters and
/// never-written tracking share. `None` for memory without a store we track
/// (ROM, VRAM, OAM, IO) and for a cart RAM write that reaches no RAM.
#[inline(always)]
pub(crate) fn store_of(gb: *mut sb::GB_gameboy_t, addr: u16) -> Option<(Region, usize)> {
    match addr {
        0xC000..=0xCFFF => Some((Region::Wram, usize::from(addr - 0xC000))),
        0xD000..=0xDFFF => {
            // SAFETY: a side-effect-free accessor on the live instance.
            let bank = usize::from(unsafe { sb::gbcli_bank_for(gb, addr) });
            Some((Region::Wram, bank * 0x1000 + usize::from(addr - 0xD000)))
        }
        0xFF80..=0xFFFE => Some((Region::Hram, usize::from(addr - 0xFF80))),
        0xFFFF => Some((Region::Ie, 0)),
        0xA000..=0xBFFF => {
            // SAFETY: as above.
            let offset = unsafe { sb::gbcli_cart_ram_offset(gb, addr) };
            usize::try_from(offset).ok().map(|o| (Region::Sram, o))
        }
        _ => None,
    }
}

/// Adds a hit to the instruction's hits.
fn record(hooks: &mut Hooks, hit: Hit) {
    match &mut hooks.hit {
        Some(hits) => hits.push(hit),
        None => hooks.hit = Some(Hits::one(hit)),
    }
}

fn peek(gb: *mut sb::GB_gameboy_t, addr: u16) -> Option<u8> {
    // SAFETY: a side-effect-free peek on the live instance.
    u8::try_from(unsafe { sb::gbcli_peek(gb, addr) }).ok()
}

pub(crate) unsafe extern "C" fn on_read(gb: *mut sb::GB_gameboy_t, addr: u16, data: u8) -> u8 {
    guard(gb, data, |hooks| {
        // An OAM DMA read is not the CPU's, and DMA runs during HALT.
        // SAFETY: plain reads of the live instance from inside its callback.
        if hooks.wake == Some(Wake::Halted) && !unsafe { sb::gbcli_in_dma_read(gb) } {
            // SAFETY: as above.
            hooks.wake = Some(Wake::At(unsafe { sb::gbcli_cycles_since_run(gb) }));
        }
        let Some(armed) = hooks.armed else {
            return data;
        };
        let addr = canonical_addr(addr);
        // SAFETY: armed filters outlive the run that armed them.
        let read = unsafe { &*armed.read };
        if read.may_match(addr) {
            // SAFETY: a side-effect-free accessor.
            let bank = unsafe { sb::gbcli_bank_for(gb, addr) };
            if read.matches(addr, bank) {
                let pc = hooks.step_pc;
                record(
                    hooks,
                    Hit {
                        access: Access::Read,
                        addr,
                        bank,
                        old: Some(data),
                        new: data,
                        pc,
                        vetoed: false,
                    },
                );
            }
        }
        data
    })
}

pub(crate) unsafe extern "C" fn on_write(gb: *mut sb::GB_gameboy_t, addr: u16, data: u8) -> bool {
    guard(gb, true, |hooks| {
        // Hits, stores and tracking all name the byte the write lands in,
        // which OAM DMA's bus conflict can move after this callback returns.
        // SAFETY: a side-effect-free accessor on the live instance.
        let Ok(addr) = u16::try_from(unsafe { sb::gbcli_write_target(gb, addr) }) else {
            return true; // it changes no memory we watch or track
        };
        let addr = canonical_addr(addr);
        // Computed at most once per write, and only when something needs it.
        let mut store = None;
        if let Some(armed) = hooks.armed {
            // SAFETY: armed filters outlive the run that armed them.
            let (write, veto, stores) = unsafe { (&*armed.write, &*armed.veto, &*armed.stores) };
            let mut matched = false;
            if write.may_match(addr) || veto.may_match(addr) {
                // SAFETY: a side-effect-free accessor.
                let bank = unsafe { sb::gbcli_bank_for(gb, addr) };
                let vetoed = veto.matches(addr, bank);
                if vetoed || write.matches(addr, bank) {
                    matched = true;
                    let pc = hooks.step_pc;
                    record(hooks, write_hit(gb, addr, bank, data, pc, vetoed));
                    if vetoed {
                        return false;
                    }
                }
            }
            if !stores.is_empty() {
                store = store_of(gb, addr);
                if !matched && store.is_some_and(|(r, o)| stores.contains(r, o)) {
                    // SAFETY: a side-effect-free accessor.
                    let bank = unsafe { sb::gbcli_bank_for(gb, addr) };
                    let pc = hooks.step_pc;
                    record(hooks, write_hit(gb, addr, bank, data, pc, false));
                }
            }
        }
        crate::audio::log_write(gb, hooks, addr, data);
        crate::raster::log_write(gb, hooks, addr, data);
        if let Some(tracking) = &mut hooks.tracking
            && let Some(store) = store.or_else(|| store_of(gb, addr))
        {
            tracking.mark_cpu(store);
        }
        true
    })
}

fn write_hit(
    gb: *mut sb::GB_gameboy_t,
    addr: u16,
    bank: u16,
    data: u8,
    pc: u16,
    vetoed: bool,
) -> Hit {
    Hit {
        access: Access::Write,
        addr,
        bank,
        old: peek(gb, addr),
        new: data,
        pc,
        vetoed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pokes_mark_only_tracked_regions_and_stay_in_bounds() {
        let mut t = Tracking::new(0x2000, 0x7F, 0);
        t.mark_poke(Region::Wram, 0x1FFE, 2);
        t.mark_poke(Region::Hram, 0x7E, 1);
        t.mark_poke(Region::Sram, 0, 4); // no cart RAM: ignored, no panic
        t.mark_poke(Region::Vram, 0, 4);
        assert!(t.view(Region::Wram).unwrap().was_poked(0x1FFF));
        assert!(t.view(Region::Hram).unwrap().was_poked(0x7E));
        assert!(t.view(Region::Vram).is_none());
    }

    #[test]
    fn bitmaps_round_trip_through_words() {
        let mut t = Tracking::new(0x2000, 0x7F, 0x8000);
        t.mark_poke(Region::Sram, 0x4000, 1);
        t.mark_poke(Region::Hram, 3, 1);
        let words = t.words();
        let mut u = Tracking::new(0x2000, 0x7F, 0x8000);
        assert!(u.restore(&words));
        assert!(u.view(Region::Sram).unwrap().was_poked(0x4000));
        assert!(u.view(Region::Hram).unwrap().was_poked(3));
        assert!(!u.restore(&words[1..]));
    }
}
