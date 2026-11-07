//! One output row: a kind word, then `key=value` fields. A value with a space,
//! a quote or an `=` is printed as a JSON string, so every row splits the
//! same way however free its text.

use std::fmt::Write;

pub struct Row(String);

impl Row {
    pub fn new(kind: &str) -> Row {
        Row(kind.to_string())
    }

    /// A bare word after the kind (a stop reason).
    pub fn word(mut self, word: &str) -> Row {
        self.0.push(' ');
        self.0.push_str(&quoted(word));
        self
    }

    pub fn field(mut self, key: &str, value: impl std::fmt::Display) -> Row {
        let _ = write!(self.0, " {key}={}", quoted(&value.to_string()));
        self
    }

    /// A JSON value, compact and unquoted: it delimits itself.
    pub fn json(mut self, key: &str, value: &serde_json::Value) -> Row {
        let _ = write!(self.0, " {key}={value}");
        self
    }

    pub fn opt(self, key: &str, value: Option<impl std::fmt::Display>) -> Row {
        match value {
            Some(v) => self.field(key, v),
            None => self,
        }
    }

    pub fn flag(self, key: &str, value: bool) -> Row {
        self.field(key, u8::from(value))
    }

    pub fn line(self) -> String {
        self.0 + "\n"
    }
}

fn quoted(text: &str) -> String {
    let plain = !text.is_empty()
        && !text
            .chars()
            .any(|c| c.is_whitespace() || c == '"' || c == '=' || c.is_control());
    if plain {
        text.to_string()
    } else {
        serde_json::to_string(text).expect("a string always serializes")
    }
}

/// `$0163`: a CPU address.
pub fn addr(a: u16) -> String {
    format!("${a:04X}")
}

/// `$C0`: a byte.
pub fn byte(b: u8) -> String {
    format!("${b:02X}")
}

/// A value in hex, as wide as it needs (bytes, words, then more).
pub fn hex(v: i64) -> Option<String> {
    match v {
        0..=0xFF => Some(format!("${v:02X}")),
        0x100..=0xFFFF => Some(format!("${v:04X}")),
        0x1_0000.. => Some(format!("${v:X}")),
        _ => None,
    }
}

/// Bytes as `$02,$0A`: the spelling `poke` and `--poke` take.
pub fn bytes(b: &[u8]) -> String {
    b.iter().map(|&b| byte(b)).collect::<Vec<_>>().join(",")
}

/// Unix seconds as UTC: `2026-11-06T12:00:00Z`.
pub fn utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_text_is_quoted_and_plain_words_are_not() {
        let line = Row::new("read")
            .field("expr", "u16[wFrame] >= 60")
            .field("value", 1)
            .field("pc", addr(0x163))
            .field("empty", "")
            .line();
        assert_eq!(
            line,
            "read expr=\"u16[wFrame] >= 60\" value=1 pc=$0163 empty=\"\"\n"
        );
    }

    #[test]
    fn hex_widths_follow_the_value() {
        assert_eq!(hex(0x91).as_deref(), Some("$91"));
        assert_eq!(hex(0x4E3).as_deref(), Some("$04E3"));
        assert_eq!(hex(0x12345).as_deref(), Some("$12345"));
        assert_eq!(hex(-1), None);
        assert_eq!(bytes(&[0, 0xA]), "$00,$0A");
    }

    #[test]
    fn utc_dates_land_on_known_days() {
        assert_eq!(utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(utc(1_793_016_000), "2026-10-26T12:00:00Z");
    }
}
