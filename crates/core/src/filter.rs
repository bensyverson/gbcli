//! Bank-aware address filters: the candidate breakpoints and watches a run
//! checks; and store filters, which match writes by the byte they land in.

use crate::Region;

const WORDS: usize = 0x1_0000 / 64;

/// Which bank an entry applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BankSel {
    /// Every bank, and unbanked memory.
    Any,
    /// Only while this bank is mapped at the address.
    Bank(u16),
}

/// A set of `(address, bank)` entries checked on every instruction or access.
///
/// The check is two-stage so a miss costs one bit test: [`may_match`] reads a
/// per-address bitmap of every entry, and only a set bit goes on to the exact,
/// bank-aware [`matches`]. Addresses are canonical: echo RAM is checked at its
/// WRAM address (see [`canonical_addr`]).
///
/// [`may_match`]: AddrFilter::may_match
/// [`matches`]: AddrFilter::matches
#[derive(Clone, Debug)]
pub struct AddrFilter {
    candidates: [u64; WORDS],
    every_bank: [u64; WORDS],
    /// `(addr, bank)` entries for one bank, sorted.
    banked: Vec<(u16, u16)>,
}

impl AddrFilter {
    /// A filter with no entries; also usable in a `static`.
    pub const EMPTY: AddrFilter = AddrFilter::new();

    pub const fn new() -> Self {
        AddrFilter {
            candidates: [0; WORDS],
            every_bank: [0; WORDS],
            banked: Vec::new(),
        }
    }

    pub fn insert(&mut self, addr: u16, bank: BankSel) {
        set(&mut self.candidates, addr);
        match bank {
            BankSel::Any => set(&mut self.every_bank, addr),
            BankSel::Bank(bank) => {
                if let Err(at) = self.banked.binary_search(&(addr, bank)) {
                    self.banked.insert(at, (addr, bank));
                }
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.banked.is_empty() && self.every_bank.iter().all(|&w| w == 0)
    }

    /// Whether any entry names `addr`, in any bank. The hot-path check.
    #[inline(always)]
    pub fn may_match(&self, addr: u16) -> bool {
        get(&self.candidates, addr)
    }

    /// Whether an entry matches `addr` with `bank` mapped there.
    pub fn matches(&self, addr: u16, bank: u16) -> bool {
        self.may_match(addr)
            && (get(&self.every_bank, addr) || self.banked.binary_search(&(addr, bank)).is_ok())
    }
}

impl Default for AddrFilter {
    fn default() -> Self {
        AddrFilter::new()
    }
}

/// The address a filter checks for a CPU access: echo RAM (E000–FDFF) is
/// reported at the WRAM address it mirrors (C000–DDFF).
#[inline(always)]
pub fn canonical_addr(addr: u16) -> u16 {
    match addr {
        0xE000..=0xFDFF => addr - 0x2000,
        _ => addr,
    }
}

/// Backing-store bytes whose CPU writes are candidates. A write is matched by
/// the byte it lands in (the same `(region, offset)` direct access reads),
/// whatever address, bank or mirror it was made through, so a reader of the
/// backing store and the write hook share one identity.
///
/// Only bytes a CPU write can change are meaningful: WRAM, HRAM and IE, and
/// cart RAM where the core has [`Feature::CartRamWrites`](crate::Feature).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StoreFilter {
    entries: Vec<(Region, usize)>,
}

impl StoreFilter {
    pub const fn new() -> Self {
        StoreFilter {
            entries: Vec::new(),
        }
    }

    pub fn insert(&mut self, region: Region, offset: usize) {
        if !self.contains(region, offset) {
            self.entries.push((region, offset));
        }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Whether a write landing at `offset` in `region` is a candidate. A
    /// predicate arms a handful of bytes, so a scan beats any index.
    #[inline]
    pub fn contains(&self, region: Region, offset: usize) -> bool {
        self.entries.contains(&(region, offset))
    }

    /// The armed bytes, in insertion order.
    pub fn entries(&self) -> &[(Region, usize)] {
        &self.entries
    }
}

#[inline(always)]
fn set(bits: &mut [u64; WORDS], addr: u16) {
    bits[usize::from(addr) / 64] |= 1 << (addr % 64);
}

#[inline(always)]
fn get(bits: &[u64; WORDS], addr: u16) -> bool {
    bits[usize::from(addr) / 64] & (1 << (addr % 64)) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_store_filter_matches_its_bytes_by_region_and_offset() {
        let mut stores = StoreFilter::new();
        assert!(stores.is_empty());
        stores.insert(Region::Wram, 0x1000);
        stores.insert(Region::Wram, 0x1000);
        stores.insert(Region::Sram, 0);
        assert_eq!(stores.entries().len(), 2, "duplicates are kept once");
        assert!(stores.contains(Region::Wram, 0x1000));
        assert!(stores.contains(Region::Sram, 0));
        assert!(!stores.contains(Region::Sram, 0x1000));
        assert!(!stores.contains(Region::Hram, 0));
    }

    #[test]
    fn an_empty_filter_matches_nothing() {
        let filter = AddrFilter::new();
        assert!(filter.is_empty());
        assert!(!filter.may_match(0x4000));
        assert!(!filter.matches(0x4000, 1));
    }

    #[test]
    fn an_any_bank_entry_matches_every_bank() {
        let mut filter = AddrFilter::new();
        filter.insert(0xC123, BankSel::Any);
        assert!(!filter.is_empty());
        assert!(filter.matches(0xC123, 0));
        assert!(filter.matches(0xC123, 7));
        assert!(!filter.matches(0xC124, 0));
    }

    #[test]
    fn a_banked_entry_matches_only_its_bank() {
        let mut filter = AddrFilter::new();
        filter.insert(0x4100, BankSel::Bank(2));
        assert!(!filter.is_empty());
        assert!(filter.may_match(0x4100));
        assert!(filter.matches(0x4100, 2));
        assert!(!filter.matches(0x4100, 1));
        assert!(!filter.matches(0x4101, 2));
    }

    #[test]
    fn entries_in_several_banks_at_one_address_each_match() {
        let mut filter = AddrFilter::new();
        filter.insert(0xA000, BankSel::Bank(1));
        filter.insert(0xA000, BankSel::Bank(3));
        filter.insert(0xA000, BankSel::Bank(3));
        assert!(filter.matches(0xA000, 1));
        assert!(filter.matches(0xA000, 3));
        assert!(!filter.matches(0xA000, 2));
    }

    #[test]
    fn the_last_address_is_filterable() {
        let mut filter = AddrFilter::new();
        filter.insert(0xFFFF, BankSel::Any);
        assert!(filter.matches(0xFFFF, 0));
    }

    #[test]
    fn echo_ram_is_checked_at_the_wram_address_it_mirrors() {
        assert_eq!(canonical_addr(0xE000), 0xC000);
        assert_eq!(canonical_addr(0xFDFF), 0xDDFF);
        assert_eq!(canonical_addr(0xDFFF), 0xDFFF);
        assert_eq!(canonical_addr(0xFE00), 0xFE00);
    }
}
