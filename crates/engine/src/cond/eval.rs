//! Evaluation over direct access only.

use std::fmt;

use gbcli_core::{Core, CoreError, Region};
use gbcli_protocol::TimeMark;

use super::{BinOp, Expr, Flag, Place, Reg, UnOp, Width};
use crate::mem::{self, Target};
use crate::sym::Symbols;

/// A watch hit's bytes, for `old` and `new`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccessValues {
    /// `None` when the core couldn't know it without a bus read.
    pub old: Option<u8>,
    pub new: u8,
}

/// What an expression is evaluated against.
pub struct EvalCtx<'a> {
    pub core: &'a dyn Core,
    pub symbols: Option<&'a Symbols>,
    /// Set inside a watch condition.
    pub access: Option<AccessValues>,
    /// The time base `frame` and `t` count from.
    pub base: TimeMark,
    /// Reading never-written RAM warns instead of failing.
    pub allow_uninit: bool,
}

/// A read of RAM the program has never written (design §8, DX 2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UninitRead {
    /// The CPU address, when the read was by address.
    pub addr: Option<u16>,
    pub region: Region,
    pub offset: usize,
    pub symbol: Option<String>,
}

impl fmt::Display for UninitRead {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.symbol, self.addr) {
            (Some(s), Some(a)) => write!(f, "{s} (${a:04X})"),
            (None, Some(a)) => write!(f, "${a:04X}"),
            _ => write!(f, "{:?}:{:#x}", self.region, self.offset),
        }
    }
}

/// Something an evaluation let through but wants reported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EvalWarning {
    UninitializedRead(UninitRead),
}

/// Why an evaluation failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EvalError {
    /// `uninitialized_read`: the remedy points at the time base.
    UninitializedRead(UninitRead),
    Core(CoreError),
    DivideByZero,
    /// `old` or `new` outside a watch condition.
    NoAccess {
        which: &'static str,
    },
    /// `old` on an access whose prior byte the core couldn't know.
    OldUnknown,
    ShiftOutOfRange {
        by: i64,
    },
    AddressOutOfRange {
        value: i64,
    },
}

impl EvalError {
    pub fn remedy(&self) -> String {
        match self {
            EvalError::UninitializedRead(r) => format!(
                "{r} has never been written at this point; run from boot (--from boot) or \
                 later, or pass --allow-uninit"
            ),
            EvalError::Core(e) => format!("the core can't read this without side effects: {e}"),
            EvalError::DivideByZero => "guard the divisor, e.g. `d != 0 && x / d > 2`".into(),
            EvalError::NoAccess { which } => {
                format!("`{which}` only means something in a watch condition")
            }
            EvalError::OldUnknown => {
                "this access's prior byte is unknown; compare `new` instead".into()
            }
            EvalError::ShiftOutOfRange { .. } => "shift by 0 to 63".into(),
            EvalError::AddressOutOfRange { .. } => "addresses run from $0000 to $FFFF".into(),
        }
    }
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::UninitializedRead(r) => write!(f, "uninitialized read of {r}"),
            EvalError::Core(e) => write!(f, "{e}"),
            EvalError::DivideByZero => write!(f, "division by zero"),
            EvalError::NoAccess { which } => write!(f, "`{which}` outside a watch condition"),
            EvalError::OldUnknown => write!(f, "`old` is unknown for this access"),
            EvalError::ShiftOutOfRange { by } => write!(f, "shift by {by}"),
            EvalError::AddressOutOfRange { value } => write!(f, "address {value:#x} out of range"),
        }
    }
}

impl std::error::Error for EvalError {}

pub fn eval(
    e: &Expr,
    ctx: &EvalCtx<'_>,
    warnings: &mut Vec<EvalWarning>,
) -> Result<i64, EvalError> {
    let core = ctx.core;
    Ok(match e {
        Expr::Num(n) => *n,
        Expr::Reg(r) => register(core, *r),
        Expr::Flag(flag) => {
            let bit = match flag {
                Flag::Z => 7,
                Flag::N => 6,
                Flag::H => 5,
                Flag::C => 4,
            };
            i64::from(core.registers().f >> bit & 1)
        }
        Expr::Ime => i64::from(core.registers().ime),
        Expr::Bank => i64::from(core.region(Region::Rom)?.bank),
        Expr::Frame => frame_number(core) as i64 - ctx.base.frame as i64,
        Expr::T => {
            let per_t = if core.registers().double_speed { 1 } else { 2 };
            (core.now().0 as i64 - ctx.base.ticks as i64) / per_t
        }
        Expr::Ly => i64::from(core.ppu().ly),
        Expr::Old => {
            let access = ctx.access.ok_or(EvalError::NoAccess { which: "old" })?;
            i64::from(access.old.ok_or(EvalError::OldUnknown)?)
        }
        Expr::New => i64::from(ctx.access.ok_or(EvalError::NoAccess { which: "new" })?.new),
        Expr::Sym { addr, .. } => i64::from(*addr),
        Expr::Read { width, place } => read(*width, place, ctx, warnings)?,
        Expr::Unary(op, inner) => {
            let v = eval(inner, ctx, warnings)?;
            match op {
                UnOp::Neg => v.wrapping_neg(),
                UnOp::Not => !v,
                UnOp::LogicalNot => i64::from(v == 0),
            }
        }
        Expr::Binary(op, l, r) => {
            let a = eval(l, ctx, warnings)?;
            // Short-circuit, so `d != 0 && x / d > 2` is safe.
            match op {
                BinOp::And if a == 0 => return Ok(0),
                BinOp::Or if a != 0 => return Ok(1),
                _ => {}
            }
            let b = eval(r, ctx, warnings)?;
            binary(*op, a, b)?
        }
    })
}

