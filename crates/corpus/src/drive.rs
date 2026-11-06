//! Driving a core over the `Core` trait: boot, run to an exit, capture serial.
//!
//! Serial output is captured without a backend callback: writes to SB (FF01)
//! and SC (FF02) are write candidates, and a write to SC that starts an
//! internally clocked transfer sends the last byte written to SB. (Reading SB
//! after the SC write is too late: the first bit may already have shifted.)

use gbcli_core::{
    Access, AddrFilter, BankSel, Budget, Core, Region, RunRequest, StopReason, Ticks,
};

/// Ticks per emulated second (2^23 Hz).
pub const TICKS_PER_SECOND: u64 = Ticks::PER_SECOND;
/// Ticks per frame: 70,224 dots of 2 ticks.
pub const TICKS_PER_FRAME: u64 = Ticks::PER_FRAME;
/// The LD B,B opcode: mooneye's software breakpoint.
pub const LD_B_B: u8 = 0x40;
/// The boot ROM hands over to the cartridge here.
const ENTRY: u16 = 0x0100;
const SB: u16 = 0xFF01;
const SC: u16 = 0xFF02;
/// SC bits 7 (start) and 0 (internal clock): a transfer this side drives.
const SC_START_INTERNAL: u8 = 0x81;
/// A boot that takes longer than this is broken.
const BOOT_LIMIT_S: u64 = 10;

/// Why [`Driver::advance`] returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// The next instruction is LD B,B.
    LdBB,
    /// A serial byte was sent (it is in [`Driver::serial`]).
    Serial,
    /// The tick budget ran out.
    Budget,
    /// The CPU is locked on an illegal opcode.
    Locked(u8),
    /// The CPU is in STOP with no wake source.
    Stopped,
}

/// A core plus what the runner observes of it.
pub struct Driver {
    pub core: Box<dyn Core>,
    /// Every byte sent on the link port since power-on.
    pub serial: Vec<u8>,
    /// Ticks at hand-over to the cartridge; exits and timeouts count from here.
    pub start: u64,
    /// Ticks the runner skipped while the core was in STOP (see `idle_until`).
    idle: u64,
    /// SB and SC.
    sc: AddrFilter,
    /// The last byte the CPU wrote to SB.
    sb: u8,
    /// LD B,B candidates in ROM; RAM is added by [`Driver::exec_filter`].
    rom_ld_b_b: AddrFilter,
    /// ROM plus RAM candidates, and the tick their RAM part was scanned at.
    ld_b_b: AddrFilter,
    scanned_at: Option<u64>,
}

impl Driver {
    /// Wraps a freshly built core and runs its boot ROM to the hand-over at $0100.
    pub fn boot(core: Box<dyn Core>, rom: &[u8]) -> Result<Driver, String> {
        let mut sc = AddrFilter::new();
        sc.insert(SB, BankSel::Any);
        sc.insert(SC, BankSel::Any);
        let mut driver = Driver {
            core,
            serial: Vec::new(),
            start: 0,
            idle: 0,
            sc,
            sb: 0,
            rom_ld_b_b: ld_b_b_candidates(rom),
            ld_b_b: AddrFilter::new(),
            scanned_at: None,
        };
        let mut entry = AddrFilter::new();
        entry.insert(ENTRY, BankSel::Any);
        let mut req = RunRequest::new(Budget::ticks(BOOT_LIMIT_S * TICKS_PER_SECOND));
        req.exec = &entry;
        let stop = driver.core.run(&mut req).map_err(|e| e.to_string())?;
        if !matches!(stop.reason, StopReason::Candidate(_)) {
            return Err(format!(
                "the boot ROM didn't reach $0100: {:?}",
                stop.reason
            ));
        }
        driver.start = driver.core.now().0;
        Ok(driver)
    }

    /// Emulated ticks since hand-over, plus any time spent idling in STOP.
    pub fn elapsed(&self) -> u64 {
        self.core.now().0 - self.start + self.idle
    }

