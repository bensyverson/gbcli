//! The opcode table's types; `table.rs` holds one [`OpInfo`] per opcode.

/// SM83 mnemonics, spelled as the opcode table spells them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mnemonic {
    Adc,
    Add,
    And,
    Bit,
    Call,
    Ccf,
    Cp,
    Cpl,
    Daa,
    Dec,
    Di,
    Ei,
    Halt,
    /// One of the eleven opcodes with no instruction; the CPU locks up.
    Illegal,
    Inc,
    Jp,
    Jr,
    Ld,
    Ldh,
    Nop,
    Or,
    Pop,
    /// 0xCB, which selects the CB table; never decoded on its own.
    Prefix,
    Push,
    Res,
    Ret,
    Reti,
    Rl,
    Rla,
    Rlc,
    Rlca,
    Rr,
    Rra,
    Rrc,
    Rrca,
    Rst,
    Sbc,
    Scf,
    Set,
    Sla,
    Sra,
    Srl,
    Stop,
    Sub,
    Swap,
    Xor,
}

impl Mnemonic {
    pub fn as_str(self) -> &'static str {
        use Mnemonic::*;
        match self {
            Adc => "ADC",
            Add => "ADD",
            And => "AND",
            Bit => "BIT",
            Call => "CALL",
            Ccf => "CCF",
            Cp => "CP",
            Cpl => "CPL",
            Daa => "DAA",
            Dec => "DEC",
            Di => "DI",
            Ei => "EI",
            Halt => "HALT",
            Illegal => "ILLEGAL",
            Inc => "INC",
            Jp => "JP",
            Jr => "JR",
            Ld => "LD",
            Ldh => "LDH",
            Nop => "NOP",
            Or => "OR",
            Pop => "POP",
            Prefix => "PREFIX",
            Push => "PUSH",
            Res => "RES",
            Ret => "RET",
            Reti => "RETI",
            Rl => "RL",
            Rla => "RLA",
            Rlc => "RLC",
            Rlca => "RLCA",
            Rr => "RR",
            Rra => "RRA",
            Rrc => "RRC",
            Rrca => "RRCA",
            Rst => "RST",
            Sbc => "SBC",
            Scf => "SCF",
            Set => "SET",
            Sla => "SLA",
            Sra => "SRA",
            Srl => "SRL",
            Stop => "STOP",
            Sub => "SUB",
            Swap => "SWAP",
            Xor => "XOR",
        }
    }
}

/// What an operand is. The table writes register C and condition C alike;
/// these tell them apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OperandKind {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
    Af,
    Bc,
    De,
    Hl,
    Sp,
    CondNz,
    CondZ,
    CondNc,
    CondC,
    /// An 8-bit immediate.
    N8,
    /// A 16-bit immediate.
    N16,
    /// An address in FF00–FFFF, as its low byte.
    A8,
    /// A 16-bit address.
    A16,
    /// JR's signed displacement from the next instruction.
    Rel8,
    /// A signed 8-bit offset added to SP.
    E8,
    /// An RST target.
    Vector(u8),
    /// A bit number, 0–7.
    Bit(u8),
}

impl OperandKind {
    /// The name the opcode table gives it.
    pub fn name(self) -> &'static str {
        use OperandKind::*;
        const VECTORS: [&str; 8] = ["$00", "$08", "$10", "$18", "$20", "$28", "$30", "$38"];
        const BITS: [&str; 8] = ["0", "1", "2", "3", "4", "5", "6", "7"];
        match self {
            A => "A",
            B => "B",
            C | CondC => "C",
            D => "D",
            E => "E",
            H => "H",
            L => "L",
            Af => "AF",
            Bc => "BC",
            De => "DE",
            Hl => "HL",
            Sp => "SP",
            CondNz => "NZ",
            CondZ => "Z",
            CondNc => "NC",
            N8 => "n8",
            N16 => "n16",
            A8 => "a8",
            A16 => "a16",
            Rel8 | E8 => "e8",
            Vector(v) => VECTORS[usize::from(v / 8) & 7],
            Bit(b) => BITS[usize::from(b) & 7],
        }
    }

    /// Bytes of instruction data the operand takes.
    pub(super) fn data_len(self) -> usize {
        match self {
            OperandKind::N8 | OperandKind::A8 | OperandKind::Rel8 | OperandKind::E8 => 1,
            OperandKind::N16 | OperandKind::A16 => 2,
            _ => 0,
        }
    }
}

/// An operand as the table describes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spec {
    pub kind: OperandKind,
    /// Memory at the operand (`[HL]`), not the operand itself.
    pub indirect: bool,
    /// `[HL+]`, and SP in `LD HL, SP+e8`.
    pub increment: bool,
    /// `[HL-]`.
    pub decrement: bool,
}

/// T-cycles at single speed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cycles {
    /// The cost, or for a conditional branch the cost when taken.
    pub taken: u8,
    /// A conditional branch's cost when not taken.
    pub not_taken: Option<u8>,
}

/// One opcode's table entry.
#[derive(Clone, Copy, Debug)]
pub(super) struct OpInfo {
    pub mnemonic: Mnemonic,
    pub len: u8,
    pub cycles: Cycles,
    /// Z, N, H and C in that order, as the table writes them: the flag's
    /// letter when the result sets it, 0 or 1 when forced, - when untouched.
    pub flags: &'static str,
    pub operands: &'static [Spec],
}

pub(super) const fn op(
    mnemonic: Mnemonic,
    len: u8,
    taken: u8,
    not_taken: Option<u8>,
    flags: &'static str,
    operands: &'static [Spec],
) -> OpInfo {
    OpInfo {
        mnemonic,
        len,
        cycles: Cycles { taken, not_taken },
        flags,
        operands,
    }
}

pub(super) const fn sp(
    kind: OperandKind,
    indirect: bool,
    increment: bool,
    decrement: bool,
) -> Spec {
    Spec {
        kind,
        indirect,
        increment,
        decrement,
    }
}
