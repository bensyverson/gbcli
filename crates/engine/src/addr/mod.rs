//! The §7 address grammar outside expressions: what a breakpoint, a watch,
//! `--to`, `--at` and a write target accept.
//!
//! Symbols resolve when the command runs, so a stale `.sym` fails there, not
//! later; near misses and ambiguous candidates are named. Region offsets are
//! checked against the region's size, so an address that doesn't exist on the
//! machine is refused rather than wrapped onto one that does.

use std::fmt;

use gbcli_core::{Region, canonical_addr};
use gbcli_protocol::Location;

use crate::inspect::regions::{ALL_REGIONS, bank_size, parse_region, region_name};
use crate::sym::{Area, Symbol, Symbols};

mod offset;
mod sizes;

pub use offset::{label, split_label};
pub use sizes::{RegionSizes, SizeBasis};

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
    /// A region form with no CPU address (`bgpal:2`, `bootrom:0`).
    NoCpuAddress { region: Region },
    /// A region offset at or past the region's end.
    RegionOffset {
        region: Region,
        offset: u64,
        /// The region's size in bytes, every bank included.
        size: usize,
        basis: SizeBasis,
    },
    /// A bank-qualified address in a bank the region doesn't have.
    BankRange {
        /// The CPU address, canonicalized.
        addr: u16,
        bank: u16,
        /// The banked region the address falls in.
        region: Region,
        /// How many banks the region has (0 for a cart without RAM).
        banks: u64,
        basis: SizeBasis,
    },
    /// A symbol offset that leaves the address space, or the switchable
    /// area the symbol's bank belongs to.
    SymbolOffset {
        /// The target as written.
        text: String,
        name: String,
        /// The symbol's own address.
        addr: u16,
        /// The symbol's bank, when its area is switchable.
        bank: Option<u16>,
    },
}

