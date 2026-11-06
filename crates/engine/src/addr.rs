//! The §7 address grammar outside expressions: what a breakpoint, a watch,
//! `--to`, `--at` and a write target accept.
//!
//! Symbols resolve when the command runs, so a stale `.sym` fails there, not
//! later; near misses and ambiguous candidates are named.

use std::fmt;

use gbcli_core::{Region, canonical_addr};
use gbcli_protocol::Location;

use crate::sym::{Area, Symbol, Symbols};

/// Why a target didn't parse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AddrError {
    /// `bad_address`, with a corrected form when one is obvious.
    Syntax { text: String, hint: Option<String> },
    /// `unknown_symbol`, with the closest names.
    UnknownSymbol { name: String, near: Vec<String> },
    /// `ambiguous_symbol`, with every candidate.
    AmbiguousSymbol {
        name: String,
        candidates: Vec<Symbol>,
    },
    /// A region form with no CPU address (`bgpal:2`, `boot:0`).
    NoCpuAddress { region: Region },
}

impl AddrError {
    pub fn code(&self) -> gbcli_protocol::ErrorCode {
        use gbcli_protocol::ErrorCode as E;
        match self {
            AddrError::Syntax { .. } => E::BadAddress,
            AddrError::UnknownSymbol { .. } => E::UnknownSymbol,
            AddrError::AmbiguousSymbol { .. } => E::AmbiguousSymbol,
            AddrError::NoCpuAddress { .. } => E::BadAddress,
        }
    }

    pub fn remedy(&self) -> String {
        match self {
            AddrError::Syntax { hint: Some(h), .. } => format!("did you mean {h}?"),
            AddrError::UnknownSymbol { near, .. } if !near.is_empty() => {
                format!("did you mean {}?", near.join(", "))
            }
            AddrError::AmbiguousSymbol { candidates, .. } => {
                let list: Vec<String> = candidates
                    .iter()
                    .map(|s| format!("{:02X}:{:04X}", s.bank, s.addr))
                    .collect();
                format!("qualify it with a bank: one of {}", list.join(", "))
            }
            other => other.code().default_remedy().to_string(),
        }
    }
}

impl fmt::Display for AddrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AddrError::Syntax { text, .. } => write!(f, "not an address: {text:?}"),
            AddrError::UnknownSymbol { name, .. } => write!(f, "unknown symbol {name:?}"),
            AddrError::AmbiguousSymbol { name, candidates } => {
                write!(f, "{name:?} names {} symbols", candidates.len())
            }
            AddrError::NoCpuAddress { region } => {
                write!(
                    f,
                    "{} has no CPU address",
                    crate::inspect::regions::region_name(*region)
                )
            }
        }
    }
}

impl std::error::Error for AddrError {}

/// A write target: a CPU address, or a ROM file offset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteTarget {
    Cpu(Location),
    /// A byte offset into the ROM file (`rom:0x14E`).
    RomOffset(u32),
}

/// Parses a target into a CPU location, resolving symbols.
pub fn parse(text: &str, symbols: Option<&Symbols>) -> Result<Location, AddrError> {
    let text = text.trim();
    if let Some(after) = text.strip_prefix("rom:") {
        let offset = hex(after).ok_or_else(|| syntax(text))?;
        let offset = u32::try_from(offset).map_err(|_| syntax(text))?;
        return Ok(rom_location(offset));
    }
    if let Some((region, after)) = text.split_once(':')
        && !region.is_empty()
        && !region.chars().all(|c| c.is_ascii_hexdigit() || c == '$')
    {
        let region = crate::inspect::regions::parse_region(region).ok_or_else(|| syntax(text))?;
        let offset = hex(after).ok_or_else(|| syntax(text))?;
        return region_location(region, offset);
    }
    if let Some((bank, addr)) = text.split_once(':') {
        let bank = hex(bank).ok_or_else(|| syntax(text))?;
        let addr = hex(addr).ok_or_else(|| syntax(text))?;
        let bank = u16::try_from(bank).map_err(|_| syntax(text))?;
        let addr = u16::try_from(addr).map_err(|_| syntax(text))?;
        return Ok(Location {
            addr: canonical_addr(addr),
            bank: Some(bank),
            symbol: None,
        });
    }
    if let Some(value) = hex_prefixed(text) {
        let addr = u16::try_from(value).map_err(|_| syntax(text))?;
        return Ok(Location {
            addr: canonical_addr(addr),
            bank: None,
            symbol: None,
        });
    }
    let (name, offset) = split_offset(text);
    let symbols = symbols.ok_or_else(|| AddrError::UnknownSymbol {
        name: name.to_string(),
        near: Vec::new(),
    })?;
    match symbols.named(name).as_slice() {
        [] => Err(AddrError::UnknownSymbol {
            name: name.to_string(),
            near: symbols
                .near_misses(name)
                .into_iter()
                .map(str::to_string)
                .collect(),
        }),
        [one] => Ok(Location {
            addr: canonical_addr(one.addr.wrapping_add(offset)),
            // A symbol in a fixed area has no bank to qualify; elsewhere its
            // own bank is the one meant (design §7).
            bank: bank_of(one),
            symbol: Some(if offset == 0 {
                name.to_string()
            } else {
                format!("{name}+{offset}")
            }),
        }),
        many => Err(AddrError::AmbiguousSymbol {
            name: name.to_string(),
            candidates: many.iter().map(|&s| s.clone()).collect(),
        }),
    }
}

