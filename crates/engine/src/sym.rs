//! RGBDS `.sym` files: `BB:AAAA Name` per line, `;` comments (rgblink's
//! `-n` output). Symbols resolve to a bank and an address (design §7).

use std::collections::HashMap;
use std::fmt;

use gbcli_core::Core;

use crate::disasm::{SymbolLookup, SymbolRef};

/// One symbol: a label or constant address in a bank.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub bank: u16,
    pub addr: u16,
}

/// The memory area an address falls in, which decides what its bank means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Area {
    Rom0,
    RomX,
    Vram,
    Sram,
    Wram0,
    WramX,
    Echo,
    Oam,
    Unusable,
    Io,
    Hram,
    Ie,
}

impl Area {
    pub fn of(addr: u16) -> Area {
        match addr {
            0x0000..=0x3FFF => Area::Rom0,
            0x4000..=0x7FFF => Area::RomX,
            0x8000..=0x9FFF => Area::Vram,
            0xA000..=0xBFFF => Area::Sram,
            0xC000..=0xCFFF => Area::Wram0,
            0xD000..=0xDFFF => Area::WramX,
            0xE000..=0xFDFF => Area::Echo,
            0xFE00..=0xFE9F => Area::Oam,
            0xFEA0..=0xFEFF => Area::Unusable,
            0xFF00..=0xFF7F => Area::Io,
            0xFF80..=0xFFFE => Area::Hram,
            0xFFFF => Area::Ie,
        }
    }

    pub fn is_rom(self) -> bool {
        matches!(self, Area::Rom0 | Area::RomX)
    }

    /// WRAM, HRAM or cart RAM: what the rebuilt-ROM check treats as RAM symbols.
    pub fn is_ram(self) -> bool {
        matches!(self, Area::Wram0 | Area::WramX | Area::Hram | Area::Sram)
    }
}

/// A line the parser couldn't read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymError {
    /// 1-based.
    pub line: usize,
    pub text: String,
}

impl fmt::Display for SymError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "line {}: expected `BB:AAAA Name`, got {:?}",
            self.line, self.text
        )
    }
}

impl std::error::Error for SymError {}

/// A parsed `.sym` file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Symbols {
    /// Sorted by (area, bank, address, file order).
    symbols: Vec<Symbol>,
    by_name: HashMap<String, Vec<usize>>,
}

impl Symbols {
    pub fn parse(text: &str) -> Result<Symbols, SymError> {
        let mut symbols = Vec::new();
        for (i, raw) in text.lines().enumerate() {
            let line = raw.split(';').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let bad = || SymError {
                line: i + 1,
                text: raw.to_string(),
            };
            let (loc, name) = line.split_once(char::is_whitespace).ok_or_else(bad)?;
            let (bank, addr) = loc.split_once(':').ok_or_else(bad)?;
            let bank = u16::from_str_radix(bank, 16).map_err(|_| bad())?;
            let addr = u16::from_str_radix(addr, 16).map_err(|_| bad())?;
            let name = name.trim();
            if name.is_empty() || name.contains(char::is_whitespace) {
                return Err(bad());
            }
            symbols.push(Symbol {
                name: name.to_string(),
                bank,
                addr,
            });
        }
        symbols.sort_by_key(|s| (Area::of(s.addr) as u8, s.bank, s.addr));
        let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, s) in symbols.iter().enumerate() {
            by_name.entry(s.name.clone()).or_default().push(i);
        }
        Ok(Symbols { symbols, by_name })
    }

    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Symbol> {
        self.symbols.iter()
    }

    /// Every symbol with this exact name (more than one is ambiguous).
    pub fn named(&self, name: &str) -> Vec<&Symbol> {
        self.by_name
            .get(name)
            .map(|ids| ids.iter().map(|&i| &self.symbols[i]).collect())
            .unwrap_or_default()
    }

    /// The symbol with this name, if exactly one has it.
    pub fn get(&self, name: &str) -> Option<&Symbol> {
        match self.named(name).as_slice() {
            [one] => Some(one),
            _ => None,
        }
    }

    /// The nearest symbol at or below `addr` in the same area and bank: the
    /// label whose code or data `addr` is in. Local labels count.
    pub fn containing(&self, bank: u16, addr: u16) -> Option<&Symbol> {
        let area = Area::of(addr);
        self.symbols
            .iter()
            .filter(|s| Area::of(s.addr) == area && s.bank == bank && s.addr <= addr)
            .max_by_key(|s| s.addr)
    }

    /// Names close to `name` (case-insensitive edit distance at most 2), nearest first.
    pub fn near_misses(&self, name: &str) -> Vec<&str> {
        let lower = name.to_ascii_lowercase();
        let mut near: Vec<(usize, &str)> = self
            .by_name
            .keys()
            .map(|k| (edit_distance(&lower, &k.to_ascii_lowercase()), k.as_str()))
            .filter(|&(d, _)| d <= 2)
            .collect();
        near.sort();
        near.into_iter().map(|(_, k)| k).take(5).collect()
    }
}

fn symbol_ref(s: &Symbol, addr: u16) -> SymbolRef {
    SymbolRef {
        name: s.name.clone(),
        offset: addr - s.addr,
    }
}