    /// Lets the clock the runner judges by jump to `at` while the core sits in
    /// STOP, which a core can't run through (it stops before the instruction).
    pub fn idle_until(&mut self, at: u64) {
        self.idle += at.saturating_sub(self.elapsed());
    }

    /// Runs until `until` ticks after hand-over, returning early on LD B,B
    /// (when `watch_ld_b_b`), a serial byte, or a CPU lock.
    pub fn advance(&mut self, until: u64, watch_ld_b_b: bool) -> Result<Event, String> {
        loop {
            let left = until.saturating_sub(self.elapsed());
            if left == 0 {
                return Ok(Event::Budget);
            }
            // Watching LD B,B rescans RAM every frame, so a run is at most a frame.
            let budget = if watch_ld_b_b {
                self.rescan_ram()?;
                left.min(TICKS_PER_FRAME)
            } else {
                left
            };
            static NONE: AddrFilter = AddrFilter::EMPTY;
            let mut req = RunRequest::new(Budget::ticks(budget));
            req.write = &self.sc;
            req.exec = if watch_ld_b_b { &self.ld_b_b } else { &NONE };
            let stop = self.core.run(&mut req).map_err(|e| e.to_string())?;
            match stop.reason {
                StopReason::Budget(_) if budget == left => return Ok(Event::Budget),
                StopReason::Budget(_) => {}
                StopReason::Locked { opcode } => return Ok(Event::Locked(opcode)),
                StopReason::Stopped => return Ok(Event::Stopped),
                StopReason::Candidate(hit) => match hit.access {
                    Access::Exec if hit.old == Some(LD_B_B) => return Ok(Event::LdBB),
                    Access::Write if hit.addr == SB => self.sb = hit.new,
                    Access::Write if hit.new & SC_START_INTERNAL == SC_START_INTERNAL => {
                        self.serial.push(self.sb);
                        return Ok(Event::Serial);
                    }
                    _ => {}
                },
                StopReason::Frame | StopReason::Step(_) => {}
            }
        }
    }

    /// Runs until LD B,B, a lock, or `until`; serial bytes don't end it.
    pub fn run_to_ld_b_b(&mut self, until: u64) -> Result<Event, String> {
        loop {
            match self.advance(until, true)? {
                Event::Serial => {}
                other => return Ok(other),
            }
        }
    }

    /// Runs to `until` regardless of LD B,B and serial; stops early only on a lock.
    pub fn run_for(&mut self, until: u64) -> Result<Event, String> {
        loop {
            match self.advance(until, false)? {
                Event::Serial => {}
                other => return Ok(other),
            }
        }
    }

    /// Runs until the next complete frame (at most two frames' time).
    pub fn next_frame(&mut self) -> Result<(), String> {
        let mut req = RunRequest::new(Budget::ticks(2 * TICKS_PER_FRAME));
        req.stop_at_frame = true;
        self.core.run(&mut req).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Rebuilds the LD B,B candidates when RAM was last scanned a frame or more ago.
    fn rescan_ram(&mut self) -> Result<(), String> {
        let now = self.core.now().0;
        if self.scanned_at.is_some_and(|at| now - at < TICKS_PER_FRAME) {
            return Ok(());
        }
        let mut filter = self.rom_ld_b_b.clone();
        add_ram_candidates(&mut filter, &self.ram()?);
        self.ld_b_b = filter;
        self.scanned_at = Some(now);
        Ok(())
    }

    fn ram(&self) -> Result<RamView<'_>, String> {
        Ok(RamView {
            wram: self.region_bytes(Region::Wram)?,
            vram: self.region_bytes(Region::Vram)?,
            sram: self.region_bytes(Region::Sram)?,
            hram: self.region_bytes(Region::Hram)?,
        })
    }

    /// A byte of a region's backing store, without side effects.
    pub fn region_bytes(&self, region: Region) -> Result<&[u8], String> {
        self.core
            .region(region)
            .map(|v| v.bytes)
            .map_err(|e| e.to_string())
    }
}