/// Parses a write target: a CPU location or an explicit ROM offset.
pub fn parse_write(text: &str, symbols: Option<&Symbols>) -> Result<WriteTarget, AddrError> {
    let text = text.trim();
    if let Some(after) = text.strip_prefix("rom:") {
        let offset = hex(after).ok_or_else(|| syntax(text))?;
        return u32::try_from(offset)
            .map(WriteTarget::RomOffset)
            .map_err(|_| syntax(text));
    }
    parse(text, symbols).map(WriteTarget::Cpu)
}

/// A bank for a symbol, when its area can be switched.
fn bank_of(symbol: &Symbol) -> Option<u16> {
    matches!(
        Area::of(symbol.addr),
        Area::RomX | Area::Vram | Area::Sram | Area::WramX
    )
    .then_some(symbol.bank)
}

fn rom_location(offset: u32) -> Location {
    if offset < 0x4000 {
        Location {
            addr: offset as u16,
            bank: Some(0),
            symbol: None,
        }
    } else {
        Location {
            addr: 0x4000 + (offset as u16 & 0x3FFF),
            bank: Some((offset / 0x4000) as u16),
            symbol: None,
        }
    }
}

/// A region offset, as a CPU location, by the whole-store layout (design §7).
pub fn region_location(region: Region, offset: u64) -> Result<Location, AddrError> {
    let no_cpu = |region| Err(AddrError::NoCpuAddress { region });
    let at = |addr: u16, bank: Option<u16>| {
        Ok(Location {
            addr,
            bank,
            symbol: None,
        })
    };
    match region {
        Region::Rom => Ok(rom_location(u32::try_from(offset).map_err(|_| {
            AddrError::Syntax {
                text: format!("{offset}"),
                hint: None,
            }
        })?)),
        Region::Wram => match offset {
            0x0000..=0x0FFF => at(0xC000 + offset as u16, Some(0)),
            0x1000..=0x1FFF => at(0xD000 + (offset as u16 - 0x1000), Some(1)),
            _ => at(
                0xD000 + (offset as u16 & 0x0FFF),
                Some((offset / 0x1000) as u16),
            ),
        },
        Region::Sram => at(
            0xA000 + (offset as u16 & 0x1FFF),
            Some((offset / 0x2000) as u16),
        ),
        Region::Vram => at(
            0x8000 + (offset as u16 & 0x1FFF),
            Some((offset / 0x2000) as u16),
        ),
        Region::Hram => at(0xFF80 + offset as u16, None),
        Region::Oam => at(0xFE00 + offset as u16, None),
        Region::Io => at(0xFF00 + offset as u16, None),
        Region::Ie => at(0xFFFF, None),
        Region::BootRom | Region::BgPalette | Region::ObjPalette => no_cpu(region),
    }
}

fn syntax(text: &str) -> AddrError {
    AddrError::Syntax {
        text: text.to_string(),
        hint: Some("$C0A0, 0xC0A0, 03:4123 or a .sym name".into()),
    }
}

/// Hex with `$` or `0x`; used where a bare number could be a symbol.
fn hex_prefixed(text: &str) -> Option<u64> {
    let text = text.trim();
    if text.starts_with('$') || text.starts_with("0x") || text.starts_with("0X") {
        hex(text)
    } else {
        None
    }
}

/// Hex with `$` or `0x`, or bare: the form region offsets and bank numbers use.
fn hex(text: &str) -> Option<u64> {
    let text = text.trim();
    let digits = text
        .strip_prefix('$')
        .or_else(|| text.strip_prefix("0x"))
        .or_else(|| text.strip_prefix("0X"))
        .unwrap_or(text);
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    u64::from_str_radix(digits, 16).ok()
}

