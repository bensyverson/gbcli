use super::*;

fn refused(label: &str) -> KeyError {
    Key::from_label(label).expect_err(label)
}

#[test]
fn a_label_is_a_character_a_code_point_or_a_name() {
    assert_eq!(Key::from_label("'A'"), Ok(Key::Char('A')));
    assert_eq!(Key::from_label("'é'"), Ok(Key::Char('é')));
    assert_eq!(Key::from_label("'♥'"), Ok(Key::Char('♥')));
    assert_eq!(Key::from_label("'''"), Ok(Key::Char('\'')));
    assert_eq!(Key::from_label("':'"), Ok(Key::Char(':')));
    assert_eq!(Key::from_label("u+00E9"), Ok(Key::Char('é')));
    assert_eq!(Key::from_label("U+0020"), Ok(Key::Char(' ')));
    assert_eq!(Key::from_label("u+0301"), Ok(Key::Char('\u{0301}')));
    assert_eq!(Key::from_label("\"heart\""), Ok(Key::Name("heart".into())));
    assert_eq!(Key::from_label("\"btn_a\""), Ok(Key::Name("btn_a".into())));
}

#[test]
fn a_decomposed_key_the_mark_table_knows_is_refused_naming_its_precomposed_form() {
    for label in ["u+0065, u+0301", "'e', u+0301", "'e\u{0301}'"] {
        let e = refused(label);
        assert!(e.message.contains("decomposed"), "{label}: {}", e.message);
        assert!(
            e.remedy.contains("'é'") && e.remedy.contains("u+00E9"),
            "{}",
            e.remedy
        );
    }
    let e = refused("u+212B");
    assert!(e.remedy.contains("'Å'"), "{}", e.remedy);
    // A singleton onto a base or a mark: a mark's key is written u+, as a
    // quote would hide it.
    let e = refused("u+212A");
    assert!(e.remedy.contains("'K'"), "{}", e.remedy);
    let e = refused("u+0341");
    assert!(e.remedy.contains("key it by u+0301:"), "{}", e.remedy);
    // From rev-cli2's review: a pair spelled with a singleton is still a
    // decomposed letter, as NFC composes it (KELVIN SIGN and cedilla is Ķ).
    for (label, letter) in [("u+212A, u+0327", "'Ķ'"), ("'A', u+0341", "'Á'")] {
        let e = refused(label);
        assert!(e.message.contains("decomposed"), "{label}: {}", e.message);
        assert!(e.remedy.contains(letter), "{label}: {}", e.remedy);
    }
    // A cluster the table doesn't know is refused as several characters.
    assert!(refused("'ff'").message.contains("several characters"));
    assert!(
        refused("u+0078, u+0301")
            .message
            .contains("several characters")
    );
}

#[test]
fn labels_outside_the_subset_are_refused_with_a_remedy() {
    assert!(refused("0x41").message.contains("codepoint label"));
    assert!(refused("65").message.contains("codepoint label"));
    assert!(refused("space").message.contains("unquoted"));
    assert!(refused("A").message.contains("unquoted"));
    assert!(refused("").message.contains("no key"));
    assert!(refused("\"a\"").remedy.contains("'a':"));
    assert!(
        refused("\"two words\"")
            .message
            .contains("isn't a glyph name")
    );
    assert!(refused("\"9lives\"").message.contains("isn't a glyph name"));
    assert!(refused("u+000A").message.contains("control character"));
    assert!(
        refused("u+D800")
            .message
            .contains("isn't a Unicode character")
    );
    assert!(refused("'A").message.contains("doesn't close"));
    assert!(refused("\"heart").message.contains("doesn't close"));
}

#[test]
fn a_key_writes_a_label_that_reads_back_and_a_charmap_string() {
    for key in [
        Key::Char('A'),
        Key::Char(' '),
        Key::Char('\''),
        Key::Char('\u{0301}'),
        Key::Char('é'),
        Key::Name("heart".into()),
    ] {
        assert_eq!(Key::from_label(&key.label()), Ok(key.clone()), "{key:?}");
        assert_eq!(Key::from_wire(&key.text()), Ok(key.clone()));
    }
    assert_eq!(Key::Char(' ').label(), "u+0020");
    assert_eq!(Key::Char('A').label(), "'A'");
    assert_eq!(Key::Name("heart".into()).charmap(), "<heart>");
    assert_eq!(Key::Char('é').charmap(), "é");
    assert!(Key::from_wire("").is_err());
}

#[test]
fn a_decomposed_voiced_kana_key_is_refused_naming_its_precomposed_form() {
    for (label, letter, code) in [
        ("'か', u+3099", "'が'", "u+304C"),
        ("u+30DB, u+309A", "'ポ'", "u+30DD"),
    ] {
        let e = refused(label);
        assert!(e.message.contains("decomposed"), "{label}: {}", e.message);
        assert!(
            e.remedy.contains(letter) && e.remedy.contains(code),
            "{}",
            e.remedy
        );
    }
    // The precomposed letters and the marks alone are still keys.
    assert_eq!(Key::from_label("'が'"), Ok(Key::Char('が')));
    assert_eq!(Key::from_label("u+3099"), Ok(Key::Char('\u{3099}')));
}

#[test]
fn a_kana_voicing_mark_is_written_by_its_code_point() {
    for mark in ['\u{3099}', '\u{309A}'] {
        let key = Key::Char(mark);
        assert_eq!(key.label(), format!("u+{:04X}", u32::from(mark)));
        assert_eq!(key.to_string(), format!("u+{:04X}", u32::from(mark)));
        assert_eq!(Key::from_label(&key.label()), Ok(key.clone()));
    }
    // The spacing voicing mark doesn't draw over its neighbor.
    assert_eq!(Key::Char('\u{309B}').label(), "'\u{309B}'");
}
