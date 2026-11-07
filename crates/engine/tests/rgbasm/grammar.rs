//! The differential test's grammar (`opcode_rgbasm_differential.rs`, which
//! lists the documented differences this keeps out): instruction lines
//! from a seed, starting from every form in the opcode table.

use gbcli_engine::disasm::{LookupError, Opcode, form, lookup};

/// A few-line xorshift64*, so the lines are the same on every run.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    /// True `percent` times in a hundred.
    fn odds(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len())]
    }

    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % (hi - lo + 1) as u64) as i64
    }
}

/// Every form the table has an instruction for, as the lookup writes it
/// (`jr nz, e8`, `ldh [a8], a`, `bit 7, [hl]`).
fn templates() -> Vec<String> {
    let all = (0..=255).map(Opcode::Base).chain((0..=255).map(Opcode::Cb));
    all.map(|op| form(op).asm)
        .filter(|asm| {
            let word = asm.split(' ').next().unwrap();
            !matches!(lookup(word), Err(LookupError::NoMnemonic { .. }))
        })
        .collect()
}

/// `count` lines from `seed`.
pub fn lines(seed: u64, count: usize) -> Vec<String> {
    let templates = templates();
    let mut rng = Rng::new(seed);
    (0..count).map(|_| line(&mut rng, &templates)).collect()
}

fn line(rng: &mut Rng, templates: &[String]) -> String {
    let template = &templates[rng.below(templates.len())];
    let (mut word, rest) = template.split_once(' ').unwrap_or((template, ""));
    let mut operands: Vec<String> = if rest.is_empty() {
        Vec::new()
    } else {
        rest.split(", ").map(|op| operand(rng, word, op)).collect()
    };
    // rgbasm's other spellings of whole instructions.
    let alu = ["add", "adc", "sub", "sbc", "and", "xor", "or", "cp"];
    if alu.contains(&word) && operands.len() == 2 && rng.odds(40) {
        operands.remove(0);
    }
    if word == "cpl" && rng.odds(30) {
        operands.push(cased(rng, "a"));
    }
    let step = operands.iter().position(|o| o == "[hl+]" || o == "[hl-]");
    if let Some(i) = step.filter(|_| word == "ld" && rng.odds(30)) {
        word = if operands[i] == "[hl+]" { "ldi" } else { "ldd" };
        operands[i] = cased(rng, "[hl]");
    }
    let high_c = rest.contains("[c]") && !operands.iter().any(|o| o.eq_ignore_ascii_case("[c]"));
    if word == "ldh" && high_c && rng.odds(20) {
        word = "ld";
    }
    let mut text = cased(rng, word);
    if !operands.is_empty() {
        // rgbasm's mnemonic ends where its name does: `rl[hl]`.
        let glued = !operands[0].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_');
        let gaps: &[&str] = if glued {
            &[" ", " ", "\t", ""]
        } else {
            &[" ", " ", "\t", "  "]
        };
        text.push_str(rng.pick(gaps));
        let mut joined = String::new();
        for (i, o) in operands.iter().enumerate() {
            if i > 0 {
                joined.push_str(rng.pick(&[", ", ", ", ",", " , ", " ,"]));
            }
            joined.push_str(o);
        }
        text.push_str(&joined);
    }
    if rng.odds(30) {
        text = junk(rng, &text);
    }
    if rng.odds(10) {
        text.push_str(rng.pick(&[" ; ", ";", "\t; "]));
        for _ in 0..rng.below(8) {
            text.push(rng.pick(&COMMENT));
        }
    }
    text
}

const COMMENT: [char; 12] = ['x', ' ', '\'', '"', ';', ',', '[', '(', '\\', '{', '0', 'é'];

