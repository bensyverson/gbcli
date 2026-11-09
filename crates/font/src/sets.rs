//! The named character sets `gbcli font new --chars SET` and `import
//! --chars SET` take, in code point order.

use crate::key::Key;

/// Every set's name, for a remedy.
pub const SET_NAMES: &[&str] = &["ascii", "latin1", "latin-ext-a"];

/// The characters of the set `name`: `ascii` (U+0020-U+007E), `latin1`
/// (ascii and U+00A0-U+00FF) or `latin-ext-a` (latin1 and
/// U+0100-U+017F).
pub fn named_set(name: &str) -> Option<Vec<Key>> {
    let last = match name {
        "ascii" => '~',
        "latin1" => '\u{FF}',
        "latin-ext-a" => '\u{17F}',
        _ => return None,
    };
    let past_ascii = ('\u{A0}'..=last).filter(|_| last != '~');
    Some((' '..='~').chain(past_ascii).map(Key::Char).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sets_nest_in_code_point_order() {
        let ascii = named_set("ascii").unwrap();
        assert_eq!(ascii.len(), 95);
        assert_eq!(ascii[0], Key::Char(' '));
        let latin1 = named_set("latin1").unwrap();
        assert_eq!(latin1.len(), 95 + 96);
        assert_eq!(latin1[95], Key::Char('\u{A0}'));
        assert_eq!(named_set("latin-ext-a").unwrap().len(), 95 + 96 + 128);
        assert_eq!(named_set("klingon"), None);
        for name in SET_NAMES {
            assert!(named_set(name).is_some());
        }
    }
}
