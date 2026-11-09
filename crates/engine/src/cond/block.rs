//! A block of bytes from one place, for `view memory` (design §7). Each byte
//! is read as `[place]` reads it: a CPU address in the banks mapped now, a
//! banked symbol or `bank:addr` in its own bank, `region:offset` in the
//! region's backing store; IO registers without side effects; and a
//! never-written RAM byte an error naming the first, unless `allow_uninit`.

use super::eval::{self, AreaEnd, EvalCtx, EvalError, EvalWarning};
use super::{Expr, ExprError, Place, parse};
use crate::sym::{Area, Symbols};

/// The most a block reads: the whole address space.
pub const MAX_BLOCK: u32 = 0x1_0000;

/// Where a block starts: what `[text]` would read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockStart {
    place: Place,
}

/// The bytes a block read, and where the first one is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    /// The CPU address of the first byte; absent for `region:offset`.
    pub addr: Option<u16>,
    /// The bank read, when the start names one (`03:4000`, a banked symbol).
    pub bank: Option<u16>,
    pub bytes: Vec<u8>,
}

impl BlockStart {
    /// Parses `text` as a read's address, resolving symbols in `symbols`.
    pub fn compile(text: &str, symbols: Option<&Symbols>) -> Result<BlockStart, ExprError> {
        Ok(BlockStart {
            place: parse::parse_place(text, symbols)?,
        })
    }

    /// Reads `len` bytes (1 to [`MAX_BLOCK`]). A block that would run past
    /// $FFFF fails before reading, as does a banked or region block that
    /// would run past its area's end (`BlockPastArea`). With `ctx.allow_uninit`, never-written
    /// bytes are read as their power-on values under one warning.
    pub fn read(
        &self,
        len: u32,
        ctx: &EvalCtx<'_>,
        warnings: &mut Vec<EvalWarning>,
    ) -> Result<Block, EvalError> {
        // The start's expression is evaluated once, so the block is one
        // span even if it names a register.
        let (place, addr, bank) = match &self.place {
            Place::Cpu(e) => {
                let value = eval::eval(e, ctx, warnings)?;
                let addr =
                    u16::try_from(value).map_err(|_| EvalError::AddressOutOfRange { value })?;
                (Place::Cpu(Box::new(Expr::Num(value))), Some(addr), None)
            }
            Place::Banked { bank, addr } => (self.place.clone(), Some(*addr), Some(*bank)),
            Place::Region { .. } => (self.place.clone(), None, None),
        };
        if let Some(addr) = addr {
            let end = u32::from(addr) + len;
            if end > MAX_BLOCK {
                return Err(EvalError::AddressOutOfRange {
                    value: i64::from(end - 1),
                });
            }
        }
        if let Some((end, fits)) = past_area(&place, len, ctx) {
            return Err(EvalError::BlockPastArea { end, fits });
        }
        let mut read = Vec::new();
        let mut bytes = Vec::with_capacity(len as usize);
        for i in 0..len {
            // A fresh list per byte: the read's dedup scans the whole list.
            let mut one = Vec::new();
            bytes.push(eval::byte(&place, i as u16, ctx, &mut one)?);
            read.append(&mut one);
        }
        warnings.extend(collapsed(read));
        Ok(Block { addr, bank, bytes })
    }
}

/// Where a banked or region block of `len` bytes would run past the end of
/// what its bank or offset names, and how many bytes fit before it: a bank
/// carried into the next area would read a bank nothing maps there. A CPU
/// address names no bank, so its block reads on through the mapped banks.
/// A region start already past its end is left to the read's own error.
fn past_area(place: &Place, len: u32, ctx: &EvalCtx<'_>) -> Option<(AreaEnd, u32)> {
    let (end, fits) = match *place {
        Place::Cpu(_) => return None,
        Place::Banked { bank, addr } => {
            let last = Area::of(addr).last();
            (
                AreaEnd::Banked { bank, last },
                u32::from(last) - u32::from(addr) + 1,
            )
        }
        Place::Region { region, offset } => {
            let size = ctx.core.region(region).ok()?.bytes.len();
            let offset = offset as usize;
            if offset >= size {
                return None;
            }
            (
                AreaEnd::Region {
                    region,
                    last: size - 1,
                },
                u32::try_from(size - offset).unwrap_or(u32::MAX),
            )
        }
    };
    (len > fits).then_some((end, fits))
}

/// A block's warnings with its never-written bytes as one.
fn collapsed(read: Vec<EvalWarning>) -> Vec<EvalWarning> {
    let (uninit, mut rest): (Vec<_>, Vec<_>) = read
        .into_iter()
        .partition(|w| matches!(w, EvalWarning::UninitializedRead(_)));
    let count = uninit.len();
    match uninit.into_iter().next() {
        Some(single) if count == 1 => rest.insert(0, single),
        Some(EvalWarning::UninitializedRead(first)) => {
            rest.insert(0, EvalWarning::UninitializedBytes { first, count })
        }
        _ => {}
    }
    rest
}