/// One operand of a template, written as rgbasm may take it.
fn operand(rng: &mut Rng, mnemonic: &str, op: &str) -> String {
    let halves = [
        ("a", "af", "high"),
        ("b", "bc", "high"),
        ("c", "bc", "low"),
        ("d", "de", "high"),
        ("e", "de", "low"),
        ("h", "hl", "high"),
        ("l", "hl", "low"),
    ];
    let branch = matches!(mnemonic, "jr" | "jp" | "call" | "ret");
    match op {
        // A condition, or `!` and its opposite (`!nc` is `c`); now and then
        // `LOW(bc)`, which is the register c and never the condition.
        "nz" | "z" | "nc" | "c" if branch => match rng.below(10) {
            0 | 1 => {
                let opposite = match op {
                    "nz" => "z",
                    "z" => "nz",
                    "nc" => "c",
                    _ => "nc",
                };
                let (nots, cond) = rng.pick(&[("!", opposite), ("! ", opposite), ("!!", op)]);
                format!("{nots}{}", cased(rng, cond))
            }
            2 if op == "c" => {
                let f = cased(rng, "low");
                format!("{f}({})", cased(rng, "bc"))
            }
            _ => cased(rng, op),
        },
        "a" | "b" | "c" | "d" | "e" | "h" | "l" if rng.odds(10) => {
            let &(_, pair, half) = halves.iter().find(|h| h.0 == op).unwrap();
            let space = if rng.odds(30) { " " } else { "" };
            format!("{}{space}({})", cased(rng, half), cased(rng, pair))
        }
        "[hl+]" => {
            let spelling = rng.pick(&["[hl+]", "[hli]", "[hl +]", "[ hl+ ]"]);
            cased(rng, spelling)
        }
        "[hl-]" => {
            let spelling = rng.pick(&["[hl-]", "[hld]", "[hl -]", "[ hl- ]"]);
            cased(rng, spelling)
        }
        "[c]" => match rng.below(3) {
            0 => cased(rng, "[c]"),
            _ => {
                let (l, r) = spaces(rng);
                format!(
                    "[{l}{}{l}+{r}{}{r}]",
                    konst(rng, 0xFF00, 0),
                    cased(rng, "c")
                )
            }
        },
        "n8" | "n16" | "a16" => data(rng),
        "[a16]" => format!("[{}]", data(rng)),
        "[a8]" => format!("[{}]", ldh_address(rng)),
        "e8" if mnemonic == "jr" => jr_target(rng),
        "e8" => small(rng),
        "sp+e8" => {
            let (l, r) = spaces(rng);
            let sign = if rng.odds(50) { "+" } else { "-" };
            format!("{}{l}{sign}{r}{}", cased(rng, "sp"), small(rng))
        }
        _ if op.starts_with('$') || op.chars().all(|c| c.is_ascii_digit()) => {
            let value = i64::from_str_radix(op.trim_start_matches('$'), 16).unwrap();
            // Now and then one off, which rgbasm refuses.
            let value = if rng.odds(5) { value + 1 } else { value };
            konst(rng, value, 0)
        }
        _ if op.starts_with('[') => {
            let (l, _) = spaces(rng);
            format!("[{l}{}{l}]", cased(rng, &op[1..op.len() - 1]))
        }
        _ => cased(rng, op),
    }
}

/// Lowercase mostly, else uppercase or mixed: registers, mnemonics,
/// conditions and functions, never symbols.
fn cased(rng: &mut Rng, text: &str) -> String {
    match rng.below(10) {
        0 => text.to_ascii_uppercase(),
        1 => text
            .chars()
            .map(|c| {
                if rng.odds(50) {
                    c.to_ascii_uppercase()
                } else {
                    c
                }
            })
            .collect(),
        _ => text.to_string(),
    }
}

/// Spacing around an operator: glued, spaced, or one side.
fn spaces(rng: &mut Rng) -> (&'static str, &'static str) {
    rng.pick(&[("", ""), (" ", " "), ("", " "), (" ", "")])
}