/// Without a core, only addresses outside the switchable areas (ROMX, VRAM,
/// cart RAM, WRAMX) are named; there the bank is always 0. Use
/// [`Symbols::mapped`] to name banked addresses in the banks mapped now.
impl SymbolLookup for Symbols {
    fn symbol(&self, addr: u16) -> Option<SymbolRef> {
        let banked = matches!(
            Area::of(addr),
            Area::RomX | Area::Vram | Area::Sram | Area::WramX
        );
        if banked {
            return None;
        }
        self.containing(0, addr).map(|s| symbol_ref(s, addr))
    }
}

/// Symbols resolved in the banks a core has mapped now.
pub struct MappedSymbols<'a> {
    symbols: &'a Symbols,
    core: &'a dyn Core,
}

impl Symbols {
    pub fn mapped<'a>(&'a self, core: &'a dyn Core) -> MappedSymbols<'a> {
        MappedSymbols {
            symbols: self,
            core,
        }
    }
}

impl SymbolLookup for MappedSymbols<'_> {
    fn symbol(&self, addr: u16) -> Option<SymbolRef> {
        let bank = crate::mem::bank_at(self.core, addr);
        self.symbols
            .containing(bank, addr)
            .map(|s| symbol_ref(s, addr))
    }
}

fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cur = row[j + 1];
            row[j + 1] = (prev + usize::from(ca != cb)).min(row[j] + 1).min(cur + 1);
            prev = cur;
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    const SYM: &str = "; File generated by rgblink\n\
        00:0150 Main\n\
        00:0158 Main.loop\n\
        00:0200 VBlank ; trailing comment\n\
        01:4000 Routine1\n\
        02:4000 Routine2\n\
        00:c000 wFrame\n\
        00:c002 wSpeed\n\
        00:ff80 hCounter\n\
        \n";

    #[test]
    fn lines_parse_to_bank_and_address() {
        let syms = Symbols::parse(SYM).unwrap();
        assert_eq!(syms.len(), 8);
        let main = syms.get("Main.loop").unwrap();
        assert_eq!((main.bank, main.addr), (0, 0x0158));
        assert_eq!(syms.get("Routine2").unwrap().bank, 2);
        assert_eq!(syms.get("hCounter").unwrap().addr, 0xFF80);
    }

    #[test]
    fn a_malformed_line_names_its_number() {
        let err = Symbols::parse("00:0150 Main\nnot a symbol\n").unwrap_err();
        assert_eq!(err.line, 2);
        assert!(Symbols::parse("0g:0150 Main\n").is_err());
        assert!(Symbols::parse("00:0150\n").is_err());
    }

    #[test]
    fn the_containing_symbol_is_the_nearest_below_in_the_same_bank() {
        let syms = Symbols::parse(SYM).unwrap();
        assert_eq!(syms.containing(0, 0x0157).unwrap().name, "Main");
        assert_eq!(syms.containing(0, 0x0160).unwrap().name, "Main.loop");
        assert_eq!(syms.containing(2, 0x4010).unwrap().name, "Routine2");
        assert_eq!(syms.containing(0, 0x0100), None);
        assert_eq!(
            syms.containing(0, 0x3FFF).unwrap().name,
            "VBlank",
            "bank 0 ROM"
        );
        assert_eq!(syms.containing(0, 0x4000), None, "ROMX bank 0 has none");
        assert_eq!(syms.containing(0, 0xC001).unwrap().name, "wFrame");
    }

    #[test]
    fn a_duplicate_name_is_ambiguous() {
        let syms = Symbols::parse("01:4000 Init\n02:4000 Init\n").unwrap();
        assert_eq!(syms.named("Init").len(), 2);
        assert_eq!(syms.get("Init"), None);
    }

    #[test]
    fn as_a_disassembler_lookup_it_names_only_unbanked_addresses() {
        use crate::disasm::{SymbolLookup, SymbolRef};
        let syms = Symbols::parse(SYM).unwrap();
        let named = |name: &str, offset| {
            Some(SymbolRef {
                name: name.into(),
                offset,
            })
        };
        assert_eq!(syms.symbol(0x0159), named("Main.loop", 1));
        assert_eq!(syms.symbol(0xC000), named("wFrame", 0));
        assert_eq!(syms.symbol(0x4000), None, "ROMX: the bank isn't known");
    }

    #[test]
    fn near_misses_suggest_close_names() {
        let syms = Symbols::parse(SYM).unwrap();
        assert_eq!(syms.near_misses("wframe"), vec!["wFrame"]);
        assert_eq!(syms.near_misses("wSped"), vec!["wSpeed"]);
        assert!(syms.near_misses("Elsewhere").is_empty());
    }

    #[test]
    fn areas_classify_ram_and_rom() {
        assert!(Area::of(0xC123).is_ram() && Area::of(0xFF90).is_ram());
        assert!(Area::of(0xA000).is_ram() && !Area::of(0x8000).is_ram());
        assert!(Area::of(0x4000).is_rom() && !Area::of(0xFF44).is_ram());
    }
}
