//! The rebuilt-ROM check (design §6): RAM can't be rebased, so a checkpoint
//! loads under a changed ROM only if nothing the state depends on moved.

use gbcli_core::Core;
use gbcli_protocol::{CheckpointHeader, LiveSymbol, SymbolRole};

use crate::checkpoint_error::{CheckpointError, Moved, MovedRole, SymbolAt, SymbolSide};
use crate::mem;
use crate::sym::{Area, Symbols};

/// How far above SP the stack scan looks, in bytes.
const STACK_SCAN: u32 = 128;

/// The symbols a state depends on: the ROM symbol containing PC, every RAM
/// symbol, and the ROM symbol containing each possible return address on the
/// stack.
///
/// The stack's top isn't known, so the scan runs from SP up to the first RAM
/// symbol above SP (a `wStackTop` label, or the next variable), the end of
/// SP's area, or 128 bytes, whichever is nearest, and stops at the first
/// never-written byte. Every word in that range that points into a ROM
/// symbol counts; a pushed value that happens to look like one makes the
/// check stricter, never looser.
pub fn live_symbols(core: &dyn Core, syms: &Symbols) -> Vec<LiveSymbol> {
    let mut live: Vec<LiveSymbol> = Vec::new();
    let mut add = |name: &str, bank: u16, addr: u16, role: SymbolRole| {
        if !live.iter().any(|l| l.name == name && l.role == role) {
            live.push(LiveSymbol {
                name: name.to_string(),
                bank,
                addr,
                role,
            });
        }
    };
    let regs = core.registers();
    if Area::of(regs.pc).is_rom()
        && let Some(bank) = mem::cart_bank_at(core, regs.pc)
        && let Some(s) = syms.containing(bank, regs.pc)
    {
        add(&s.name, s.bank, s.addr, SymbolRole::Pc);
    }
    for s in syms.iter().filter(|s| Area::of(s.addr).is_ram()) {
        add(&s.name, s.bank, s.addr, SymbolRole::Ram);
    }
    for word in stack_words(core, syms) {
        if Area::of(word).is_rom()
            && let Some(bank) = mem::cart_bank_at(core, word)
            && let Some(s) = syms.containing(bank, word)
        {
            add(&s.name, s.bank, s.addr, SymbolRole::Stack);
        }
    }
    live
}

fn stack_words(core: &dyn Core, syms: &Symbols) -> Vec<u16> {
    let sp = core.registers().sp;
    let area = Area::of(sp);
    let area_end: u32 = match area {
        Area::Wram0 => 0xD000,
        Area::WramX => 0xE000,
        Area::Hram => 0xFFFF,
        _ => return Vec::new(),
    };
    let next_symbol = syms
        .iter()
        .filter(|s| Area::of(s.addr) == area && s.addr > sp)
        .map(|s| u32::from(s.addr))
        .min()
        .unwrap_or(area_end);
    let limit = area_end.min(next_symbol).min(u32::from(sp) + STACK_SCAN);
    let mut words = Vec::new();
    let mut at = u32::from(sp);
    while at + 1 < limit {
        let (lo, hi) = (at as u16, at as u16 + 1);
        if mem::never_written(core, lo) || mem::never_written(core, hi) {
            break;
        }
        match (mem::peek(core, lo), mem::peek(core, hi)) {
            (Ok(lo), Ok(hi)) => words.push(u16::from_le_bytes([lo, hi])),
            _ => break,
        }
        at += 2;
    }
    words
}

/// What a passing check compared.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RomCheck {
    /// Live symbols compared.
    pub checked: usize,
}

/// Compares the saved state's live symbols against the current ROM's `.sym`.
/// Refuses if any moved or there's nothing to compare by. A breakpoint's
/// symbol is not checked: one set by a symbol follows it into the new build
/// ([`Machine::attach_debugger`](crate::Machine::attach_debugger)).
pub fn check(
    header: &CheckpointHeader,
    current: Option<&Symbols>,
) -> Result<RomCheck, CheckpointError> {
    let current = match (header.sym_sha256.is_some(), current) {
        (true, Some(syms)) => syms,
        (false, None) => return Err(missing(SymbolSide::Both)),
        (false, Some(_)) => return Err(missing(SymbolSide::Saved)),
        (true, None) => return Err(missing(SymbolSide::Current)),
    };
    let at = |name: &str| {
        current.get(name).map(|s| SymbolAt {
            bank: s.bank,
            addr: s.addr,
        })
    };
    let mut moved = Vec::new();
    let mut checked = 0;
    for live in &header.symbols_live {
        checked += 1;
        let old = SymbolAt {
            bank: live.bank,
            addr: live.addr,
        };
        let new = at(&live.name);
        if new != Some(old) {
            let role = match live.role {
                SymbolRole::Pc => MovedRole::Pc,
                SymbolRole::Ram => MovedRole::Ram,
                SymbolRole::Stack => MovedRole::Stack,
            };
            moved.push(Moved {
                name: live.name.clone(),
                role,
                old,
                new,
            });
        }
    }
    if moved.is_empty() {
        Ok(RomCheck { checked })
    } else {
        Err(CheckpointError::RomChanged { moved })
    }
}

fn missing(side: SymbolSide) -> CheckpointError {
    CheckpointError::SymbolsMissing { side }
}