/// A number literal of `value` (non-negative) in one of rgbasm's bases,
/// with `_`s now and then, and rarely one rgbasm refuses.
fn number(rng: &mut Rng, value: i64) -> String {
    let (prefixes, radix): (&[&str], u32) = match rng.below(7) {
        0..=2 => (&[""], 10),
        3 => (&["$", "$", "0x", "0X"], 16),
        4 => (&["%", "0b", "0B"], 2),
        5 => (&["&", "0o", "0O"], 8),
        _ => (&["$"], 16),
    };
    let prefix = rng.pick(prefixes);
    let mut digits = match radix {
        10 => format!("{value}"),
        16 => format!("{value:x}"),
        2 => format!("{value:b}"),
        _ => format!("{value:o}"),
    };
    if rng.odds(10) {
        digits.insert(0, '0');
    }
    if radix == 16 {
        digits = cased(rng, &digits);
    }
    let mut out = String::from(prefix);
    if !prefix.is_empty() && rng.odds(5) {
        out.push('_');
    }
    let chars: Vec<char> = digits.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        out.push(*c);
        if i + 1 < chars.len() && rng.odds(8) {
            out.push('_');
        }
    }
    if rng.odds(2) {
        out.push('_');
    }
    out
}

/// A character constant or one-character string, with escapes.
fn character(rng: &mut Rng, value: i64) -> Option<String> {
    let c = char::from_u32(u32::try_from(value).ok()?)?;
    let quote = if rng.odds(80) { '\'' } else { '"' };
    let escaped = match c {
        '\n' => "\\n".into(),
        '\t' => "\\t".into(),
        '\r' => "\\r".into(),
        '\0' => "\\0".into(),
        '\\' => "\\\\".into(),
        '{' => "\\{".into(),
        '}' if rng.odds(50) => "\\}".into(),
        c if c == quote => format!("\\{c}"),
        '\'' | '"' if rng.odds(30) => format!("\\{c}"),
        c if (' '..='~').contains(&c) => c.to_string(),
        _ => return None,
    };
    Some(format!("{quote}{escaped}{quote}"))
}

