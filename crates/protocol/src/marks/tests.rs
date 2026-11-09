use std::collections::BTreeSet;

use super::*;

#[test]
fn the_table_knows_latin_1_and_extended_a_letters_by_base_and_mark() {
    assert_eq!(compose('e', '\u{0301}'), Some('é'));
    assert_eq!(compose('Z', '\u{030C}'), Some('Ž'));
    assert_eq!(compose('c', '\u{0327}'), Some('ç'));
    assert_eq!(compose('x', '\u{0301}'), None);
    assert_eq!(decompose('é'), Some(('e', '\u{0301}')));
    assert_eq!(decompose('Ž'), Some(('Z', '\u{030C}')));
    assert_eq!(decompose('e'), None);
    assert_eq!(singleton('\u{212B}'), Some('Å'));
    assert_eq!(singleton('A'), None);
}

#[test]
fn the_kana_voicing_marks_compose_every_voiced_kana() {
    const DAKUTEN: char = '\u{3099}';
    const HANDAKUTEN: char = '\u{309A}';
    let voiced = [
        ('か', 'が'),
        ('こ', 'ご'),
        ('さ', 'ざ'),
        ('そ', 'ぞ'),
        ('た', 'だ'),
        ('つ', 'づ'),
        ('と', 'ど'),
        ('は', 'ば'),
        ('ほ', 'ぼ'),
        ('う', 'ゔ'),
        ('ゝ', 'ゞ'),
        ('カ', 'ガ'),
        ('ト', 'ド'),
        ('ホ', 'ボ'),
        ('ウ', 'ヴ'),
        ('ワ', 'ヷ'),
        ('ヰ', 'ヸ'),
        ('ヱ', 'ヹ'),
        ('ヲ', 'ヺ'),
        ('ヽ', 'ヾ'),
    ];
    for (base, letter) in voiced {
        assert_eq!(compose(base, DAKUTEN), Some(letter), "{base}");
        assert_eq!(decompose(letter), Some((base, DAKUTEN)), "{letter}");
    }
    let half_voiced = [
        ('は', 'ぱ'),
        ('ひ', 'ぴ'),
        ('ふ', 'ぷ'),
        ('へ', 'ぺ'),
        ('ほ', 'ぽ'),
        ('ハ', 'パ'),
        ('ヒ', 'ピ'),
        ('フ', 'プ'),
        ('ヘ', 'ペ'),
        ('ホ', 'ポ'),
    ];
    for (base, letter) in half_voiced {
        assert_eq!(compose(base, HANDAKUTEN), Some(letter), "{base}");
        assert_eq!(decompose(letter), Some((base, HANDAKUTEN)), "{letter}");
    }
    // Unvoiceable kana and a mark on the wrong base compose to nothing.
    assert_eq!(compose('あ', DAKUTEN), None);
    assert_eq!(compose('か', HANDAKUTEN), None);
    assert_eq!(compose('e', DAKUTEN), None);
    // 22 hiragana and 26 katakana take a dakuten, 5 of each a handakuten.
    let count = |mark: char| DECOMPOSITIONS.iter().filter(|d| d.mark == mark).count();
    assert_eq!(count(DAKUTEN), 22 + 26);
    assert_eq!(count(HANDAKUTEN), 5 + 5);
}

#[test]
fn every_row_round_trips_and_names_one_letter() {
    for d in DECOMPOSITIONS {
        assert_eq!(compose(d.base, d.mark), Some(d.precomposed), "{d:?}");
        assert_eq!(decompose(d.precomposed), Some((d.base, d.mark)), "{d:?}");
        assert!(!is_combining_mark(d.base), "{d:?}");
        assert!(is_combining_mark(d.mark), "{d:?}");
        assert!(MARKS.contains(&d.mark), "{d:?}");
        assert_eq!(singleton(d.precomposed), None, "{d:?}");
    }
    let letters: BTreeSet<char> = DECOMPOSITIONS.iter().map(|d| d.precomposed).collect();
    assert_eq!(letters.len(), DECOMPOSITIONS.len());
    let bases: BTreeSet<char> = DECOMPOSITIONS.iter().map(|d| d.base).collect();
    for s in SINGLETONS {
        let target = s.canonical;
        assert!(
            letters.contains(&target) || bases.contains(&target) || is_combining_mark(target),
            "{s:?}"
        );
        assert!(!letters.contains(&s.singleton), "{s:?}");
        assert!(!bases.contains(&s.singleton), "{s:?}");
    }
}

/// From the rev-cli review's probe: NFC maps these singletons onto a mark
/// or a base the table composes with, so composing a string first maps
/// them (K, U+212A KELVIN SIGN, then U+0327 is Ķ under NFC).
#[test]
fn singletons_onto_a_mark_or_a_base_are_in_the_table() {
    assert_eq!(singleton('\u{0340}'), Some('\u{0300}'));
    assert_eq!(singleton('\u{0341}'), Some('\u{0301}'));
    assert_eq!(singleton('\u{0343}'), Some('\u{0313}'));
    assert_eq!(singleton('\u{212A}'), Some('K'));
    assert_eq!(compose('K', '\u{0327}'), Some('\u{0136}'));
    assert_eq!(singleton('\u{0300}'), None);
    assert_eq!(singleton('K'), None);
}

#[test]
fn marks_lists_each_mark_the_rows_use_once_in_order() {
    let used: BTreeSet<char> = DECOMPOSITIONS.iter().map(|d| d.mark).collect();
    let listed: Vec<char> = used.into_iter().collect();
    assert_eq!(MARKS, listed.as_slice());
    assert!(MARKS.contains(&'\u{0301}'));
    assert!(MARKS.contains(&'\u{3099}'));
    assert!(MARKS.contains(&'\u{309A}'));
    // In the class but used by no row.
    assert!(is_combining_mark('\u{036F}') && !MARKS.contains(&'\u{036F}'));
}

#[test]
fn the_mark_class_is_the_diacritical_block_and_the_two_voicing_marks() {
    for c in ['\u{0300}', '\u{0301}', '\u{036F}', '\u{3099}', '\u{309A}'] {
        assert!(is_combining_mark(c), "u+{:04X}", u32::from(c));
    }
    // Neighbors: U+309B and U+309C are the spacing (non-combining) marks.
    for c in [
        '\u{02FF}', '\u{0370}', '\u{3098}', '\u{309B}', '\u{309C}', '\u{1AB0}', '\u{1DC0}',
        '\u{20D0}', '\u{FE20}', 'e', 'か',
    ] {
        assert!(!is_combining_mark(c), "u+{:04X}", u32::from(c));
    }
}