impl AddrError {
    pub fn code(&self) -> gbcli_protocol::ErrorCode {
        use gbcli_protocol::ErrorCode as E;
        match self {
            AddrError::Syntax { .. } => E::BadAddress,
            AddrError::UnknownSymbol { .. } => E::UnknownSymbol,
            AddrError::AmbiguousSymbol { .. } => E::AmbiguousSymbol,
            AddrError::NoCpuAddress { .. }
            | AddrError::RegionOffset { .. }
            | AddrError::BankRange { .. }
            | AddrError::SymbolOffset { .. } => E::BadAddress,
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
            AddrError::BankRange { addr, banks, .. } if *banks > 0 => {
                let last = banks - 1;
                format!("use a bank from 00 to {last:02X}, such as {last:02X}:{addr:04X}")
            }
            AddrError::RegionOffset { size: 0, .. } | AddrError::BankRange { .. } => {
                "this cart has no cart RAM; wram: and hram: offsets reach the RAM it has".into()
            }
            AddrError::RegionOffset { region, size, .. } => {
                let name = region_name(*region);
                format!(
                    "use an offset from {name}:0 to {name}:{}",
                    number(*size as u64 - 1)
                )
            }
            AddrError::SymbolOffset {
                text,
                name,
                addr,
                bank,
            } => offset::describe(text, name, *addr, *bank).1,
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
                write!(f, "{} has no CPU address", region_name(*region))
            }
            AddrError::RegionOffset {
                region,
                offset,
                size,
                basis,
            } => {
                let name = region_name(*region);
                let at = format!("{name}:{}", number(*offset));
                if *size == 0 {
                    return write!(f, "{at} doesn't exist: this cart has no cart RAM");
                }
                let bytes = match size {
                    1 => "1 byte".to_string(),
                    n => format!("{} bytes", number(*n as u64)),
                };
                let banks = match bank_size(*region) {
                    Some(bank) if *size > bank => format!(" (banks 0-{})", size / bank - 1),
                    _ => String::new(),
                };
                write!(
                    f,
                    "{at} is past the end of {name}, which holds {bytes}{banks} {}",
                    whose(*basis)
                )
            }
            AddrError::SymbolOffset {
                text,
                name,
                addr,
                bank,
            } => f.write_str(&offset::describe(text, name, *addr, *bank).0),
            AddrError::BankRange {
                addr,
                bank,
                region,
                banks,
                basis,
            } => {
                let at = format!("{bank:02X}:{addr:04X}");
                if *banks == 0 {
                    return write!(f, "{at} doesn't exist: this cart has no cart RAM");
                }
                let kind = bank_kind(*region);
                let plural = if *banks == 1 { "" } else { "s" };
                write!(
                    f,
                    "{at} is in {kind} bank {bank:02X}, past the {banks} {kind} bank{plural} \
                     (00-{:02X}) {}",
                    banks - 1,
                    whose(*basis)
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

/// Parses a target into a CPU location, resolving symbols, with region
/// offsets bounded by the largest any model has. Prefer [`parse_on`] when a
/// machine is at hand: it refuses offsets this model and cart lack.
pub fn parse(text: &str, symbols: Option<&Symbols>) -> Result<Location, AddrError> {
    parse_on(text, symbols, &RegionSizes::ANY_MODEL)
}

/// Parses a target on a machine whose region sizes are `sizes`.
pub fn parse_on(
    text: &str,
    symbols: Option<&Symbols>,
    sizes: &RegionSizes,
) -> Result<Location, AddrError> {
    let text = text.trim();
    if let Some((head, tail)) = text.split_once(':') {
        // A hex head is a bank, prefixed or not; anything else names a region.
        let Some(bank) = hex(head) else {
            let region = parse_region(head.trim()).ok_or_else(|| unknown_region(text, tail))?;
            let offset = hex(tail).ok_or_else(|| syntax(text))?;
            return region_location(region, offset, sizes);
        };
        let addr = hex(tail).ok_or_else(|| syntax(text))?;
        let bank = u16::try_from(bank).map_err(|_| syntax(text))?;
        let addr = canonical_addr(u16::try_from(addr).map_err(|_| syntax(text))?);
        bank_within(addr, bank, sizes)?;
        return Ok(Location {
            addr,
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
    let (name, delta) = offset::split_offset(text)?;
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
        // A symbol in a fixed area has no bank to qualify; elsewhere its own
        // bank is the one meant (design §7).
        [one] => offset::resolve(text, name, delta, one),
        many => Err(AddrError::AmbiguousSymbol {
            name: name.to_string(),
            candidates: many.iter().map(|&s| s.clone()).collect(),
        }),
    }
}

/// Parses a write target (a CPU location or an explicit ROM offset), with
/// sizes bounded by the largest any model has; see [`parse_write_on`].
pub fn parse_write(text: &str, symbols: Option<&Symbols>) -> Result<WriteTarget, AddrError> {
    parse_write_on(text, symbols, &RegionSizes::ANY_MODEL)
}

/// Parses a write target on a machine whose region sizes are `sizes`.
pub fn parse_write_on(
    text: &str,
    symbols: Option<&Symbols>,
    sizes: &RegionSizes,
) -> Result<WriteTarget, AddrError> {
    let text = text.trim();
    if let Some(after) = text.strip_prefix("rom:") {
        let offset = hex(after).ok_or_else(|| syntax(text))?;
        within(Region::Rom, offset, sizes)?;
        return Ok(WriteTarget::RomOffset(offset as u32));
    }
    parse_on(text, symbols, sizes).map(WriteTarget::Cpu)
}

/// A region offset, as a CPU location, by the whole-store layout (design §7);
/// refused at or past the region's end in `sizes`.
pub fn region_location(
    region: Region,
    offset: u64,
    sizes: &RegionSizes,
) -> Result<Location, AddrError> {
    within(region, offset, sizes)?;
    // `within` bounds the offset by a region size, at most 8 MiB.
    let offset = offset as u32;
    let (addr, bank) = match region {
        Region::Rom if offset < 0x4000 => (offset as u16, Some(0)),
        Region::Rom => (0x4000 + (offset & 0x3FFF) as u16, Some(offset / 0x4000)),
        Region::Wram if offset < 0x1000 => (0xC000 + offset as u16, Some(0)),
        Region::Wram => (0xD000 + (offset & 0x0FFF) as u16, Some(offset / 0x1000)),
        Region::Sram => (0xA000 + (offset & 0x1FFF) as u16, Some(offset / 0x2000)),
        Region::Vram => (0x8000 + (offset & 0x1FFF) as u16, Some(offset / 0x2000)),
        Region::Hram => (0xFF80 + offset as u16, None),
        Region::Oam => (0xFE00 + offset as u16, None),
        Region::Io => (0xFF00 + offset as u16, None),
        Region::Ie => (0xFFFF, None),
        Region::BootRom | Region::BgPalette | Region::ObjPalette => {
            unreachable!("within refuses a region with no CPU address")
        }
    };
    Ok(Location {
        addr,
        bank: bank.map(|b| b as u16),
        symbol: None,
    })
}

/// Refuses `offset` unless it is inside `region` on `sizes`.
fn within(region: Region, offset: u64, sizes: &RegionSizes) -> Result<(), AddrError> {
    let size = sizes
        .len(region)
        .ok_or(AddrError::NoCpuAddress { region })?;
    if offset >= size as u64 {
        return Err(AddrError::RegionOffset {
            region,
            offset,
            size,
            basis: sizes.basis,
        });
    }
    Ok(())
}

/// Refuses `bank` at `addr` unless the switchable region there has that
/// bank on `sizes`; an address in a fixed area passes unchecked.
fn bank_within(addr: u16, bank: u16, sizes: &RegionSizes) -> Result<(), AddrError> {
    let region = match Area::of(addr) {
        Area::RomX => Region::Rom,
        Area::WramX => Region::Wram,
        Area::Sram => Region::Sram,
        Area::Vram => Region::Vram,
        _ => return Ok(()),
    };
    let (Some(size), Some(per_bank)) = (sizes.len(region), bank_size(region)) else {
        return Ok(());
    };
    let banks = (size as u64).div_ceil(per_bank as u64);
    if u64::from(bank) >= banks {
        return Err(AddrError::BankRange {
            addr,
            bank,
            region,
            banks,
            basis: sizes.basis,
        });
    }
    Ok(())
}

/// What an error calls a banked region's banks.
fn bank_kind(region: Region) -> &'static str {
    match region {
        Region::Rom => "ROM",
        Region::Wram => "WRAM",
        Region::Sram => "cart RAM",
        Region::Vram => "VRAM",
        _ => region_name(region),
    }
}

fn whose(basis: SizeBasis) -> &'static str {
    match basis {
        SizeBasis::ThisMachine => "on this machine",
        SizeBasis::AnyModel => "on any model",
    }
}

fn syntax(text: &str) -> AddrError {
    AddrError::Syntax {
        text: text.to_string(),
        hint: Some("$C0A0, 0xC0A0, 03:4123 or a .sym name".into()),
    }
}

fn unknown_region(text: &str, tail: &str) -> AddrError {
    let names: Vec<&str> = ALL_REGIONS.iter().map(|&r| region_name(r)).collect();
    AddrError::Syntax {
        text: text.to_string(),
        hint: Some(format!(
            "wram:{} (a region is one of {}; a bank is hex)",
            tail.trim(),
            names.join(", ")
        )),
    }
}

/// A number as the grammar writes it: hex with `0x`, bare below 10 where
/// hex and decimal agree.
fn number(n: u64) -> String {
    if n < 10 {
        n.to_string()
    } else {
        format!("{n:#X}")
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

#[cfg(test)]
mod tests;