/// A constant expression meant to equal `value`, with no symbol in it.
/// Near enough: its parts aren't parenthesized, so rgbasm's precedence may
/// make it another value, and rgbasm referees whatever it is.
fn konst(rng: &mut Rng, value: i64, depth: usize) -> String {
    let (l, r) = spaces(rng);
    let leaf = depth >= 3 || rng.odds(35);
    if leaf {
        let quoted = ((0..128).contains(&value) && rng.odds(20))
            .then(|| character(rng, value))
            .flatten();
        if let Some(c) = quoted {
            return c;
        }
        return if value < 0 {
            format!("-{l}{}", number(rng, -value))
        } else {
            number(rng, value)
        };
    }
    let d = depth + 1;
    match rng.below(12) {
        0 => {
            let (pl, pr) = if rng.odds(30) { (" ", " ") } else { ("", "") };
            format!("({pl}{}{pr})", konst(rng, value, d))
        }
        1 => {
            let a = rng.range(0, 300);
            format!("{}{l}+{r}{}", konst(rng, a, d), konst(rng, value - a, d))
        }
        2 => {
            let a = rng.range(0, 300);
            format!("{}{l}-{r}{}", konst(rng, value + a, d), konst(rng, a, d))
        }
        3 => {
            let m = rng.range(1, 9);
            let rem = rng.range(0, m - 1);
            if value >= 0 {
                format!(
                    "{}{l}/{r}{}",
                    konst(rng, value * m + rem, d),
                    konst(rng, m, d)
                )
            } else {
                format!("{}{l}*{r}{}", konst(rng, value, d), konst(rng, 1, d))
            }
        }
        4 if (0..0x100).contains(&value) => {
            let low = rng.odds(50);
            let f = cased(rng, if low { "low" } else { "high" });
            let space = if rng.odds(20) { " " } else { "" };
            let other = rng.range(0, 255);
            let inner = if low {
                other * 256 + value
            } else {
                value * 256 + other
            };
            format!("{f}{space}({})", konst(rng, inner, d))
        }
        5 if value >= 0 => {
            let s = rng.range(1, 4);
            format!("{}{l}>>{r}{}", konst(rng, value << s, d), konst(rng, s, d))
        }
        6 if value >= 0 && value % 4 == 0 => {
            format!("{}{l}<<{r}{}", konst(rng, value / 4, d), konst(rng, 2, d))
        }
        7 if (0..0x10000).contains(&value) => {
            let mask = if value < 256 { 0xFF } else { 0xFFFF };
            let high = rng.range(1, 15) << 16;
            let op = rng.pick(&["&", "|", "^"]);
            match op {
                "&" => format!(
                    "{}{l}&{r}{}",
                    konst(rng, value | high, d),
                    number(rng, mask)
                ),
                "|" => format!("{}{l}|{r}{}", konst(rng, value, d), konst(rng, 0, d)),
                _ => format!(
                    "{}{l}^{r}{}",
                    konst(rng, value ^ 0x55, d),
                    konst(rng, 0x55, d)
                ),
            }
        }
        8 if value >= 0 => {
            let m = value + rng.range(1, 20);
            let k = rng.range(0, 3);
            format!(
                "{}{l}%{r}{}",
                konst(rng, value + m * k, d),
                konst(rng, m, d)
            )
        }
        9 => match rng.below(4) {
            0 => format!("-{l}{}", konst(rng, -value, d)),
            1 => format!("~{l}{}", konst(rng, !value, d)),
            2 => format!("+{l}{}", konst(rng, value, d)),
            _ => format!("{}{l}**{r}{}", konst(rng, value, d), konst(rng, 1, d)),
        },
        10 if value == 0 || value == 1 => {
            let a = rng.range(0, 9);
            let (op, b) = match (value, rng.below(4)) {
                (1, 2) => return format!("{}{l}||{r}{}", konst(rng, a + 1, d), konst(rng, 0, d)),
                (1, 0) => ("==", a),
                (0, 0) => ("!=", a),
                (1, 1) => ("<", a + 1),
                (0, 1) => (">=", a + 1),
                (0, 2) => ("&&", 0),
                (_, _) => ("<=", if value == 1 { a } else { a - 1 }),
            };
            format!("{}{l}{op}{r}{}", konst(rng, a, d), konst(rng, b, d))
        }
        11 if (0..=0x7FFF).contains(&value) => {
            format!("{}{l}*{r}{}", konst(rng, value, d), konst(rng, 1, d))
        }
        _ => konst(rng, value, depth + 3),
    }
}

/// A small signed value: sp's offset, `add sp`'s operand.
fn small(rng: &mut Rng) -> String {
    let value = rng.range(-128, 127);
    konst(rng, value, 1)
}

/// Data or an address: a constant, a symbol, or rgbasm's other numbers.
fn data(rng: &mut Rng) -> String {
    let (l, r) = spaces(rng);
    let symbol = rng.pick(&["wFoo", "rLCDC", "Top", "@", "."]);
    match rng.below(14) {
        0..=5 => {
            let value = rng.range(0, 0xFFFF);
            let value = if rng.odds(30) { value & 0xFF } else { value };
            konst(rng, value, 0)
        }
        6 => symbol.into(),
        7 => {
            let op = rng.pick(&["+", "-", "|", "&", "*", "**", "<<"]);
            let value = rng.range(0, 3);
            format!("{symbol}{l}{op}{r}{}", konst(rng, value, 1))
        }
        8 => {
            let f = rng.pick(&["high", "low"]);
            let f = cased(rng, f);
            let space = if rng.odds(20) { " " } else { "" };
            format!("{f}{space}({symbol})")
        }
        9 => format!("{}{symbol}", rng.pick(&["!", "~", "-", "+"])),
        10 => {
            let arg = rng.pick(&["Top", "@"]);
            let f = rng.pick(&["BANK", "bank"]);
            format!("{f}({arg})")
        }
        11 => rng
            .pick(&[
                "SIZEOF(\"s\")",
                "STRLEN(\"abc\")",
                "DEF(wFoo)",
                "ISCONST(wFoo)",
                "BITWIDTH(5)",
                "STRCMP(\"a\", \"b\")",
                "strlen (\"\")",
            ])
            .into(),
        12 => rng
            .pick(&[
                "1.5", "5.", "1.5q8", "0.25", "5.5_5", "1.5e", "5._5", "5_.5",
            ])
            .into(),
        _ => {
            let digits: String = (0..rng.range(1, 9))
                .map(|_| char::from(b'0' + rng.below(4) as u8))
                .collect();
            format!("`{digits}")
        }
    }
}

