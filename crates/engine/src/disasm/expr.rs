//! rgbasm's constant numeric expressions, for the operands `lookup` needs a
//! value of (an RST vector, a bit number): numbers in rgbasm's bases,
//! character constants, parentheses and every operator, with rgbasm 1.0.4's
//! precedence and 32-bit arithmetic (rgbasm(5), Operators). A symbol or a
//! function has no value here.

/// The value of `text` (whitespace already removed), or `None` when it
/// isn't a constant expression this reads or rgbasm would refuse it
/// (division by zero, a negative power).
pub(super) fn eval(text: &str) -> Option<i32> {
    let mut p = Parser {
        s: text.as_bytes(),
        i: 0,
    };
    let v = p.or()?;
    (p.i == p.s.len()).then_some(v)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn eat(&mut self, op: &str) -> bool {
        if self.s[self.i..].starts_with(op.as_bytes()) {
            self.i += op.len();
            true
        } else {
            false
        }
    }

    /// The first of `ops` here, longest first, unless it is the start of
    /// one of `not` (so `&` isn't read from `&&`).
    fn op(&mut self, ops: &[&'static str], not: &[&str]) -> Option<&'static str> {
        if not
            .iter()
            .any(|n| self.s[self.i..].starts_with(n.as_bytes()))
        {
            return None;
        }
        ops.iter().copied().find(|op| self.eat(op))
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
            // rgbasm shifts by any amount: 32 or more shifts every bit out.
            let by = u32::try_from(r).ok()?;
            v = match op {
                "<<" => v.checked_shl(by).unwrap_or(0),
                ">>" => v.checked_shr(by).unwrap_or(if v < 0 { -1 } else { 0 }),
                _ => (v as u32).checked_shr(by).unwrap_or(0) as i32,
            };
        }
        Some(v)
    }

    fn product(&mut self) -> Option<i32> {
        let mut v = self.unary()?;
        while let Some(op) = self.op(&["*", "/", "%"], &["**"]) {
            let r = self.unary()?;
            if op != "*" && r == 0 {
                return None;
            }
            // Division rounds down; the remainder takes the divisor's sign.
            let rem = v.wrapping_rem(r);
            let floored = rem != 0 && (rem < 0) != (r < 0);
            v = match op {
                "*" => v.wrapping_mul(r),
                "/" => v.wrapping_div(r) - i32::from(floored),
                _ => rem + if floored { r } else { 0 },
            };
        }
        Some(v)
    }

    fn unary(&mut self) -> Option<i32> {
        if self.eat("+") {
            return self.unary();
        }
        if self.eat("-") {
            return self.unary().map(i32::wrapping_neg);
        }
        if self.eat("~") {
            return self.unary().map(|v| !v);
        }
        if self.op(&["!"], &["!="]).is_some() {
            return self.unary().map(|v| i32::from(v == 0));
        }
        self.power()
    }

    /// `**` binds tighter than the unary operators and to the right.
    fn power(&mut self) -> Option<i32> {
        let base = self.primary()?;
        if !self.eat("**") {
            return Some(base);
        }
        let exp = self.unary_power()?;
        let exp = u32::try_from(exp).ok()?;
        Some((0..exp).fold(1i32, |acc, _| acc.wrapping_mul(base)))
    }

    /// A power's exponent: itself a power, perhaps negated.
    fn unary_power(&mut self) -> Option<i32> {
        if self.eat("-") {
            return self.unary_power().map(i32::wrapping_neg);
        }
        if self.eat("+") {
            return self.unary_power();
        }
        self.power()
    }

    fn primary(&mut self) -> Option<i32> {
        if self.eat("(") {
            let v = self.or()?;
            return self.eat(")").then_some(v);
        }
        let rest = &self.s[self.i..];
        if rest.first() == Some(&b'\'') {
            // One character, as the default charmap maps it.
            let (&c, close) = (rest.get(1)?, rest.get(2)?);
            if *close != b'\'' || c == b'\'' || c == b'\\' {
                return None;
            }
            self.i += 3;
            return Some(i32::from(c));
        }
        let len = number_len(rest);
        let text = std::str::from_utf8(&rest[..len]).ok()?;
        self.i += len;
        number(text)
    }
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

/// A number literal: rgbasm's bases, with `_` between digits.
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
        (text, 10)
    };
    if digits.is_empty() || digits.starts_with('_') {
        return None;
    }
    let digits: String = digits.chars().filter(|&c| c != '_').collect();
    if !digits.chars().all(|c| c.is_digit(radix)) {
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
    use super::eval;

    #[test]
    fn numbers_in_every_base_and_characters() {
        for (text, value) in [
            ("56", 56),
            ("$38", 56),
            ("0x38", 56),
            ("%111000", 56),
            ("0b111000", 56),
            ("&70", 56),
            ("0o70", 56),
            ("5_6", 56),
            ("'8'", 56),
        ] {
            assert_eq!(eval(text), Some(value), "{text}");
        }
        for text in ["", "_5", "$", "5x", "wFoo", "'ab'", "high(5)"] {
            assert_eq!(eval(text), None, "{text}");
        }
    }

    #[test]
    fn operators_take_rgbasms_precedence() {
        for (text, value) in [
            ("8*7", 56),
            ("1+1*2", 3),
            // `&` binds tighter than `+`, unlike C.
            ("2+7&1", 3),
            ("(2+7)&1", 1),
            ("1<<3|1", 9),
            // `**` binds tighter than unary minus, and to the right.
            ("-2**2", -4),
            ("2**3**2", 512),
            ("-1%8", 7),
            ("7%-3", -2),
            ("-7/2", -4),
            ("~0", -1),
            ("!0", 1),
            ("1==1&&2>1", 1),
            ("-8>>1", -4),
            ("-8>>>28", 15),
        ] {
            assert_eq!(eval(text), Some(value), "{text}");
        }
        for text in ["8/0", "8%0", "2**-1", "(1", "1)", "1+"] {
            assert_eq!(eval(text), None, "{text}");
        }
    }
}
