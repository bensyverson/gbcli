//! Deriving a predicate's evaluation cadence from what it reads (design §8,
//! §11), and the addresses to arm with the write hook when only writes can
//! change it.
//!
//! The cadence is the finest trigger among everything the expression reads:
//! `EveryInstruction` > `Scanline` > `Vblank` > `Writes`. A static read the
//! write hook covers arms a candidate on each byte it takes; anything the
//! hook misses (or a value that changes without a write at the address) needs
//! the finer baseline.

use gbcli_core::{BankSel, Region, canonical_addr};
use gbcli_protocol::Cadence;

use super::{BinOp, Expr, Place, UnOp, Width};
use crate::sym::Area;

/// A predicate's cadence and, when it is `Writes`, the addresses to arm.
pub(crate) struct Analysis {
    pub cadence: Cadence,
    pub writes: Vec<(u16, BankSel)>,
}

/// The cadence, and the addresses to arm for a `writes` cadence.
pub(crate) fn analyze(expr: &Expr) -> Analysis {
    let mut analysis = Analysis {
        cadence: Cadence::Writes,
        writes: Vec::new(),
    };
    walk(expr, &mut analysis);
    analysis
}

impl Analysis {
    fn join(&mut self, cadence: Cadence) {
        self.cadence = self.cadence.max(cadence);
    }
}

fn walk(e: &Expr, out: &mut Analysis) {
    match e {
        Expr::Num(_) | Expr::Sym { .. } => {}
        Expr::Frame => out.join(Cadence::Vblank),
        Expr::Ly => out.join(Cadence::Scanline),
        Expr::Reg(_) | Expr::Flag(_) | Expr::Ime | Expr::Bank | Expr::T | Expr::Old | Expr::New => {
            out.join(Cadence::EveryInstruction)
        }
        Expr::Unary(_, inner) => walk(inner, out),
        Expr::Binary(_, l, r) => {
            walk(l, out);
            walk(r, out);
        }
        Expr::Read { width, place } => read(*width, place, out),
    }
}

fn read(width: Width, place: &Place, out: &mut Analysis) {
    let len = match width {
        Width::U8 | Width::S8 => 1,
        Width::U16 | Width::S16 => 2,
    };
    match place {
        Place::Cpu(e) => {
            let Some(base) = const_value(e).and_then(|v| u16::try_from(v).ok()) else {
                out.join(Cadence::EveryInstruction);
                return;
            };
            for i in 0..len {
                match cpu_candidate(base.wrapping_add(i as u16)) {
                    Some(sel) => out
                        .writes
                        .push((canonical_addr(base.wrapping_add(i as u16)), sel)),
                    None => out.join(Cadence::EveryInstruction),
                }
            }
        }
        Place::Banked { bank, addr } => {
            for i in 0..len {
                let addr = canonical_addr(addr.wrapping_add(i as u16));
                match banked_candidate(addr, *bank) {
                    Some(sel) => out.writes.push((addr, sel)),
                    None => out.join(Cadence::EveryInstruction),
                }
            }
        }
        Place::Region { region, offset } => {
            let Some(base) = const_value(offset).and_then(|v| usize::try_from(v).ok()) else {
                out.join(Cadence::EveryInstruction);
                return;
            };
            for i in 0..len {
                match region_candidate(*region, base + i) {
                    Some((addr, sel)) => out.writes.push((addr, sel)),
                    None => out.join(Cadence::EveryInstruction),
                }
            }
        }
    }
}

/// A CPU address read in the mapping of the moment. `None` when the value can
/// change without a write to the address: a bank switch changes a switchable
/// area's byte, an internal write (serial, APU, PPU) is invisible to the write
/// hook, and DMA can change VRAM or OAM.
fn cpu_candidate(addr: u16) -> Option<BankSel> {
    match Area::of(canonical_addr(addr)) {
        Area::Rom0 | Area::Wram0 | Area::Hram | Area::Ie => Some(BankSel::Any),
        _ => None,
    }
}

/// A read of a named bank: a bank switch cannot change it, so only the write
/// hook can. VRAM and OAM still fall back (DMA), and IO changes internally
/// often enough that auditing every register is not worth the speed.
fn banked_candidate(addr: u16, bank: u16) -> Option<BankSel> {
    match Area::of(canonical_addr(addr)) {
        Area::Rom0 | Area::Wram0 | Area::Hram | Area::Ie => Some(BankSel::Any),
        Area::RomX | Area::Sram | Area::WramX => Some(BankSel::Bank(bank)),
        _ => None,
    }
}

/// A region offset, mapped to the CPU address(es) it is readable at.
fn region_candidate(region: Region, offset: usize) -> Option<(u16, BankSel)> {
    let any = |addr: u16| Some((addr, BankSel::Any));
    match region {
        Region::Wram if offset < 0x1000 => any(0xC000 + offset as u16),
        Region::Wram if offset < 0x2000 => {
            Some((0xD000 + (offset - 0x1000) as u16, BankSel::Bank(1)))
        }
        Region::Wram => Some((
            0xD000 + (offset % 0x1000) as u16,
            BankSel::Bank((offset / 0x1000) as u16),
        )),
        Region::Hram if offset < 0x7F => any(0xFF80 + offset as u16),
        Region::Ie if offset == 0 => any(0xFFFF),
        Region::Sram => Some((
            0xA000 + (offset % 0x2000) as u16,
            BankSel::Bank((offset / 0x2000) as u16),
        )),
        Region::Rom if offset < 0x4000 => any(offset as u16),
        Region::Rom => Some((
            0x4000 + (offset % 0x4000) as u16,
            BankSel::Bank((offset / 0x4000) as u16),
        )),
        _ => None,
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
