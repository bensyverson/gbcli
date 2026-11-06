//! Names an expression knows without a `.sym`: registers, flags, machine
//! values and the hardware registers by their hardware.inc names.

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

/// Hardware register addresses by their hardware.inc names (pandocs names).
const HARDWARE: &[(&str, u16)] = &[
    ("rP1", 0xFF00),
    ("rJOYP", 0xFF00),
    ("rSB", 0xFF01),
    ("rSC", 0xFF02),
    ("rDIV", 0xFF04),
    ("rTIMA", 0xFF05),
    ("rTMA", 0xFF06),
    ("rTAC", 0xFF07),
    ("rIF", 0xFF0F),
    ("rNR10", 0xFF10),
    ("rNR11", 0xFF11),
    ("rNR12", 0xFF12),
    ("rNR13", 0xFF13),
    ("rNR14", 0xFF14),
    ("rNR21", 0xFF16),
    ("rNR22", 0xFF17),
    ("rNR23", 0xFF18),
    ("rNR24", 0xFF19),
    ("rNR30", 0xFF1A),
    ("rNR31", 0xFF1B),
    ("rNR32", 0xFF1C),
    ("rNR33", 0xFF1D),
    ("rNR34", 0xFF1E),
    ("rNR41", 0xFF20),
    ("rNR42", 0xFF21),
    ("rNR43", 0xFF22),
    ("rNR44", 0xFF23),
    ("rNR50", 0xFF24),
    ("rNR51", 0xFF25),
    ("rNR52", 0xFF26),
    ("rLCDC", 0xFF40),
    ("rSTAT", 0xFF41),
    ("rSCY", 0xFF42),
    ("rSCX", 0xFF43),
    ("rLY", 0xFF44),
    ("rLYC", 0xFF45),
    ("rDMA", 0xFF46),
    ("rBGP", 0xFF47),
    ("rOBP0", 0xFF48),
    ("rOBP1", 0xFF49),
    ("rWY", 0xFF4A),
    ("rWX", 0xFF4B),
    ("rKEY1", 0xFF4D),
    ("rSPD", 0xFF4D),
    ("rVBK", 0xFF4F),
    ("rHDMA1", 0xFF51),
    ("rHDMA2", 0xFF52),
    ("rHDMA3", 0xFF53),
    ("rHDMA4", 0xFF54),
    ("rHDMA5", 0xFF55),
    ("rRP", 0xFF56),
    ("rBCPS", 0xFF68),
    ("rBGPI", 0xFF68),
    ("rBCPD", 0xFF69),
    ("rBGPD", 0xFF69),
    ("rOCPS", 0xFF6A),
    ("rOBPI", 0xFF6A),
    ("rOCPD", 0xFF6B),
    ("rOBPD", 0xFF6B),
    ("rSVBK", 0xFF70),
    ("rWBK", 0xFF70),
    ("rIE", 0xFFFF),
];

pub fn hardware(name: &str) -> Option<u16> {
    HARDWARE.iter().find(|(n, _)| *n == name).map(|&(_, a)| a)
}
