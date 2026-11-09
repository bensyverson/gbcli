//! gbcli's mark table (Ben's ruling 3): the precomposed letters of
//! Latin-1 and Latin Extended-A as a base and a combining mark, generated
//! from Unicode's data by `scripts/fonts/gen_marks.py`, so gbcli needs no
//! normalization dependency. The reader refuses a decomposed key the table
//! knows; composing a missing letter builds on it.

mod table;

use table::{DECOMPOSED, SINGLETONS};

/// The precomposed letter `base` followed by `mark` is, when the table
/// knows it.
pub fn precomposed(base: char, mark: char) -> Option<char> {
    DECOMPOSED
        .iter()
        .find(|(b, m, _)| *b == base && *m == mark)
        .map(|(_, _, letter)| *letter)
}

/// The base and combining mark the table's letter `c` is made of.
pub fn decompose(c: char) -> Option<(char, char)> {
    DECOMPOSED
        .iter()
        .find(|(_, _, letter)| *letter == c)
        .map(|(b, m, _)| (*b, *m))
}

/// The table's letter that `c` is canonically, when `c` is a singleton
/// such as U+212B ANGSTROM SIGN.
pub fn singleton(c: char) -> Option<char> {
    SINGLETONS
        .iter()
        .find(|(s, _)| *s == c)
        .map(|(_, letter)| *letter)
}

/// Whether `c` is in a block of combining marks, which draw over the
/// character before them.
pub(crate) fn is_combining(c: char) -> bool {
    matches!(
        c,
        '\u{0300}'..='\u{036F}'
            | '\u{1AB0}'..='\u{1AFF}'
            | '\u{1DC0}'..='\u{1DFF}'
            | '\u{20D0}'..='\u{20FF}'
            | '\u{FE20}'..='\u{FE2F}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_knows_latin_1_and_extended_a_letters_by_base_and_mark() {
        assert_eq!(precomposed('e', '\u{0301}'), Some('é'));
        assert_eq!(precomposed('Z', '\u{030C}'), Some('Ž'));
        assert_eq!(precomposed('c', '\u{0327}'), Some('ç'));
        assert_eq!(precomposed('x', '\u{0301}'), None);
        assert_eq!(singleton('\u{212B}'), Some('Å'));
        assert_eq!(singleton('A'), None);
        // Every letter in the table is in the two blocks, once.
        let letters: std::collections::BTreeSet<char> =
            DECOMPOSED.iter().map(|(_, _, l)| *l).collect();
        assert_eq!(letters.len(), DECOMPOSED.len());
        assert!(
            letters
                .iter()
                .all(|l| ('\u{00C0}'..='\u{017F}').contains(l))
        );
        assert!(DECOMPOSED.iter().all(|(_, m, _)| is_combining(*m)));
        assert_eq!(decompose('é'), Some(('e', '\u{0301}')));
        assert_eq!(decompose('Ž'), Some(('Z', '\u{030C}')));
        assert_eq!(decompose('e'), None);
    }
}
