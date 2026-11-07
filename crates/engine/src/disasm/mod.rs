//! A structured SM83 disassembler built from gbdev's opcode table
//! (`crates/engine/data/gb-opcodes/`, CC0-1.0; `table.rs` is generated from it
//! by `scripts/disasm/gen_opcodes.py`). Symbols come from an injected
//! [`SymbolLookup`], so this module never parses a `.sym` file.

mod lookup;
mod table;
mod types;

pub use lookup::{Form, LookupError, form, lookup};
pub use types::{Cycles, Mnemonic, OperandKind, Spec};

use crate::inspect::hwreg;

/// Which table an opcode is from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Opcode {
    Base(u8),
    /// The byte after 0xCB.
    Cb(u8),
}

/// A symbol and the offset from it, as in `MainLoop+3`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolRef {
    pub name: String,
    pub offset: u16,
}

/// Names addresses. The implementor knows the banks in play (for example the
/// mapped ROM bank), so lookups take only the CPU address.
pub trait SymbolLookup {
    fn symbol(&self, addr: u16) -> Option<SymbolRef>;
}

/// A lookup with no symbols; hardware register names still apply.
pub struct NoSymbols;

impl SymbolLookup for NoSymbols {
    fn symbol(&self, _addr: u16) -> Option<SymbolRef> {
        None
    }
}

/// An operand's data from the instruction bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value {
    U8(u8),
    U16(u16),
    I8(i8),
}

/// A decoded operand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Operand {
    pub kind: OperandKind,
    pub indirect: bool,
    pub increment: bool,
    pub decrement: bool,
    /// For immediates, addresses and displacements.
    pub value: Option<Value>,
    /// The address the operand names: an a16 or a8 address, a JR target or an
    /// RST vector. Not set for n16, which may be a plain number.
    pub address: Option<u16>,
    /// The symbol at `address`, else the hardware register there.
    pub symbol: Option<SymbolRef>,
}

/// One decoded instruction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instruction {
    pub addr: u16,
    pub opcode: Opcode,
    pub mnemonic: Mnemonic,
    /// Bytes, including the 0xCB prefix.
    pub len: u8,
    pub bytes: Vec<u8>,
    pub cycles: Cycles,
    pub operands: Vec<Operand>,
    /// An opcode with no instruction: executing it locks the CPU.
    pub illegal: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// The instruction needs `need` bytes; only `have` were given.
    Truncated { need: u8, have: usize },
}

/// Decodes the instruction at the start of `bytes`, which sits at `addr`.
pub fn decode(
    bytes: &[u8],
    addr: u16,
    symbols: &dyn SymbolLookup,
) -> Result<Instruction, DecodeError> {
    let truncated = |need| DecodeError::Truncated {
        need,
        have: bytes.len(),
    };
    let (opcode, info) = match bytes {
        [] => return Err(truncated(1)),
        [0xCB] => return Err(truncated(2)),
        [0xCB, op, ..] => (Opcode::Cb(*op), &table::CB[usize::from(*op)]),
        [op, ..] => (Opcode::Base(*op), &table::BASE[usize::from(*op)]),
    };
    let len = usize::from(info.len);
    let bytes = bytes.get(..len).ok_or(truncated(info.len))?;
    let mut data = &bytes[len - data_len(info.operands)..];
    let next = addr.wrapping_add(u16::from(info.len));
    let operands = info
        .operands
        .iter()
        .map(|spec| {
            let (field, rest) = data.split_at(spec.kind.data_len());
            data = rest;
            operand(spec, field, next, symbols)
        })
        .collect();
    Ok(Instruction {
        addr,
        opcode,
        mnemonic: info.mnemonic,
        len: info.len,
        bytes: bytes.to_vec(),
        cycles: info.cycles,
        operands,
        illegal: info.mnemonic == Mnemonic::Illegal,
    })
}

fn data_len(specs: &[Spec]) -> usize {
    specs.iter().map(|s| s.kind.data_len()).sum()
}

fn operand(spec: &Spec, field: &[u8], next: u16, symbols: &dyn SymbolLookup) -> Operand {
    let (value, address) = match (spec.kind, field) {
        (OperandKind::N8, &[b]) => (Some(Value::U8(b)), None),
        (OperandKind::A8, &[b]) => (Some(Value::U8(b)), Some(0xFF00 | u16::from(b))),
        (OperandKind::E8, &[b]) => (Some(Value::I8(b as i8)), None),
        (OperandKind::Rel8, &[b]) => (
            Some(Value::I8(b as i8)),
            Some(next.wrapping_add_signed(i16::from(b as i8))),
        ),
        (OperandKind::N16, &[lo, hi]) => (Some(Value::U16(u16::from_le_bytes([lo, hi]))), None),
        (OperandKind::A16, &[lo, hi]) => {
            let a = u16::from_le_bytes([lo, hi]);
            (Some(Value::U16(a)), Some(a))
        }
        (OperandKind::Vector(v), _) => (None, Some(u16::from(v))),
        _ => (None, None),
    };
    Operand {
        kind: spec.kind,
        indirect: spec.indirect,
        increment: spec.increment,
        decrement: spec.decrement,
        value,
        address,
        symbol: address.and_then(|a| symbols.symbol(a).or_else(|| hardware_register(a))),
    }
}

fn hardware_register(addr: u16) -> Option<SymbolRef> {
    hwreg::by_addr(addr).map(|r| SymbolRef {
        name: r.name.to_owned(),
        offset: 0,
    })
}
