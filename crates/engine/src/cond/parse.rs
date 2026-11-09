//! A precedence-climbing parser with C precedence.

use super::lex::{Prefix, Tok, lex};
use super::{BinOp, Expr, ExprError, Place, UnOp, Width};
use super::{names, oam};
use crate::addr::banked_area;
use crate::inspect::hwreg;
use crate::inspect::regions::parse_region;
use crate::sym::{Area, Symbols};

pub fn parse(text: &str, syms: Option<&Symbols>) -> Result<Expr, ExprError> {
    let mut p = Parser {
        toks: lex(text)?,
        pos: 0,
        end: text.len(),
        syms,
    };
    let expr = p.expr(0)?;
    if let Some((at, tok)) = p.toks.get(p.pos) {
        return Err(ExprError::syntax(
            *at,
            format!("unexpected {tok:?} after a complete expression"),
        ));
    }
    check_bare_ram_symbols(&expr, text)?;
    Ok(expr)
}

/// Parses `text` as what `[text]` would read: a place (design §7).
pub fn parse_place(text: &str, syms: Option<&Symbols>) -> Result<Place, ExprError> {
    let mut p = Parser {
        toks: lex(text)?,
        pos: 0,
        end: text.len(),
        syms,
    };
    let place = p.place()?;
    if let Some((at, tok)) = p.toks.get(p.pos) {
        return Err(ExprError::syntax(
            *at,
            format!("unexpected {tok:?} after a complete address"),
        ));
    }
    Ok(place)
}

struct Parser<'a> {
    toks: Vec<(usize, Tok)>,
    pos: usize,
    end: usize,
    syms: Option<&'a Symbols>,
}

