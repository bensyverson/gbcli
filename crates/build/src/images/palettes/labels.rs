//! Two palette names can make one label: a label capitalizes each word
//! and drops the `_` between them, so `forest2` and `forest_2` are both
//! `PaletteForest2`. The rules (`NamedPalettes::faults`) see two names; the
//! program would see one symbol twice, so the file is refused at the
//! second line, naming the first.

use std::collections::BTreeMap;

use super::super::emit;
use super::{Bad, Parsed};

/// The problem's remedy, in place of the file's general one.
pub(super) const REMEDY: &str = "rename one of them; a palette's label capitalizes each word and \
                                 drops the `_`s, so `forest_2` is PaletteForest2, as `forest2` \
                                 is";

/// The first palette whose symbols an earlier one's already are.
pub(super) fn check(parsed: &Parsed) -> Result<(), Bad> {
    let mut by_symbol: BTreeMap<String, usize> = BTreeMap::new();
    for (i, p) in parsed.named.palettes.iter().enumerate() {
        for symbol in emit::palette_symbols(&p.name) {
            if let Some(&first) = by_symbol.get(&symbol) {
                let first_name = &parsed.named.palettes[first].name;
                return Err(Bad::at(
                    parsed.palette_lines[i],
                    format!(
                        "{} would define `{symbol}` in the program, as {first_name} (line {}) \
                         does",
                        p.name, parsed.palette_lines[first]
                    ),
                ));
            }
            by_symbol.insert(symbol, i);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::parse;
    use super::*;

    const COLORS: &str = "#000 #111 #222 #333";

    fn palettes(names: &[&str]) -> Parsed {
        let text: String = names.iter().map(|n| format!("{n}: {COLORS}\n")).collect();
        parse(&text).unwrap()
    }

    #[test]
    fn names_that_capitalize_alike_are_one_label() {
        let bad = check(&palettes(&["x_2", "y", "x2"])).unwrap_err();
        assert_eq!(bad.line, Some(3));
        for part in ["x2 would define `PaletteX2`", "as x_2 (line 1)"] {
            assert!(bad.message.contains(part), "{part:?} in {}", bad.message);
        }
        let bad = check(&palettes(&["a_b", "a__b"])).unwrap_err();
        assert!(bad.message.contains("`PaletteAB`"), "{}", bad.message);
    }

    #[test]
    fn labels_that_differ_in_case_pass() {
        // `PaletteAB` and `PaletteAb`: rgbasm's labels are case-sensitive.
        assert_eq!(check(&palettes(&["a_b", "ab"])), Ok(()));
    }
}
