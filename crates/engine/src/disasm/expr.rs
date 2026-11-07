//! rgbasm's numeric expressions, for the operands `lookup` reads (data, an
//! address, an RST vector, a bit number), as rgbasm 1.0.4 reads them
//! (rgbasm(5), Numeric expressions): numbers in every base with `_`
//! between digits; character constants and one-character strings in the
//! default charmap (ASCII), with rgbasm's escapes; parentheses; `HIGH()`
//! and `LOW()`; and every operator with rgbasm's precedence and 32-bit
//! wrapping arithmetic. Read but not valued ([`Value::Unknown`]): symbols
//! (a label, `@`, an anonymous label), rgbasm's other functions that return
//! a number, fixed-point numbers and graphics constants. Refused: a
//! register or condition name inside an expression, interpolation (`{…}`),
//! string expressions other than a string literal as a function's argument,
//! and nesting deeper than [`MAX_DEPTH`].

mod literal;

/// How deep parentheses, unary operators, exponents and functions may
/// nest. The parser recurses once for each level, so a line nested
/// thousands deep (rgbasm itself takes 10,000) would overflow the stack;
/// past this it is refused instead.
pub(super) const MAX_DEPTH: usize = 64;

/// Why a text has no value here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Refused {
    /// Not an expression this reads, or one rgbasm refuses (division by
    /// zero, a negative power).
    Invalid,
    /// Nested deeper than [`MAX_DEPTH`].
    TooDeep,
}

/// What an expression rgbasm takes is worth here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Value {
    Known(i32),
    /// It names a symbol, or a function or literal whose value isn't
    /// worked out here: rgbasm takes it, but its value is unknown.
    Unknown,
}

use Value::{Known, Unknown};

/// rgbasm's register and condition names, which can't be symbols: an
/// expression naming one is refused (`hl+1`, `a+1`).
const REGISTERS: &[&str] = &[
    "a", "b", "c", "d", "e", "h", "l", "af", "bc", "de", "hl", "hli", "hld", "sp", "nz", "z", "nc",
];

/// rgbasm 1.0.4's functions that return a number (rgbasm(5)), besides
/// `HIGH()` and `LOW()`. Their arguments are read as expressions or string
/// literals; what kind and how many each takes is not checked.
const FUNCTIONS: &[&str] = &[
    "bitwidth",
    "tzcount",
    "div",
    "mul",
    "fmod",
    "pow",
    "log",
    "round",
    "ceil",
    "floor",
    "sin",
    "cos",
    "tan",
    "asin",
    "acos",
    "atan",
    "atan2",
    "strlen",
    "strcmp",
    "strfind",
    "strrfind",
    "bytelen",
    "strbyte",
    "incharmap",
    "charlen",
    "charcmp",
    "charsize",
    "charval",
    "def",
    "isconst",
    "bank",
    "sizeof",
    "startof",
];

/// The value of `text`, an operand as written (whitespace between tokens is
/// allowed, and separates operator characters as it does in rgbasm: `& &`
/// is not `&&`).
pub(super) fn eval(text: &str) -> Result<Value, Refused> {
    let mut p = Parser {
        s: text.as_bytes(),
        i: 0,
        depth: 0,
        too_deep: false,
    };
    let v = p.or();
    p.skip_space();
    match v {
        _ if p.too_deep => Err(Refused::TooDeep),
        Some(v) if p.i == p.s.len() => Ok(v),
        _ => Err(Refused::Invalid),
    }
}

/// `f` of two values when both are known; unknown when either is.
/// (`&&`, `||` and `&` fold first: see [`folded`].)
fn both(l: Value, r: Value, f: impl FnOnce(i32, i32) -> i32) -> Value {
    match (l, r) {
        (Known(l), Known(r)) => Known(f(l, r)),
        _ => Unknown,
    }
}

/// What rgbasm makes constant whatever the other side: `x && 0`, `x || 5`
/// and `x & 0`, either way round.
fn folded(op: &str, l: Value, r: Value) -> Option<Value> {
    let absorbs = |v: Value| match (op, v) {
        ("&&" | "&", Known(0)) => Some(Known(0)),
        ("||", Known(v)) if v != 0 => Some(Known(1)),
        _ => None,
    };
    absorbs(l).or_else(|| absorbs(r))
}

fn truth(b: bool) -> i32 {
    i32::from(b)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    depth: usize,
    too_deep: bool,
}