fn binop(op: &str) -> Option<(u8, BinOp)> {
    Some(match op {
        "||" => (1, BinOp::Or),
        "&&" => (2, BinOp::And),
        "|" => (3, BinOp::BitOr),
        "^" => (4, BinOp::BitXor),
        "&" => (5, BinOp::BitAnd),
        "==" => (6, BinOp::Eq),
        "!=" => (6, BinOp::Ne),
        "<" => (7, BinOp::Lt),
        "<=" => (7, BinOp::Le),
        ">" => (7, BinOp::Gt),
        ">=" => (7, BinOp::Ge),
        "<<" => (8, BinOp::Shl),
        ">>" => (8, BinOp::Shr),
        "+" => (9, BinOp::Add),
        "-" => (9, BinOp::Sub),
        "*" => (10, BinOp::Mul),
        "/" => (10, BinOp::Div),
        "%" => (10, BinOp::Rem),
        _ => return None,
    })
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos).map(|(_, t)| t)
    }

    fn at(&self) -> usize {
        self.toks.get(self.pos).map_or(self.end, |(at, _)| *at)
    }

    fn next(&mut self) -> Result<(usize, Tok), ExprError> {
        let tok = self
            .toks
            .get(self.pos)
            .cloned()
            .ok_or_else(|| ExprError::syntax(self.end, "the condition ends too soon"))?;
        self.pos += 1;
        Ok(tok)
    }

    fn expect(&mut self, want: Tok) -> Result<(), ExprError> {
        let (at, tok) = self.next()?;
        if tok == want {
            Ok(())
        } else {
            Err(ExprError::syntax(
                at,
                format!("expected {want:?}, got {tok:?}"),
            ))
        }
    }

    fn expr(&mut self, min: u8) -> Result<Expr, ExprError> {
        let mut lhs = self.unary()?;
        while let Some(Tok::Op(op)) = self.peek() {
            let Some((prec, op)) = binop(op) else { break };
            if prec < min {
                break;
            }
            self.pos += 1;
            let rhs = self.expr(prec + 1)?;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn unary(&mut self) -> Result<Expr, ExprError> {
        let op = match self.peek() {
            Some(Tok::Op("-")) => UnOp::Neg,
            Some(Tok::Op("~")) => UnOp::Not,
            Some(Tok::Op("!")) => UnOp::LogicalNot,
            _ => return self.primary(),
        };
        self.pos += 1;
        Ok(Expr::Unary(op, Box::new(self.unary()?)))
    }

    fn primary(&mut self) -> Result<Expr, ExprError> {
        let (at, tok) = self.next()?;
        match tok {
            Tok::Num { value: Some(v), .. } => Ok(Expr::Num(v)),
            Tok::Num { raw, .. } => Err(ExprError::syntax(
                at,
                format!("{raw:?} isn't decimal; write hex as ${raw} or 0x{raw}"),
            )),
            Tok::LParen => {
                let e = self.expr(0)?;
                self.expect(Tok::RParen)?;
                Ok(e)
            }
            Tok::LBracket => self.read(Width::U8),
            Tok::Ident(name) => {
                if let Some(width) = names::width(&name) {
                    return match self.next()? {
                        (_, Tok::LBracket) => self.read(width),
                        (at, _) => Err(ExprError::syntax(at, format!("expected [ after {name}"))),
                    };
                }
                if let Some(signed) = names::c_signed_width(&name)
                    && self.peek() == Some(&Tok::LBracket)
                {
                    return Err(ExprError::Syntax {
                        at,
                        message: format!("{name} is not a width"),
                        hint: Some(format!(
                            "{signed}[ADDR]? Reads are u8 and u16 (unsigned), s8 and s16 \
                             (signed, sign-extended)"
                        )),
                    });
                }
                if let Some(e) = names::keyword(&name) {
                    return Ok(e);
                }
                if name == oam::NAME && self.peek() == Some(&Tok::LBracket) {
                    return self.oam(at);
                }
                if let Some(addr) = hwreg::address_of(&name) {
                    return Ok(Expr::Num(i64::from(addr)));
                }
                self.symbol(name)
            }
            other => Err(ExprError::syntax(at, format!("unexpected {other:?}"))),
        }
    }

    fn symbol(&self, name: String) -> Result<Expr, ExprError> {
        let Some(syms) = self.syms else {
            return Err(ExprError::NoSymbols { name });
        };
        match syms.named(&name).as_slice() {
            [s] => Ok(Expr::Sym {
                name,
                bank: s.bank,
                addr: s.addr,
            }),
            [] => Err(ExprError::UnknownSymbol {
                near: syms
                    .near_misses(&name)
                    .into_iter()
                    .map(String::from)
                    .collect(),
                name,
            }),
            many => Err(ExprError::AmbiguousSymbol {
                name,
                candidates: many.iter().map(|s| (*s).clone()).collect(),
            }),
        }
    }

    /// After `oam`, at `[`: `[N]` and a field (design §7).
    fn oam(&mut self, at: usize) -> Result<Expr, ExprError> {
        self.expect(Tok::LBracket)?;
        let (index_at, index) = self.next()?;
        self.expect(Tok::RBracket)?;
        let field = match self.toks.get(self.pos) {
            Some((field_at, Tok::Ident(f))) if f.starts_with('.') => {
                let field = (*field_at, f.clone());
                self.pos += 1;
                Some(field)
            }
            _ => None,
        };
        let shown = field.as_ref().map_or("", |(_, f)| f.as_str());
        let index = oam::index(index_at, &index, shown)?;
        match field {
            Some((field_at, f)) => oam::field(field_at, index, &f),
            None => Err(oam::whole_entry(at, index)),
        }
    }

    /// After `[`: a place, then `]`.
    fn read(&mut self, width: Width) -> Result<Expr, ExprError> {
        let place = self.place()?;
        self.expect(Tok::RBracket)?;
        Ok(Expr::Read { width, place })
    }

    /// What a read's brackets hold: `bank:addr`, `region:offset`, or an
    /// expression (a banked symbol, alone or moved, reading its own bank).
    fn place(&mut self) -> Result<Place, ExprError> {
        let colon_next = matches!(self.toks.get(self.pos + 1), Some((_, Tok::Colon)));
        let place = match (self.peek().cloned(), colon_next) {
            // A head that is hex is a bank, as in the address grammar, even
            // one that lexes as a name (`ff:4000`); any other names a region.
            (Some(Tok::Ident(name)), true) if !name.bytes().all(|b| b.is_ascii_hexdigit()) => {
                let at = self.at();
                let region = parse_region(&name).ok_or_else(|| {
                    ExprError::syntax(
                        at,
                        format!(
                            "{name:?} isn't a region (wram, sram, vram, hram, oam, io, rom, ...)"
                        ),
                    )
                })?;
                self.pos += 2;
                let (offset_at, offset) = self.next()?;
                Place::Region {
                    region,
                    offset: region_offset(offset_at, &offset)?,
                }
            }
            (Some(bank @ (Tok::Num { .. } | Tok::Ident(_))), true) => {
                let bank_at = self.at();
                self.pos += 2;
                let (addr_at, addr) = self.next()?;
                Place::Banked {
                    bank: hex(bank_at, &bank)? as u16,
                    addr: hex(addr_at, &addr)? as u16,
                }
            }
            _ => {
                let e = self.expr(0)?;
                match banked_symbol(&e) {
                    Some((bank, addr)) => Place::Banked { bank, addr },
                    None => Place::Cpu(Box::new(e)),
                }
            }
        };
        Ok(place)
    }
}

/// A bank-qualified part (`03`, `$4123`, `A000`): hex, with or without a
/// prefix. Unprefixed hex that starts with a letter lexes as a name.
fn hex(at: usize, tok: &Tok) -> Result<i64, ExprError> {
    hex_value(tok)
        .filter(|v| (0..=0xFFFF).contains(v))
        .ok_or_else(|| {
            ExprError::syntax(at, format!("expected a hex bank or address, got {tok:?}"))
        })
}

/// A region offset (`sram:10`, `wram:$1A0`): one hex number, prefixed or
/// not, as the address grammar reads it (design §7). Bounds against the
/// region's size are checked when it is read.
fn region_offset(at: usize, tok: &Tok) -> Result<u32, ExprError> {
    hex_value(tok)
        .and_then(|v| u32::try_from(v).ok())
        .ok_or_else(|| {
            ExprError::syntax(
                at,
                format!("expected a hex region offset (sram:10 is offset $10), got {tok:?}"),
            )
        })
}

/// Hex with `$` or `0x`, or bare; not `%` binary, which the address
/// grammar refuses in these parts.
fn hex_value(tok: &Tok) -> Option<i64> {
    match tok {
        Tok::Num {
            value,
            prefix: Prefix::Hex,
            ..
        } => *value,
        Tok::Num {
            raw,
            prefix: Prefix::None,
            ..
        } => i64::from_str_radix(raw, 16).ok(),
        Tok::Num {
            prefix: Prefix::Binary,
            ..
        } => None,
        Tok::Ident(raw) if raw.bytes().all(|b| b.is_ascii_hexdigit()) => {
            i64::from_str_radix(raw, 16).ok()
        }
        _ => None,
    }
}

/// A symbol (plus or minus a constant) in a switchable area reads in its own
/// bank rather than whatever is mapped.
fn banked_symbol(e: &Expr) -> Option<(u16, u16)> {
    let (bank, addr) = match e {
        Expr::Sym { bank, addr, .. } => (*bank, i64::from(*addr)),
        Expr::Binary(op @ (super::BinOp::Add | super::BinOp::Sub), l, r) => match (&**l, &**r) {
            (Expr::Sym { bank, addr, .. }, Expr::Num(n)) => {
                let n = if *op == super::BinOp::Add { *n } else { -n };
                (*bank, i64::from(*addr) + n)
            }
            (Expr::Num(n), Expr::Sym { bank, addr, .. }) if *op == super::BinOp::Add => {
                (*bank, i64::from(*addr) + n)
            }
            _ => return None,
        },
        _ => return None,
    };
    let addr = u16::try_from(addr).ok()?;
    banked_area(Area::of(addr)).map(|_| (bank, addr))
}

/// `wFrame >= 60` compares an address: refuse it and suggest the reads.
fn check_bare_ram_symbols(e: &Expr, text: &str) -> Result<(), ExprError> {
    match e {
        Expr::Binary(op, l, r) => {
            let comparison = matches!(
                op,
                BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::Eq | BinOp::Ne
            );
            if comparison {
                for side in [l, r] {
                    refuse_ram_symbol(side, text)?;
                }
            }
            check_bare_ram_symbols(l, text)?;
            check_bare_ram_symbols(r, text)
        }
        Expr::Unary(_, inner) => check_bare_ram_symbols(inner, text),
        _ => Ok(()),
    }
}

/// A RAM symbol read as true or false always holds, being its address:
/// refuse one as the whole condition or an operand of `&&`, `||` or `!`.
pub fn check_truth_positions(e: &Expr, text: &str) -> Result<(), ExprError> {
    match e {
        Expr::Binary(BinOp::And | BinOp::Or, l, r) => {
            check_truth_positions(l, text)?;
            check_truth_positions(r, text)
        }
        Expr::Unary(UnOp::LogicalNot, inner) => check_truth_positions(inner, text),
        _ => refuse_ram_symbol(e, text),
    }
}

/// Refuses `e` when it is a bare RAM symbol, suggesting the reads.
fn refuse_ram_symbol(e: &Expr, text: &str) -> Result<(), ExprError> {
    let Expr::Sym { name, addr, .. } = e else {
        return Ok(());
    };
    if !Area::of(*addr).is_ram() {
        return Ok(());
    }
    let at = bare_occurrence(text, name).unwrap_or(0);
    let read = |prefix: &str| {
        if text[at..].starts_with(name.as_str()) {
            format!(
                "{}{prefix}[{name}]{}",
                &text[..at],
                &text[at + name.len()..]
            )
        } else {
            format!("{prefix}[{name}]")
        }
    };
    Err(ExprError::Syntax {
        at,
        message: format!("{name} is an address, not the value stored there"),
        hint: Some(format!(
            "{} or {}? A bare symbol is its address",
            read(""),
            read("u16")
        )),
    })
}

/// Where `name` appears as a whole word outside any `[...]`, so the hint
/// rewrites that occurrence and not one already read.
fn bare_occurrence(text: &str, name: &str) -> Option<usize> {
    let word = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '#' | '@');
    let mut depth = 0i32;
    for (i, c) in text.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => depth -= 1,
            _ => {}
        }
        if depth == 0
            && text[i..].starts_with(name)
            && !text[..i].chars().next_back().is_some_and(word)
            && !text[i + name.len()..].chars().next().is_some_and(word)
        {
            return Some(i);
        }
    }
    None
}
