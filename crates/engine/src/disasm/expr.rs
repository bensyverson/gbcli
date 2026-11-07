//! rgbasm's constant numeric expressions, for the operands `lookup` needs a
//! value of (an RST vector, a bit number), as rgbasm 1.0.4 reads them
//! (rgbasm(5), Numeric expressions): numbers in every base with `_`
//! between digits; character constants and one-character strings in the
//! default charmap (ASCII), with rgbasm's escapes; parentheses; `HIGH()`
//! and `LOW()`; and every operator with rgbasm's precedence and 32-bit
//! wrapping arithmetic. Not read, so refused: symbols, every other
//! function, graphics constants (`` `0123 ``), fixed-point numbers, and
//! nesting deeper than [`MAX_DEPTH`].

/// How deep parentheses, unary operators, exponents and functions may
/// nest. The parser recurses once for each level, so a line nested
/// thousands deep (rgbasm itself takes 10,000) would overflow the stack;
/// past this it is refused instead.
pub(super) const MAX_DEPTH: usize = 64;

/// Why a text has no value here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Refused {
    /// Not a constant expression this reads, or one rgbasm refuses
    /// (division by zero, a negative power).
    Invalid,
    /// Nested deeper than [`MAX_DEPTH`].
    TooDeep,
}

/// The value of `text`, an operand as written (whitespace between tokens is
/// allowed, and separates operator characters as it does in rgbasm: `& &`
/// is not `&&`).
pub(super) fn eval(text: &str) -> Result<i32, Refused> {
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
    /// one of `not` (so `&` isn't read from `&&`).
    fn op(&mut self, ops: &[&'static str], not: &[&str]) -> Option<&'static str> {
        self.skip_space();
        if not.iter().any(|n| self.rest().starts_with(n.as_bytes())) {
            return None;
        }
        ops.iter().copied().find(|op| self.eat(op))
    }

    /// `f` one level deeper, or nothing past [`MAX_DEPTH`].
    fn nested(&mut self, f: impl FnOnce(&mut Self) -> Option<i32>) -> Option<i32> {
        if self.depth >= MAX_DEPTH {
            self.too_deep = true;
            return None;
        }
        self.depth += 1;
        let v = f(self);
        self.depth -= 1;
        v
    }

    fn or(&mut self) -> Option<i32> {
        let mut v = self.and()?;
        while self.eat("||") {
            let r = self.and()?;
            v = i32::from(v != 0 || r != 0);
        }
        Some(v)
    }

    fn and(&mut self) -> Option<i32> {
        let mut v = self.compare()?;
        while self.eat("&&") {
            let r = self.compare()?;
            v = i32::from(v != 0 && r != 0);
        }
        Some(v)
    }

    fn compare(&mut self) -> Option<i32> {
        let mut v = self.sum()?;
        while let Some(op) = self.op(&["==", "!=", "<=", ">=", "<", ">"], &["<<", ">>"]) {
            let r = self.sum()?;
            v = i32::from(match op {
                "==" => v == r,
                "!=" => v != r,
                "<=" => v <= r,
                ">=" => v >= r,
                "<" => v < r,
                _ => v > r,
            });
        }
        Some(v)
    }

    fn sum(&mut self) -> Option<i32> {
        let mut v = self.bits()?;
        while let Some(op) = self.op(&["+", "-"], &[]) {
            let r = self.bits()?;
            v = if op == "+" {
                v.wrapping_add(r)
            } else {
                v.wrapping_sub(r)
            };
        }
        Some(v)
    }

    fn bits(&mut self) -> Option<i32> {
        let mut v = self.shift()?;
        while let Some(op) = self.op(&["&", "|", "^"], &["&&", "||"]) {
            let r = self.shift()?;
            v = match op {
                "&" => v & r,
                "|" => v | r,
                _ => v ^ r,
            };
        }
        Some(v)
    }

    fn shift(&mut self) -> Option<i32> {
        let mut v = self.product()?;
        while let Some(op) = self.op(&[">>>", "<<", ">>"], &[]) {
            let r = self.product()?;
            // A negative amount shifts the other way (rgbasm warns), and
            // 32 or more shifts every bit out.
            let by = r.unsigned_abs();
            let left = (op == "<<") == (r >= 0);
            v = if left {
                v.checked_shl(by).unwrap_or(0)
            } else if op == ">>>" {
                (v as u32).checked_shr(by).unwrap_or(0) as i32
            } else {
                v.checked_shr(by).unwrap_or(if v < 0 { -1 } else { 0 })
            };
        }
        Some(v)
    }

    fn product(&mut self) -> Option<i32> {
        let mut v = self.unary()?;
        while let Some(op) = self.op(&["*", "/", "%"], &["**"]) {
            let r = self.unary()?;
            if op == "*" {
                v = v.wrapping_mul(r);
                continue;
            }
            if r == 0 {
                return None;
            }
            // Division rounds down; the remainder takes the divisor's sign.
            let rem = v.wrapping_rem(r);
            let floored = rem != 0 && (rem < 0) != (r < 0);
            v = if op == "/" {
                v.wrapping_div(r) - i32::from(floored)
            } else {
                rem + if floored { r } else { 0 }
            };
        }
        Some(v)
    }

    /// A unary operator binds looser than `**`: `-2 ** 2` is -4.
    fn unary(&mut self) -> Option<i32> {
        if let Some(op) = self.op(&["+", "-", "~", "!"], &["!="]) {
            let v = self.nested(Self::unary)?;
            return Some(match op {
                "+" => v,
                "-" => v.wrapping_neg(),
                "~" => !v,
                _ => i32::from(v == 0),
            });
        }
        self.power()
    }

    /// `**` binds to the right, and its exponent is any unary expression
    /// (`8 ** !0`, `2 ** -1 ** 2`); a negative one is refused.
    fn power(&mut self) -> Option<i32> {
        let base = self.primary()?;
        if !self.eat("**") {
            return Some(base);
        }
        let exp = self.nested(Self::unary)?;
        Some(base.wrapping_pow(u32::try_from(exp).ok()?))
    }

    fn primary(&mut self) -> Option<i32> {
        self.skip_space();
        let s = self.s;
        let rest = &s[self.i..];
        match rest.first()? {
            b'(' => {
                self.i += 1;
                let v = self.nested(Self::or)?;
                self.eat(")").then_some(v)
            }
            &q @ (b'\'' | b'"') => {
                let (v, len) = character(rest, q)?;
                self.i += len;
                Some(v)
            }
            c if c.is_ascii_alphabetic() => {
                let len = rest
                    .iter()
                    .take_while(|c| c.is_ascii_alphanumeric())
                    .count();
                let name = std::str::from_utf8(&rest[..len]).ok()?.to_ascii_lowercase();
                self.i += len;
                let shift = match name.as_str() {
                    "high" => 8,
                    "low" => 0,
                    _ => return None,
                };
                if !self.eat("(") {
                    return None;
                }
                let v = self.nested(Self::or)?;
                self.eat(")").then_some((v >> shift) & 0xFF)
            }
            _ => {
                let len = number_len(rest);
                let text = std::str::from_utf8(&rest[..len]).ok()?;
                self.i += len;
                number(text)
            }
        }
    }
}

/// A character constant (`'x'`) or a one-character string (`"x"`, which
/// rgbasm still reads as a number) at the start of `s`, quoted by `quote`:
/// its value in the default charmap and its length in bytes. One ASCII
/// character or escape; `{` starts an interpolation, which isn't read.
fn character(s: &[u8], quote: u8) -> Option<(i32, usize)> {
    let (value, len) = match *s.get(1)? {
        b'\\' => {
            let value = match *s.get(2)? {
                c @ (b'\\' | b'"' | b'\'' | b'{' | b'}') => c,
                b'n' => b'\n',
                b'r' => b'\r',
                b't' => b'\t',
                b'0' => 0,
                _ => return None,
            };
            (value, 2)
        }
        b'{' => return None,
        c if c == quote || !c.is_ascii() => return None,
        c => (c, 1),
    };
    (s.get(1 + len) == Some(&quote)).then_some((i32::from(value), len + 2))
}

/// How many bytes of `s` are one number literal.
fn number_len(s: &[u8]) -> usize {
    let prefix = match s {
        [b'0', b'x' | b'X' | b'b' | b'B' | b'o' | b'O', ..] => 2,
        [b'$' | b'%' | b'&', ..] => 1,
        _ => 0,
    };
    prefix
        + s[prefix..]
            .iter()
            .take_while(|c| c.is_ascii_alphanumeric() || **c == b'_')
            .count()
}

/// A number literal: rgbasm's bases, with single `_`s between digits or
/// after a base prefix (`$_3`), never at the end.
fn number(text: &str) -> Option<i32> {
    let (digits, radix) = if let Some(d) = text
        .strip_prefix('$')
        .or_else(|| text.strip_prefix("0x"))
        .or_else(|| text.strip_prefix("0X"))
    {
        (d, 16)
    } else if let Some(d) = text
        .strip_prefix('%')
        .or_else(|| text.strip_prefix("0b"))
        .or_else(|| text.strip_prefix("0B"))
    {
        (d, 2)
    } else if let Some(d) = text
        .strip_prefix('&')
        .or_else(|| text.strip_prefix("0o"))
        .or_else(|| text.strip_prefix("0O"))
    {
        (d, 8)
    } else {
        // A leading `_` makes a decimal an identifier.
        (text.strip_prefix('_').map_or(text, |_| ""), 10)
    };
    if digits.ends_with('_') || digits.contains("__") {
        return None;
    }
    let digits: String = digits.chars().filter(|&c| c != '_').collect();
    if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
        return None;
    }
    // rgbasm keeps 32 bits of a number that has more.
    u64::from_str_radix(&digits, radix)
        .ok()
        .filter(|&v| v <= u64::from(u32::MAX))
        .map(|v| v as u32 as i32)
}

#[cfg(test)]
mod tests {
    use super::{MAX_DEPTH, Refused, eval};

    fn value(text: &str) -> Option<i32> {
        eval(text).ok()
    }

    #[test]
    fn numbers_in_every_base_and_characters() {
        for (text, v) in [
            ("56", 56),
            ("$38", 56),
            ("0x38", 56),
            ("%111000", 56),
            ("0b111000", 56),
            ("&70", 56),
            ("0o70", 56),
            ("5_6", 56),
            ("$_38", 56),
            ("'8'", 56),
            ("'C'", 67),
            ("\"8\"", 56),
            ("' '", 32),
            ("'\\n'", 10),
            ("'\\''", 39),
            ("'\\{'", 123),
            ("'}'", 125),
        ] {
            assert_eq!(value(text), Some(v), "{text}");
        }
        for text in [
            "", "_5", "$", "5x", "wFoo", "'ab'", "''", "'{'", "'\\x'", "\"ab\"", "5_", "5__6",
            "$__3", "$ 3",
        ] {
            assert_eq!(value(text), None, "{text}");
        }
    }

    #[test]
    fn operators_take_rgbasms_precedence() {
        for (text, v) in [
            ("8*7", 56),
            ("8*0", 0),
            ("1+1*2", 3),
            // `&` binds tighter than `+`, unlike C.
            ("2+7&1", 3),
            ("(2+7)&1", 1),
            ("1<<3|1", 9),
            // `**` binds tighter than unary minus, and to the right.
            ("-2**2", -4),
            ("2**3**2", 512),
            ("8**!0", 8),
            ("2**+3", 8),
            ("0**0", 1),
            ("-1%8", 7),
            ("7%-3", -2),
            ("-7/2", -4),
            ("~0", -1),
            ("!0", 1),
            ("1==1&&2>1", 1),
            ("-8>>1", -4),
            ("-8>>>28", 15),
            ("1>>-1", 2),
            ("4<<-1", 2),
            ("high($3812)", 0x38),
            ("LOW ( $3812 )", 0x12),
            ("2 ** $7FFFFFFF", 0),
        ] {
            assert_eq!(value(text), Some(v), "{text}");
        }
        for text in [
            "8/0", "8%0", "2**-1", "(1", "1)", "1+", "7 & & 1", "3 * * 1", "high(5", "bank(5)",
        ] {
            assert_eq!(value(text), None, "{text}");
        }
    }

    #[test]
    fn nesting_past_the_cap_is_refused_as_too_deep() {
        let at = format!("{}1{}", "(".repeat(MAX_DEPTH), ")".repeat(MAX_DEPTH));
        assert_eq!(eval(&at), Ok(1));
        let past = format!(
            "{}1{}",
            "(".repeat(MAX_DEPTH + 1),
            ")".repeat(MAX_DEPTH + 1)
        );
        assert_eq!(eval(&past), Err(Refused::TooDeep));
        assert_eq!(eval(&"-".repeat(100_000)), Err(Refused::TooDeep));
    }
}