fn binary(op: BinOp, a: i64, b: i64) -> Result<i64, EvalError> {
    let shift = |b: i64| {
        u32::try_from(b)
            .ok()
            .filter(|&b| b < 64)
            .ok_or(EvalError::ShiftOutOfRange { by: b })
    };
    Ok(match op {
        BinOp::Mul => a.wrapping_mul(b),
        BinOp::Div | BinOp::Rem if b == 0 => return Err(EvalError::DivideByZero),
        BinOp::Div => a.wrapping_div(b),
        BinOp::Rem => a.wrapping_rem(b),
        BinOp::Add => a.wrapping_add(b),
        BinOp::Sub => a.wrapping_sub(b),
        BinOp::Shl => a.wrapping_shl(shift(b)?),
        BinOp::Shr => a.wrapping_shr(shift(b)?),
        BinOp::Lt => i64::from(a < b),
        BinOp::Le => i64::from(a <= b),
        BinOp::Gt => i64::from(a > b),
        BinOp::Ge => i64::from(a >= b),
        BinOp::Eq => i64::from(a == b),
        BinOp::Ne => i64::from(a != b),
        BinOp::BitAnd => a & b,
        BinOp::BitXor => a ^ b,
        BinOp::BitOr => a | b,
        BinOp::And | BinOp::Or => i64::from(b != 0),
    })
}

fn register(core: &dyn Core, r: Reg) -> i64 {
    let regs = core.registers();
    let pair = |hi: u8, lo: u8| i64::from(u16::from_be_bytes([hi, lo]));
    match r {
        Reg::A => i64::from(regs.a),
        Reg::F => i64::from(regs.f),
        Reg::B => i64::from(regs.b),
        Reg::C => i64::from(regs.c),
        Reg::D => i64::from(regs.d),
        Reg::E => i64::from(regs.e),
        Reg::H => i64::from(regs.h),
        Reg::L => i64::from(regs.l),
        Reg::Af => pair(regs.a, regs.f),
        Reg::Bc => pair(regs.b, regs.c),
        Reg::De => pair(regs.d, regs.e),
        Reg::Hl => pair(regs.h, regs.l),
        Reg::Sp => i64::from(regs.sp),
        Reg::Pc => i64::from(regs.pc),
    }
}

/// Vblanks since power-on.
pub fn frame_number(core: &dyn Core) -> u64 {
    core.last_complete_frame().map_or(0, |f| f.tag.number)
}

fn read(
    width: Width,
    place: &Place,
    ctx: &EvalCtx<'_>,
    warnings: &mut Vec<EvalWarning>,
) -> Result<i64, EvalError> {
    let len = match width {
        Width::U8 | Width::S8 => 1,
        Width::U16 | Width::S16 => 2,
    };
    let mut bytes = [0u8; 2];
    for (i, byte) in bytes.iter_mut().enumerate().take(len) {
        let (target, addr) = match place {
            Place::Cpu(e) => {
                let value = eval(e, ctx, warnings)?;
                let base =
                    u16::try_from(value).map_err(|_| EvalError::AddressOutOfRange { value })?;
                let addr = base.wrapping_add(i as u16);
                (mem::target(ctx.core, addr)?, Some(addr))
            }
            Place::Banked { bank, addr } => {
                let addr = addr.wrapping_add(i as u16);
                (mem::banked_target(*bank, addr)?, Some(addr))
            }
            Place::Region { region, offset } => {
                let value = eval(offset, ctx, warnings)?;
                let offset =
                    usize::try_from(value).map_err(|_| EvalError::AddressOutOfRange { value })? + i;
                (
                    Target::Backing {
                        region: *region,
                        offset,
                    },
                    None,
                )
            }
        };
        if let Target::Backing { region, offset } = target {
            check_written(ctx, region, offset, addr, warnings)?;
        }
        *byte = match target {
            Target::Backing { region, offset } => mem::read_backing(ctx.core, region, offset)?,
            Target::Io(reg) => ctx.core.io_register(reg)?,
        };
    }
    Ok(match width {
        Width::U8 => i64::from(bytes[0]),
        Width::S8 => i64::from(bytes[0] as i8),
        Width::U16 => i64::from(u16::from_le_bytes(bytes)),
        Width::S16 => i64::from(i16::from_le_bytes(bytes)),
    })
}

fn check_written(
    ctx: &EvalCtx<'_>,
    region: Region,
    offset: usize,
    addr: Option<u16>,
    warnings: &mut Vec<EvalWarning>,
) -> Result<(), EvalError> {
    if !matches!(region, Region::Wram | Region::Hram | Region::Sram) {
        return Ok(());
    }
    let Ok(written) = ctx.core.written(region) else {
        return Ok(());
    };
    if offset >= written.len || !written.never_written(offset) {
        return Ok(());
    }
    let symbol = addr.and_then(|a| {
        let s = ctx.symbols?.containing(mem::bank_at(ctx.core, a), a)?;
        Some(if s.addr == a {
            s.name.clone()
        } else {
            format!("{}+{}", s.name, a - s.addr)
        })
    });
    let read = UninitRead {
        addr,
        region,
        offset,
        symbol,
    };
    if ctx.allow_uninit {
        warnings.push(EvalWarning::UninitializedRead(read));
        Ok(())
    } else {
        Err(EvalError::UninitializedRead(read))
    }
}
