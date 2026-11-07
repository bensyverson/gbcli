//! What can make a predicate change (design §8, §11): the triggers a run
//! re-evaluates it on.
//!
//! The triggers are a set, not a ladder: `frame` changes at a vblank, which
//! with the LCD off is not a scanline change, so a predicate reading `frame`
//! and `ly` needs both. Memory reads arm the backing bytes the evaluator
//! reads ([`mem::fixed_target`], [`mem::target_in_bank`] or the region form):
//! the write hook matches a write by the byte it lands in (after the bank,
//! a mirror, or OAM DMA's bus conflict moving it), so the two sides share
//! one identity and cannot disagree.
//! Anything that can change without a CPU write to its byte needs the
//! per-instruction baseline.

use gbcli_core::{Region, StoreFilter, Support};
use gbcli_protocol::Cadence;

use super::{BinOp, Expr, Place, UnOp, Width};
use crate::mem::{self, CartSizes, Target};

/// Everything that can make a predicate change.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Triggers {
    /// A vblank (`frame`).
    pub vblank: bool,
    /// An LY change (`ly`).
    pub scanline: bool,
    /// Any instruction: registers, machine values, dynamic addresses, and
    /// memory that changes without a CPU write to it.
    pub every_instruction: bool,
    /// The backing bytes read, which only a CPU write landing there changes.
    /// Unused when every instruction triggers, since that sees every write.
    pub stores: StoreFilter,
}

impl Triggers {
    /// The costliest trigger, which the stop record names: every instruction,
    /// then scanlines (a run checks LY after each instruction), then vblanks,
    /// then writes alone. Writes to [`stores`] always trigger too.
    ///
    /// [`stores`]: Triggers::stores
    pub fn cadence(&self) -> Cadence {
        if self.every_instruction {
            Cadence::EveryInstruction
        } else if self.scanline {
            Cadence::Scanline
        } else if self.vblank {
            Cadence::Vblank
        } else {
            Cadence::Writes
        }
    }
}

/// The triggers of `expr`. `cart_ram` is whether the core places every cart
/// RAM change at its byte (`Feature::CartRamWrites`); without it a cart RAM
/// read falls back to every instruction. `sizes` alias a banked read as the
/// evaluator aliases it (`mem::target_in_bank`).
pub(crate) fn analyze(expr: &Expr, cart_ram: Support, sizes: CartSizes) -> Triggers {
    let mut triggers = Triggers::default();
    let cart = Cart {
        ram: cart_ram,
        sizes,
    };
    walk(expr, cart, &mut triggers);
    triggers
}

/// What the analysis needs to know about the cartridge.
#[derive(Clone, Copy)]
struct Cart {
    ram: Support,
    sizes: CartSizes,
}

fn walk(e: &Expr, cart: Cart, out: &mut Triggers) {
    match e {
        Expr::Num(_) | Expr::Sym { .. } => {}
        Expr::Frame => out.vblank = true,
        Expr::Ly => out.scanline = true,
        Expr::Reg(_) | Expr::Flag(_) | Expr::Ime | Expr::Bank | Expr::T | Expr::Old | Expr::New => {
            out.every_instruction = true
        }
        Expr::Unary(_, inner) => walk(inner, cart, out),
        Expr::Binary(_, l, r) => {
            walk(l, cart, out);
            walk(r, cart, out);
        }
        Expr::Read { width, place } => read(*width, place, cart, out),
    }
}

fn read(width: Width, place: &Place, cart: Cart, out: &mut Triggers) {
    let len = match width {
        Width::U8 | Width::S8 => 1,
        Width::U16 | Width::S16 => 2,
    };
    for i in 0..len {
        match target(place, i, cart.sizes) {
            Some(Ok(target)) => arm(target, cart.ram, out),
            // A read the evaluator can't resolve fails whenever it runs; the
            // baseline at least evaluates it at the first instruction.
            Some(Err(())) | None => out.every_instruction = true,
        }
    }
}

/// Byte `i` of a read, resolved as the evaluator resolves it (`eval::read`);
/// `None` when that depends on the machine (a dynamic address, or a CPU
/// address in a switchable area, which a bank switch remaps).
fn target(place: &Place, i: usize, sizes: CartSizes) -> Option<Result<Target, ()>> {
    match place {
        Place::Cpu(e) => {
            let base = u16::try_from(const_value(e)?).ok()?;
            Some(mem::fixed_target(base.wrapping_add(i as u16))?.map_err(drop))
        }
        Place::Banked { bank, addr } => Some(
            mem::banked_target(*bank, addr.wrapping_add(i as u16))
                .map(|t| sizes.alias(t))
                .map_err(drop),
        ),
        Place::Region { region, offset } => {
            let offset = usize::try_from(const_value(offset)?).ok()?;
            Some(Ok(Target::Backing {
                region: *region,
                offset: offset + i,
            }))
        }
    }
}

/// The trigger for one byte the evaluator reads.
fn arm(target: Target, cart_ram: Support, out: &mut Triggers) {
    match target {
        // Nothing a run does changes ROM, the boot ROM included.
        Target::Backing {
            region: Region::Rom | Region::BootRom,
            ..
        } => {}
        Target::Backing {
            region: region @ (Region::Wram | Region::Hram | Region::Ie),
            offset,
        } => out.stores.insert(region, offset),
        Target::Backing {
            region: Region::Sram,
            offset,
        } if cart_ram == Support::Yes => out.stores.insert(Region::Sram, offset),
        // Cart RAM on a mapper the hook doesn't mirror; VRAM and OAM, which
        // DMA writes without the hook; palette RAM and IO, which change
        // through registers and internal writes.
        _ => out.every_instruction = true,
    }
}

/// The value of an expression that doesn't depend on the machine; `None` for
/// anything dynamic (so the safe per-instruction baseline is taken).
fn const_value(e: &Expr) -> Option<i64> {
    match e {
        Expr::Num(n) => Some(*n),
        Expr::Sym { addr, .. } => Some(i64::from(*addr)),
        Expr::Unary(op, inner) => {
            let v = const_value(inner)?;
            Some(match op {
                UnOp::Neg => v.wrapping_neg(),
                UnOp::Not => !v,
                UnOp::LogicalNot => i64::from(v == 0),
            })
        }
        Expr::Binary(op, l, r) => {
            let a = const_value(l)?;
            let b = const_value(r)?;
            Some(match op {
                BinOp::Add => a.wrapping_add(b),
                BinOp::Sub => a.wrapping_sub(b),
                BinOp::Mul => a.wrapping_mul(b),
                BinOp::Div if b != 0 => a.wrapping_div(b),
                BinOp::Rem if b != 0 => a.wrapping_rem(b),
                BinOp::BitAnd => a & b,
                BinOp::BitOr => a | b,
                BinOp::BitXor => a ^ b,
                BinOp::Shl if (0..64).contains(&b) => a.wrapping_shl(b as u32),
                BinOp::Shr if (0..64).contains(&b) => a.wrapping_shr(b as u32),
                _ => return None,
            })
        }
        _ => None,
    }
}