/// `ldh`'s address: mostly $FF00-$FFFF, sometimes not.
fn ldh_address(rng: &mut Rng) -> String {
    let (l, r) = spaces(rng);
    match rng.below(6) {
        0 => {
            let value = rng.range(0, 0x20);
            let op = rng.pick(&["+", "-"]);
            format!("rLCDC{l}{op}{r}({})", konst(rng, value, 1))
        }
        1 => "rLCDC".into(),
        2 => {
            let value = rng.range(0, 0x1FFFF);
            konst(rng, value, 0)
        }
        _ => {
            let value = rng.range(0xFF00, 0xFFFF);
            konst(rng, value, 0)
        }
    }
}

/// `jr`'s target, within reach of the line.
fn jr_target(rng: &mut Rng) -> String {
    let (l, r) = spaces(rng);
    let at = rng.pick(&["Top", "@"]);
    match rng.below(3) {
        0 => at.into(),
        _ => {
            let value = rng.range(0, 100);
            let op = rng.pick(&["+", "-"]);
            format!("{at}{l}{op}{r}({})", konst(rng, value, 1))
        }
    }
}

/// The line with one thing wrong, or nearly: a character in or out, a
/// comma missing or extra, parentheses for brackets, or cut short. Cuts and
/// insertions fall between words, never inside an identifier; a deletion
/// can still join two words into a symbol that isn't defined, which the
/// test sets aside by rgblink's error.
fn junk(rng: &mut Rng, text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    // Positions between two characters that aren't both part of a word.
    let cuts: Vec<usize> = (1..chars.len())
        .filter(|&i| !(ident(chars[i - 1]) && ident(chars[i])))
        .collect();
    let at = if cuts.is_empty() {
        chars.len()
    } else {
        rng.pick(&cuts)
    };
    let (head, tail): (String, String) =
        (chars[..at].iter().collect(), chars[at..].iter().collect());
    match rng.below(7) {
        0 => {
            let c = rng.pick(&[
                ' ', ',', '(', ')', '[', ']', '\'', '"', '$', '%', '&', '+', '-', '*', '/', '<',
                '>', '=', '!', '~', '^', '|', '_', ';', '\\', '{', '}', '0', '1', '7', '8',
            ]);
            format!("{head}{c}{tail}")
        }
        1 => {
            // A character out, if it isn't a letter.
            let others: Vec<usize> = (0..chars.len())
                .filter(|&i| !chars[i].is_ascii_alphabetic())
                .collect();
            if others.is_empty() {
                return text.into();
            }
            let gone = rng.pick(&others);
            (0..chars.len())
                .filter(|&i| i != gone)
                .map(|i| chars[i])
                .collect()
        }
        2 => text.replacen(',', rng.pick(&["", " "]), 1),
        3 => format!("{text}{}", rng.pick(&[",", ", a", ", 1", " a"])),
        4 => text.replace('[', "(").replace(']', ")"),
        5 => head,
        _ => format!("{head} {tail}"),
    }
}
