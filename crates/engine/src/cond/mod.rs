//! The condition language (design §8): side-effect free by construction,
//! since evaluation reads only through `region`, `io_register` and
//! `registers`, never `bus_read`. Used by breakpoint and watch conditions.

mod cadence;
pub(crate) mod eval;
mod lex;
mod names;
mod parse;

use std::fmt;

use gbcli_core::{BankSel, CoreError, Region};

use crate::sym::{Symbol, Symbols};

pub use eval::{AccessValues, EvalCtx, EvalError, EvalWarning};

/// A compiled condition: its source and its tree, symbols already resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Condition {
    source: String,
    expr: Expr,
}

impl Condition {
    /// Parses `text`, resolving symbols in `symbols` (design §7).
    pub fn compile(text: &str, symbols: Option<&Symbols>) -> Result<Condition, ExprError> {
        let expr = parse::parse(text, symbols)?;
        Ok(Condition {
            source: text.to_string(),
            expr,
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    /// The value, on i64 with wrapping arithmetic.
    pub fn eval(
        &self,
        ctx: &EvalCtx<'_>,
        warnings: &mut Vec<EvalWarning>,
    ) -> Result<i64, EvalError> {
        eval::eval(&self.expr, ctx, warnings)
    }

    /// Whether the value is nonzero.
    pub fn holds(
        &self,
        ctx: &EvalCtx<'_>,
        warnings: &mut Vec<EvalWarning>,
    ) -> Result<bool, EvalError> {
        Ok(self.eval(ctx, warnings)? != 0)
    }

    /// The finest trigger this condition needs (design §8, §11): the cadence
    /// the run evaluates it on, and thus what it costs.
    pub fn cadence(&self) -> gbcli_protocol::Cadence {
        cadence::analyze(&self.expr).cadence
    }

    /// The static addresses to arm with the write hook, one per byte of each
    /// read. Used for every cadence but `EveryInstruction`, which already
    /// sees every write.
    ///
    /// [`cadence`]: Condition::cadence
    pub(crate) fn write_candidates(&self) -> Vec<(u16, BankSel)> {
        cadence::analyze(&self.expr).writes
    }
}

/// A CPU register an expression names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reg {
    A,
    F,
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
    Pc,
}

/// A flag bit of F.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flag {
    Z,
    N,
    H,
    C,
}

/// How many bytes a read takes and how to extend them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Width {
    U8,
    U16,
    S8,
    S16,
}

/// What a memory read reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    /// A CPU address in the banks mapped now.
    Cpu(Box<Expr>),
    /// An address in a given bank, whatever is mapped (`03:4123`, a banked symbol).
    Banked { bank: u16, addr: u16 },
    /// An offset into a region's whole backing store (`wram:0x1A0`).
    Region { region: Region, offset: Box<Expr> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
    LogicalNot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Mul,
    Div,
    Rem,
    Add,
    Sub,
    Shl,
    Shr,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    BitAnd,
    BitXor,
    BitOr,
    And,
    Or,
}

/// A condition's tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    Num(i64),
    Reg(Reg),
    Flag(Flag),
    Ime,
    /// The ROM bank mapped now.
    Bank,
    /// Vblanks since the time base.
    Frame,
    /// CPU T-cycles since the time base.
    T,
    Ly,
    /// In a watch condition: the byte before the access.
    Old,
    /// In a watch condition: the byte read or written.
    New,
    /// A symbol stands for its address.
    Sym {
        name: String,
        bank: u16,
        addr: u16,
    },
    Read {
        width: Width,
        place: Place,
    },
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
}

/// A condition that doesn't compile. Protocol codes: `bad_expression`,
/// `unknown_symbol`, `ambiguous_symbol`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExprError {
    Syntax {
        /// Byte offset in the source.
        at: usize,
        message: String,
        /// A corrected condition, when one is obvious.
        hint: Option<String>,
    },
    UnknownSymbol {
        name: String,
        near: Vec<String>,
    },
    AmbiguousSymbol {
        name: String,
        candidates: Vec<Symbol>,
    },
}

impl ExprError {
    pub(crate) fn syntax(at: usize, message: impl Into<String>) -> ExprError {
        ExprError::Syntax {
            at,
            message: message.into(),
            hint: None,
        }
    }

    pub fn remedy(&self) -> String {
        match self {
            ExprError::Syntax { hint: Some(h), .. } => format!("did you mean {h}"),
            ExprError::Syntax { .. } => "see `gbcli help expressions`".into(),
            ExprError::UnknownSymbol { near, .. } if near.is_empty() => {
                "check the name against `gbcli sym <pattern>`, or load the ROM's .sym".into()
            }
            ExprError::UnknownSymbol { near, .. } => format!("did you mean {}?", near.join(", ")),
            ExprError::AmbiguousSymbol { candidates, .. } => {
                let list: Vec<String> = candidates
                    .iter()
                    .map(|s| format!("{:02X}:{:04X}", s.bank, s.addr))
                    .collect();
                format!("qualify it with a bank: one of {}", list.join(", "))
            }
        }
    }
}

impl fmt::Display for ExprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExprError::Syntax { at, message, .. } => write!(f, "at {at}: {message}"),
            ExprError::UnknownSymbol { name, .. } => write!(f, "unknown symbol {name:?}"),
            ExprError::AmbiguousSymbol { name, candidates } => {
                write!(f, "{name:?} names {} symbols", candidates.len())
            }
        }
    }
}

impl std::error::Error for ExprError {}

impl From<CoreError> for EvalError {
    fn from(e: CoreError) -> Self {
        EvalError::Core(e)
    }
}

#[cfg(test)]
mod tests;
