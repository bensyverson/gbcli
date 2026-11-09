//! A working copy's base: the font as it was when the copy was made,
//! recorded as a hash of its bytes in `NAME.base` beside the copy, so a
//! save can refuse to write the copy's glyphs over a font that changed
//! since. Beside the copy, not inside it, because an editor that rewrites
//! the PNG or BDF drops whatever it doesn't know.
//!
//! Data in, data out: the caller reads and writes the files.

/// The base file's extension.
pub(crate) const BASE_EXTENSION: &str = "base";

/// What a working copy was made from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Base {
    /// No font yet (`font new`).
    Absent,
    /// A font whose bytes hash to this (FNV-1a, 64-bit).
    Hash(u64),
}

const ABSENT: &str = "absent";
const HASH: &str = "fnv1a64";

impl Base {
    /// The base of a font with these bytes, or of none.
    pub fn of(font: Option<&[u8]>) -> Base {
        match font {
            None => Base::Absent,
            Some(bytes) => Base::Hash(fnv1a64(bytes)),
        }
    }

    /// The base file's text.
    pub fn to_text(self) -> String {
        match self {
            Base::Absent => format!("{ABSENT}\n"),
            Base::Hash(h) => format!("{HASH} {h:016x}\n"),
        }
    }

    /// The base a base file's text records, if it is one.
    pub fn parse(text: &str) -> Option<Base> {
        let text = text.trim();
        if text == ABSENT {
            return Some(Base::Absent);
        }
        let hex = text.strip_prefix(HASH)?.strip_prefix(' ')?;
        (hex.len() == 16)
            .then(|| u64::from_str_radix(hex, 16).ok())
            .flatten()
            .map(Base::Hash)
    }
}

/// How the font differs from the copy's base, when it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Drift {
    /// The font was edited after the copy was made.
    Changed,
    /// The font was made after the copy (of a font not yet made).
    Made,
    /// The font was removed after the copy was made.
    Removed,
}

/// Whether a copy recorded as `recorded` may be saved over the font as it
/// is `now`. A copy with no record (made before gbcli recorded one, or
/// its record removed to force the save) may.
pub(crate) fn drift(recorded: Option<Base>, now: Base) -> Option<Drift> {
    match (recorded?, now) {
        (a, b) if a == b => None,
        (Base::Absent, Base::Hash(_)) => Some(Drift::Made),
        (Base::Hash(_), Base::Absent) => Some(Drift::Removed),
        _ => Some(Drift::Changed),
    }
}

/// FNV-1a, 64-bit: a change check, not a defense against forgery.
fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_base_reads_back_from_its_text_and_nothing_else_reads_as_one() {
        for b in [Base::Absent, Base::of(Some(b"x")), Base::Hash(0)] {
            assert_eq!(Base::parse(&b.to_text()), Some(b));
        }
        for t in [
            "",
            "absent!",
            "fnv1a64 12",
            "fnv1a64  0000000000000000",
            "sha 00",
        ] {
            assert_eq!(Base::parse(t), None, "{t:?}");
        }
    }

    #[test]
    fn fnv1a64_matches_its_reference_values() {
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn only_a_font_unlike_the_record_drifts() {
        let a = Base::of(Some(b"a"));
        let b = Base::of(Some(b"b"));
        assert_eq!(drift(Some(a), a), None);
        assert_eq!(drift(None, b), None);
        assert_eq!(drift(Some(a), b), Some(Drift::Changed));
        assert_eq!(drift(Some(Base::Absent), b), Some(Drift::Made));
        assert_eq!(drift(Some(a), Base::Absent), Some(Drift::Removed));
        assert_eq!(drift(Some(Base::Absent), Base::Absent), None);
    }
}