/// Every ROM address an LD B,B could execute from: offsets holding $40 in
/// any bank. Hits whose opcode isn't $40 are resumed; RAM is added per run
/// by [`add_ram_candidates`].
pub fn ld_b_b_candidates(rom: &[u8]) -> AddrFilter {
    let mut filter = AddrFilter::new();
    for (offset, _) in rom.iter().enumerate().filter(|&(_, &b)| b == LD_B_B) {
        if offset < 0x4000 {
            filter.insert(offset as u16, BankSel::Any);
        } else {
            let bank = BankSel::Bank((offset / 0x4000) as u16);
            filter.insert((0x4000 + offset % 0x4000) as u16, bank);
        }
    }
    filter
}

/// The RAM a core exposes, each region's whole backing store.
pub struct RamView<'a> {
    pub wram: &'a [u8],
    pub vram: &'a [u8],
    pub sram: &'a [u8],
    pub hram: &'a [u8],
}

/// Adds every RAM address that holds $40 now, in any bank, echo RAM included.
/// Code copied to RAM is scanned when the next run starts (at most a frame
/// later), so LD B,B in RAM is found unless it is written and executed within
/// one frame and then overwritten.
pub fn add_ram_candidates(filter: &mut AddrFilter, ram: &RamView<'_>) {
    let mut fold = |bytes: &[u8], fixed: usize, base: u16, window: usize| {
        for (i, _) in bytes.iter().enumerate().filter(|&(_, &b)| b == LD_B_B) {
            let offset = if i < fixed {
                i
            } else {
                fixed + (i - fixed) % window
            };
            let addr = base + offset as u16;
            filter.insert(addr, BankSel::Any);
            if (0xC000..0xDE00).contains(&addr) {
                filter.insert(addr + 0x2000, BankSel::Any);
            }
        }
    };
    fold(ram.wram, 0x1000, 0xC000, 0x1000);
    fold(ram.vram, 0, 0x8000, 0x2000);
    fold(ram.sram, 0, 0xA000, 0x2000);
    fold(ram.hram, 0x7F, 0xFF80, 1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ld_b_b_candidates_fold_banked_rom_into_the_switchable_window() {
        let mut rom = vec![0u8; 0x10000];
        rom[0x0150] = LD_B_B;
        rom[0x8123] = LD_B_B; // bank 2, offset $0123
        let filter = ld_b_b_candidates(&rom);
        assert!(filter.matches(0x0150, 0));
        assert!(filter.matches(0x4123, 2));
        assert!(!filter.matches(0x0151, 0));
        assert!(!filter.matches(0x4124, 1));
        assert!(
            !filter.matches(0x4123, 1),
            "a banked candidate matches only its bank"
        );
        assert!(
            !filter.matches(0xC000, 0),
            "RAM is scanned per run, not here"
        );
    }

    #[test]
    fn ram_candidates_are_the_bytes_holding_ld_b_b_in_each_window() {
        let mut wram = vec![0u8; 0x8000]; // CGB: bank 0 plus seven switchable banks
        wram[0x0010] = LD_B_B; // C010
        wram[0x3020] = LD_B_B; // bank 3 at D020
        let mut vram = vec![0u8; 0x4000];
        vram[0x2005] = LD_B_B; // bank 1 at 8005
        let mut sram = vec![0u8; 0x8000];
        sram[0x6001] = LD_B_B; // bank 3 at A001
        let mut hram = vec![0u8; 0x7F];
        hram[0x7E] = LD_B_B; // FFFE
        let mut filter = AddrFilter::new();
        add_ram_candidates(
            &mut filter,
            &RamView {
                wram: &wram,
                vram: &vram,
                sram: &sram,
                hram: &hram,
            },
        );
        for addr in [0xC010, 0xE010, 0xD020, 0xF020, 0x8005, 0xA001, 0xFFFE] {
            assert!(filter.matches(addr, 0), "{addr:#06x}");
        }
        for addr in [0xC011, 0xD010, 0x8006, 0xA000, 0xFF80] {
            assert!(!filter.matches(addr, 0), "{addr:#06x}");
        }
    }
}