/// Splits `Name+3` and `Name+$3`; the base may itself contain `+`.
fn split_offset(text: &str) -> (&str, u16) {
    let Some((base, tail)) = text.rsplit_once('+') else {
        return (text, 0);
    };
    match hex(tail) {
        Some(n) if !base.is_empty() => (base, u16::try_from(n).unwrap_or(0)),
        _ => (text, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sym::Symbols;

    const SYM: &str = "\
00:0150 Main\n\
00:0153 Main.loop\n\
03:4000 Banked\n\
00:C000 wFrame\n";

    fn symbols() -> Symbols {
        Symbols::parse(SYM).unwrap()
    }

    #[test]
    fn a_standalone_address_needs_a_prefix_but_a_symbol_does_not() {
        let s = symbols();
        let at = parse("$C0A0", Some(&s)).unwrap();
        assert_eq!((at.addr, at.bank), (0xC0A0, None));
        let at = parse("0xC0A0", Some(&s)).unwrap();
        assert_eq!(at.addr, 0xC0A0);
        assert!(parse("C0A0", Some(&s)).is_err(), "no prefix on bare hex");
    }

    #[test]
    fn a_symbol_resolves_to_its_bank_and_address() {
        let s = symbols();
        let at = parse("Main", Some(&s)).unwrap();
        assert_eq!((at.addr, at.bank), (0x0150, None));
        let at = parse("Banked", Some(&s)).unwrap();
        assert_eq!((at.addr, at.bank), (0x4000, Some(3)));
        assert_eq!(parse("Main+3", Some(&s)).unwrap().addr, 0x0153);
        assert_eq!(parse("Banked+$10", Some(&s)).unwrap().addr, 0x4010);
        assert_eq!(parse("Main.loop+1", Some(&s)).unwrap().addr, 0x0154);
        assert!(parse("Main+$", Some(&s)).is_err());
    }

    #[test]
    fn an_unknown_symbol_lists_near_misses() {
        let err = parse("Mian", Some(&symbols())).unwrap_err();
        let AddrError::UnknownSymbol { near, .. } = &err else {
            panic!("{err:?}")
        };
        assert_eq!(near, &["Main".to_string()]);
        assert!(err.remedy().contains("Main"));
    }

    #[test]
    fn a_bank_qualified_address_carries_its_bank() {
        let at = parse("03:4123", None).unwrap();
        assert_eq!((at.addr, at.bank), (0x4123, Some(3)));
        let at = parse("$03:$4123", None).unwrap();
        assert_eq!((at.addr, at.bank), (0x4123, Some(3)));
    }

    #[test]
    fn region_forms_map_onto_the_whole_backing_store() {
        assert_eq!(parse("wram:0x1A0", None).unwrap().addr, 0xC1A0);
        assert_eq!(parse("wram:0x1000", None).unwrap().addr, 0xD000);
        assert_eq!(parse("wram:0x1000", None).unwrap().bank, Some(1));
        assert_eq!(parse("wram:0x2000", None).unwrap().bank, Some(2));
        assert_eq!(parse("sram:0x2000", None).unwrap().addr, 0xA000);
        assert_eq!(parse("sram:0x2000", None).unwrap().bank, Some(1));
        assert_eq!(parse("vram:0x2000", None).unwrap().addr, 0x8000);
        assert_eq!(parse("vram:0x2000", None).unwrap().bank, Some(1));
        assert_eq!(parse("oam:4", None).unwrap().addr, 0xFE04);
        assert_eq!(parse("io:0x44", None).unwrap().addr, 0xFF44);
        assert_eq!(parse("rom:0x14E", None).unwrap().addr, 0x014E);
        assert_eq!(parse("rom:0x4010", None).unwrap().addr, 0x4010);
        assert_eq!(parse("rom:0x4010", None).unwrap().bank, Some(1));
    }

    #[test]
    fn a_region_without_a_cpu_address_is_refused() {
        assert!(matches!(
            parse("bgpal:2", None),
            Err(AddrError::NoCpuAddress {
                region: Region::BgPalette
            })
        ));
    }

    #[test]
    fn echo_ram_canonicalizes() {
        assert_eq!(parse("$E000", None).unwrap().addr, 0xC000);
    }

    #[test]
    fn a_rom_offset_write_stays_a_file_offset() {
        assert_eq!(
            parse_write("rom:0x14E", None).unwrap(),
            WriteTarget::RomOffset(0x014E)
        );
        assert!(matches!(
            parse_write("wFrame", Some(&symbols())).unwrap(),
            WriteTarget::Cpu(_)
        ));
    }
}
