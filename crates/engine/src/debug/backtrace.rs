//! A backtrace from the stack, not a shadow call stack: tracking every CALL
//! and RET would need an execution hook on every instruction, which SameBoy's
//! backend doesn't offer (event sinks are unsupported), so this scans instead.
//! A stack word counts as a return address when the bytes before it are a
//! CALL (3 bytes) or an RST (1 byte). Data that looks like one shows up too.

use gbcli_core::Core;

use crate::mem;
use crate::sym::{Area, Symbols};

/// How far above SP the scan looks, in bytes.
const SCAN: u32 = 256;

/// A return address on the stack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallFrame {
    /// Where the word sits on the stack.
    pub sp: u16,
    pub return_addr: u16,
    /// The bank mapped at `return_addr` now; a far call may have returned
    /// into another.
    pub bank: u16,
    /// `Name` or `Name+3`.
    pub symbol: Option<String>,
}

pub fn backtrace(core: &dyn Core, syms: Option<&Symbols>) -> Vec<CallFrame> {
    let sp = core.registers().sp;
    let end: u32 = match Area::of(sp) {
        Area::Wram0 | Area::WramX => 0xE000,
        Area::Hram => 0xFFFF,
        _ => return Vec::new(),
    };
    let limit = end.min(u32::from(sp) + SCAN);
    let mut frames = Vec::new();
    let mut at = u32::from(sp);
    while at + 1 < limit {
        let (lo, hi) = (at as u16, at as u16 + 1);
        if mem::never_written(core, lo) || mem::never_written(core, hi) {
            break;
        }
        let (Ok(l), Ok(h)) = (mem::peek(core, lo), mem::peek(core, hi)) else {
            break;
        };
        let word = u16::from_le_bytes([l, h]);
        if Area::of(word).is_rom() && after_call(core, word) {
            let bank = mem::bank_at(core, word);
            let symbol = syms.and_then(|s| s.containing(bank, word)).map(|s| {
                if s.addr == word {
                    s.name.clone()
                } else {
                    format!("{}+{}", s.name, word - s.addr)
                }
            });
            frames.push(CallFrame {
                sp: lo,
                return_addr: word,
                bank,
                symbol,
            });
        }
        at += 2;
    }
    frames
}

fn after_call(core: &dyn Core, ret: u16) -> bool {
    let byte = |back: u16| mem::peek(core, ret.wrapping_sub(back)).ok();
    matches!(byte(3), Some(0xCD | 0xC4 | 0xCC | 0xD4 | 0xDC))
        || matches!(
            byte(1),
            Some(0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF)
        )
}
