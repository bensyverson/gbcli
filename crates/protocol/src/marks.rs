//! gbcli's one mark table (Ben's rulings: pixel-fonts design ruling 3, and
//! one composition table owned by gbcli and read by its consumers): every
//! precomposed letter of Latin-1 and Latin Extended-A, and every kana
//! letter voiced by U+3099 or U+309A, as a base and one combining mark.
//! It is generated from Unicode's data by `scripts/fonts/gen_marks.py`, so
//! neither gbcli nor a consumer needs a normalization dependency: the font
//! reader refuses a decomposed key the table knows, font composition builds
//! a missing letter from it, and LoadHigh composes string literals to NFC
//! with it. Data and lookups only: no I/O, no allocation.

mod table;

pub use table::{DECOMPOSITIONS, MARKS, SINGLETONS};

/// One row of the table: `base` followed by `mark` is canonically
/// `precomposed`, and NFC composes the pair to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decomposition {
    pub base: char,
    pub mark: char,
    pub precomposed: char,
}

/// A character that is canonically, by itself, one of the table's letters
/// (U+212B ANGSTROM SIGN is canonically Å).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Singleton {
    pub singleton: char,
    pub canonical: char,
}

const fn row(base: char, mark: char, precomposed: char) -> Decomposition {
    Decomposition {
        base,
        mark,
        precomposed,
    }
}

const fn alias(singleton: char, canonical: char) -> Singleton {
    Singleton {
        singleton,
        canonical,
    }
}

/// The table's letter that `base` followed by `mark` composes to.
pub fn compose(base: char, mark: char) -> Option<char> {
    DECOMPOSITIONS
        .iter()
        .find(|d| d.base == base && d.mark == mark)
        .map(|d| d.precomposed)
}

/// The base and combining mark the table's letter `c` is made of.
pub fn decompose(c: char) -> Option<(char, char)> {
    DECOMPOSITIONS
        .iter()
        .find(|d| d.precomposed == c)
        .map(|d| (d.base, d.mark))
}

/// The table's letter that `c` is canonically, when `c` is a singleton
/// such as U+212B ANGSTROM SIGN.
pub fn singleton(c: char) -> Option<char> {
    SINGLETONS
        .iter()
        .find(|s| s.singleton == c)
        .map(|s| s.canonical)
}

/// Whether `c` is in the class of combining marks the table holds: the
/// Combining Diacritical Marks block (U+0300–U+036F) and the two kana
/// voicing marks (U+3099, U+309A). A mark in this class that doesn't
/// join its base is a decomposed sequence the table can't compose; any
/// other character is not a mark at all, as far as the table is concerned.
/// [`MARKS`] lists the ones the table's rows actually use.
pub fn is_combining_mark(c: char) -> bool {
    matches!(c, '\u{0300}'..='\u{036F}' | '\u{3099}' | '\u{309A}')
}

#[cfg(test)]
mod tests;