impl Parser<'_> {
    fn skip_space(&mut self) {
        while self.s.get(self.i).is_some_and(u8::is_ascii_whitespace) {
            self.i += 1;
        }
    }

    fn rest(&self) -> &[u8] {
        &self.s[self.i..]
    }

    fn eat(&mut self, op: &str) -> bool {
        self.skip_space();
        if self.rest().starts_with(op.as_bytes()) {
            self.i += op.len();
            true
        } else {
            false
        }
    }

    /// The first of `ops` here, longest first, unless it is the start of
    /// one of `not` (so `&` isn't read from `&&`) or of a number: rgbasm's
    /// lexer reads `&` before an octal digit or `_`, and `%` before 0, 1 or
    /// `_`, as a number's prefix, so `15&7` is two numbers.
    fn op(&mut self, ops: &[&'static str], not: &[&str]) -> Option<&'static str> {
        self.skip_space();
        let rest = self.rest();
        let number = matches!(
            rest,
            [b'&', b'0'..=b'7' | b'_', ..] | [b'%', b'0' | b'1' | b'_', ..]
        );
        if number || not.iter().any(|n| rest.starts_with(n.as_bytes())) {
            return None;
        }
        ops.iter().copied().find(|op| self.eat(op))
    }

    /// `f` one level deeper, or nothing past [`MAX_DEPTH`].
    fn nested(&mut self, f: impl FnOnce(&mut Self) -> Option<Value>) -> Option<Value> {
        if self.depth >= MAX_DEPTH {
            self.too_deep = true;
            return None;
        }
        self.depth += 1;
        let v = f(self);
        self.depth -= 1;
        v
    }

    fn or(&mut self) -> Option<Value> {
        let mut v = self.and()?;
        while self.eat("||") {
            let r = self.and()?;
            v = folded("||", v, r).unwrap_or_else(|| both(v, r, |l, r| truth(l != 0 || r != 0)));
        }
        Some(v)
    }

    fn and(&mut self) -> Option<Value> {
        let mut v = self.compare()?;
        while self.eat("&&") {
            let r = self.compare()?;
            v = folded("&&", v, r).unwrap_or_else(|| both(v, r, |l, r| truth(l != 0 && r != 0)));
        }
        Some(v)
    }

    fn compare(&mut self) -> Option<Value> {
        let mut v = self.sum()?;
        while let Some(op) = self.op(&["==", "!=", "<=", ">=", "<", ">"], &["<<", ">>"]) {
            let r = self.sum()?;
            v = both(v, r, |l, r| {
                truth(match op {
                    "==" => l == r,
                    "!=" => l != r,
                    "<=" => l <= r,
                    ">=" => l >= r,
                    "<" => l < r,
                    _ => l > r,
                })
            });
        }
        Some(v)
    }

    fn sum(&mut self) -> Option<Value> {
        let mut v = self.bits()?;
        // `++` is rgbasm's string concatenation, never two `+`s.
        while let Some(op) = self.op(&["+", "-"], &["++"]) {
            let r = self.bits()?;
            v = both(v, r, |l, r| {
                if op == "+" {
                    l.wrapping_add(r)
                } else {
                    l.wrapping_sub(r)
                }
            });
        }
        Some(v)
    }

    fn bits(&mut self) -> Option<Value> {
        let mut v = self.shift()?;
        while let Some(op) = self.op(&["&", "|", "^"], &["&&", "||"]) {
            let r = self.shift()?;
            if let Some(f) = folded(op, v, r) {
                v = f;
                continue;
            }
            v = both(v, r, |l, r| match op {
                "&" => l & r,
                "|" => l | r,
                _ => l ^ r,
            });
        }
        Some(v)
    }

    fn shift(&mut self) -> Option<Value> {
        let mut v = self.product()?;
        while let Some(op) = self.op(&[">>>", "<<", ">>"], &[]) {
            let r = self.product()?;
            v = both(v, r, |v, r| {
                // A negative amount shifts the other way (rgbasm warns), and
                // 32 or more shifts every bit out.
                let by = r.unsigned_abs();
                let left = (op == "<<") == (r >= 0);
                if left {
                    v.checked_shl(by).unwrap_or(0)
                } else if op == ">>>" {
                    (v as u32).checked_shr(by).unwrap_or(0) as i32
                } else {
                    v.checked_shr(by).unwrap_or(if v < 0 { -1 } else { 0 })
                }
            });
        }
        Some(v)
    }

    fn product(&mut self) -> Option<Value> {
        let mut v = self.unary()?;
        while let Some(op) = self.op(&["*", "/", "%"], &["**"]) {
            let r = self.unary()?;
            // By zero is refused, whatever is divided.
            if op != "*" && r == Known(0) {
                return None;
            }
            v = both(v, r, |v, r| {
                if op == "*" {
                    return v.wrapping_mul(r);
                }
                // Division rounds down; the remainder takes the divisor's sign.
                let rem = v.wrapping_rem(r);
                let floored = rem != 0 && (rem < 0) != (r < 0);
                if op == "/" {
                    v.wrapping_div(r) - i32::from(floored)
                } else {
                    rem + if floored { r } else { 0 }
                }
            });
        }
        Some(v)
    }

    /// A unary operator binds looser than `**`: `-2 ** 2` is -4.
    fn unary(&mut self) -> Option<Value> {
        if let Some(op) = self.op(&["+", "-", "~", "!"], &["!=", "++"]) {
            let v = self.nested(Self::unary)?;
            return Some(match (op, v) {
                (_, Unknown) => Unknown,
                ("+", Known(v)) => Known(v),
                ("-", Known(v)) => Known(v.wrapping_neg()),
                ("~", Known(v)) => Known(!v),
                (_, Known(v)) => Known(truth(v == 0)),
            });
        }
        self.power()
    }

    /// `**` binds to the right, and its exponent is any unary expression
    /// (`8 ** !0`, `2 ** -1 ** 2`); a negative one is refused.
    fn power(&mut self) -> Option<Value> {
        let base = self.primary()?;
        if !self.eat("**") {
            return Some(base);
        }
        let exp = self.nested(Self::unary)?;
        match (base, exp) {
            (_, Known(e)) if e < 0 => None,
            (Known(b), Known(e)) => Some(Known(b.wrapping_pow(e.unsigned_abs()))),
            _ => Some(Unknown),
        }
    }

    fn primary(&mut self) -> Option<Value> {
        self.skip_space();
        let s = self.s;
        let rest = &s[self.i..];
        let (v, len) = match *rest.first()? {
            b'(' => {
                self.i += 1;
                let v = self.nested(Self::or)?;
                return self.eat(")").then_some(v);
            }
            b'\'' | b'"' => literal::character(rest)?,
            b'`' => literal::graphics(rest)?,
            // An anonymous label: `:+`, `:--`.
            b':' => {
                let run = |sign: u8| rest[1..].iter().take_while(|&&c| c == sign).count();
                let len = 1 + run(b'+').max(run(b'-'));
                (len > 1).then_some((Unknown, len))?
            }
            c if c.is_ascii_alphabetic() || b"_.@#".contains(&c) => return self.name(),
            _ => literal::number(rest)?,
        };
        self.i += len;
        Some(v)
    }

    /// A symbol, or a function and its arguments.
    fn name(&mut self) -> Option<Value> {
        let s = self.s;
        let rest = &s[self.i..];
        let len = rest
            .iter()
            .take_while(|c| c.is_ascii_alphanumeric() || b"_.@#$".contains(c))
            .count();
        let word = &rest[..len];
        self.i += len;
        // `#name` is a symbol even where `name` is a keyword.
        if word[0] == b'#' {
            return (len > 1).then_some(Unknown);
        }
        let name = std::str::from_utf8(word).ok()?.to_ascii_lowercase();
        // `.` and `..` are the current label scopes; `Top.` is nothing.
        if matches!(word, b"." | b"..") {
            return Some(Unknown);
        }
        if word.ends_with(b".") || REGISTERS.contains(&name.as_str()) {
            return None;
        }
        let shift = match name.as_str() {
            "high" => 8,
            "low" => 0,
            _ if FUNCTIONS.contains(&name.as_str()) => return self.arguments(),
            _ => return Some(Unknown),
        };
        if !self.eat("(") {
            return None;
        }
        let v = self.nested(Self::or)?;
        if !self.eat(")") {
            return None;
        }
        Some(match v {
            Known(v) => Known((v >> shift) & 0xFF),
            Unknown => Unknown,
        })
    }

    /// A function's parenthesized arguments, at least one, each an
    /// expression or a string literal: the call's value is unknown.
    fn arguments(&mut self) -> Option<Value> {
        if !self.eat("(") {
            return None;
        }
        loop {
            self.skip_space();
            let string = (self.rest().first() == Some(&b'"'))
                .then(|| literal::string_len(self.rest()))
                .flatten();
            let after = string.map(|len| {
                let tail = &self.rest()[len..];
                let pad = tail.iter().take_while(|c| c.is_ascii_whitespace()).count();
                tail.get(pad).copied()
            });
            match (string, after) {
                // A string of any length, when nothing but `,` or `)` follows.
                (Some(len), Some(Some(b',' | b')'))) => self.i += len,
                _ => {
                    self.nested(Self::or)?;
                }
            }
            if self.eat(")") {
                return Some(Unknown);
            }
            if !self.eat(",") {
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests;
