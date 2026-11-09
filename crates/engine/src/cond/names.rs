//! Names an expression knows without a `.sym`: registers, flags and machine
//! values. Hardware register names come from `inspect::hwreg::address_of`,
//! which the address grammar shares.

use super::{Expr, Flag, Reg, Width};

/// A keyword's meaning, if `name` is one.
pub fn keyword(name: &str) -> Option<Expr> {
    Some(match name {
        "a" => Expr::Reg(Reg::A),
        "f" => Expr::Reg(Reg::F),
        "b" => Expr::Reg(Reg::B),
        "c" => Expr::Reg(Reg::C),
        "d" => Expr::Reg(Reg::D),
        "e" => Expr::Reg(Reg::E),
        "h" => Expr::Reg(Reg::H),
        "l" => Expr::Reg(Reg::L),
        "af" => Expr::Reg(Reg::Af),
        "bc" => Expr::Reg(Reg::Bc),
        "de" => Expr::Reg(Reg::De),
        "hl" => Expr::Reg(Reg::Hl),
        "sp" => Expr::Reg(Reg::Sp),
        "pc" => Expr::Reg(Reg::Pc),
        "zf" => Expr::Flag(Flag::Z),
        "nf" => Expr::Flag(Flag::N),
        "hf" => Expr::Flag(Flag::H),
        "cf" => Expr::Flag(Flag::C),
        "ime" => Expr::Ime,
        "bank" => Expr::Bank,
        "frame" => Expr::Frame,
        "t" => Expr::T,
        "ly" => Expr::Ly,
        "old" => Expr::Old,
        "new" => Expr::New,
        _ => return None,
    })
}

pub fn width(name: &str) -> Option<Width> {
    Some(match name {
        "u8" => Width::U8,
        "u16" => Width::U16,
        "s8" => Width::S8,
        "s16" => Width::S16,
        _ => return None,
    })
}

/// C's spelling of a signed width (`i8`, `i16`), as this grammar spells it.
pub fn c_signed_width(name: &str) -> Option<&'static str> {
    match name {
        "i8" => Some("s8"),
        "i16" => Some("s16"),
        _ => None,
    }
}
