//! A symbol plus or minus an offset (`Main+3`, `Work-$10`): split, resolved
//! exactly or refused, and labeled so the grammar reads the label back.

use gbcli_core::canonical_addr;
use gbcli_protocol::Location;

use super::{AddrError, hex};
use crate::inspect::hwreg;
use crate::sym::{Area, Symbol};

/// A switchable area: its name and its CPU span. A symbol there carries its
/// bank, which means nothing outside the span.
pub(crate) fn banked_area(area: Area) -> Option<(&'static str, u16, u16)> {
    match area {
        Area::RomX => Some(("ROMX", 0x4000, 0x7FFF)),
        Area::Vram => Some(("VRAM", 0x8000, 0x9FFF)),
        Area::Sram => Some(("SRAM", 0xA000, 0xBFFF)),
        Area::WramX => Some(("WRAMX", 0xD000, 0xDFFF)),
        _ => None,
    }
}

/// A bank for a symbol, when its area can be switched.
pub(crate) fn bank_of(symbol: &Symbol) -> Option<u16> {
    banked_area(Area::of(symbol.addr)).map(|_| symbol.bank)
}

/// Splits `Name+3`, `Name+$3` and `Name-0x3` (offsets are hex, prefixed or
/// not); a name with no sign comes back with offset 0.
pub(crate) fn split_offset(text: &str) -> Result<(&str, i64), AddrError> {
    let Some(at) = text.rfind(['+', '-']) else {
        return Ok((text, 0));
    };
    let base = text[..at].trim();
    let negative = text[at..].starts_with('-');
    let bad = |hint: String| AddrError::Syntax {
        text: text.to_string(),
        hint: Some(hint),
    };
    if base.is_empty() {
        return Err(bad("a symbol before the offset, such as Main+$10".into()));
    }
    let Some(n) = hex(&text[at + 1..]) else {
        return Err(bad(format!("{base}+$10 (a hex offset)")));
    };
    let n = i64::try_from(n).unwrap_or(i64::MAX);
    Ok((base, if negative { -n } else { n }))
}

/// `name` moved by `delta`, written as the grammar reads it.
pub fn label(name: &str, delta: i64) -> String {
    match delta {
        0 => name.to_string(),
        d if d > 0 => format!("{name}+${d:X}"),
        d => format!("{name}-${:X}", d.unsigned_abs()),
    }
}

/// Splits a stored label (`Name+$3`, `Name-$3`) into its symbol and offset;
/// anything else is a bare name.
pub fn split_label(label: &str) -> (&str, i64) {
    split_offset(label).unwrap_or((label, 0))
}

/// `symbol` moved by `delta`: exactly there, or refused when it leaves the
/// address space, or the switchable area its bank belongs to.
pub(crate) fn resolve(
    text: &str,
    name: &str,
    delta: i64,
    symbol: &Symbol,
) -> Result<Location, AddrError> {
    let bank = bank_of(symbol);
    let (lo, hi) = banked_area(Area::of(symbol.addr)).map_or((0, 0xFFFF), |(_, lo, hi)| (lo, hi));
    let target = i64::from(symbol.addr).saturating_add(delta);
    if target < i64::from(lo) || target > i64::from(hi) {
        return Err(AddrError::SymbolOffset {
            text: text.to_string(),
            name: name.to_string(),
            addr: symbol.addr,
            bank,
        });
    }
    Ok(Location {
        addr: canonical_addr(target as u16),
        bank,
        symbol: Some(label(name, delta)),
    })
}

/// A hardware register name, alone or with an offset (`rLCDC+1`), from
/// inspection's table; `None` when `text` names no register. The whole text
/// is tried first: hardware.inc names wave RAM bytes `_AUD3WAVERAM+10` in
/// decimal, where the grammar's offsets are hex. The hardware fixes the
/// address, not a `.sym`, so the location names no symbol for a checkpoint's
/// ROM check to look up again.
pub(crate) fn hardware(text: &str) -> Result<Option<Location>, AddrError> {
    let fixed = |addr| Location {
        addr,
        bank: None,
        symbol: None,
    };
    if let Some(addr) = hwreg::address_of(text) {
        return Ok(Some(fixed(addr)));
    }
    let (name, delta) = split_offset(text)?;
    let Some(addr) = hwreg::address_of(name) else {
        return Ok(None);
    };
    let reg = Symbol {
        name: name.to_string(),
        bank: 0,
        addr,
    };
    resolve(text, name, delta, &reg).map(|at| Some(fixed(at.addr)))
}

/// The message and remedy for a [`AddrError::SymbolOffset`].
pub(crate) fn describe(text: &str, name: &str, addr: u16, bank: Option<u16>) -> (String, String) {
    let area = banked_area(Area::of(addr)).filter(|_| bank.is_some());
    let (lo, hi) = area.map_or((0, 0xFFFF), |(_, lo, hi)| (lo, hi));
    let range = format!(
        "keep the offset between {} and {}",
        label(name, i64::from(lo) - i64::from(addr)),
        label(name, i64::from(hi) - i64::from(addr))
    );
    match (area, bank) {
        (Some((area, ..)), Some(bank)) => (
            format!(
                "{text} leaves {area} (${lo:04X}-${hi:04X}), the only span where \
                 {name}'s bank {bank:02X} applies"
            ),
            format!("{range}, or name the bank and address, such as {bank:02X}:{hi:04X}"),
        ),
        _ => (
            format!("{text} leaves the address space ($0000-$FFFF)"),
            range,
        ),
    }
}
