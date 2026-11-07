use super::*;

#[test]
fn the_primer_fits_in_40_lines_and_names_every_verb_and_topic() {
    let primer = primer();
    assert!(
        primer.lines().count() <= 40,
        "{} lines",
        primer.lines().count()
    );
    for spec in VERBS {
        assert!(primer.contains(spec.name), "{}", spec.name);
    }
    for topic in Topic::ALL {
        assert!(primer.contains(topic.name()), "{}", topic.name());
    }
    assert!(primer.lines().all(|l| l.chars().count() <= 80), "{primer}");
    assert!(
        primer.contains("gbcli run game.gb --session demo --frames 120 --shot first.png"),
        "design §12's first command"
    );
}

#[test]
fn verb_help_carries_the_class_flags_and_one_example() {
    let run = verb(crate::verbs::by_name("run").unwrap());
    assert!(run.starts_with("gbcli run — "), "{run}");
    assert!(run.contains("CLASS\n  act: "), "{run}");
    assert!(run.contains("  --frames N"), "{run}");
    assert!(run.contains("  -s, --session NAME"), "{run}");
    assert_eq!(run.matches("EXAMPLE").count(), 1);
    for spec in VERBS {
        let text = verb(spec);
        assert!(text.contains(spec.example.0), "{}", spec.name);
        assert!(text.contains(spec.class.name()), "{}", spec.name);
        for flag in spec.all_flags() {
            assert!(
                text.contains(&format!("--{}", flag.long)),
                "{} --{}",
                spec.name,
                flag.long
            );
        }
    }
}

#[test]
fn wrap_breaks_at_the_width() {
    assert_eq!(wrap("aa bb cc", 5), "aa bb\ncc");
}

#[test]
fn verb_help_heads_each_flag_group_by_name() {
    let run = VERBS.iter().find(|s| s.name == "run").unwrap();
    let text = verb(run);
    for heading in [
        "SHOT FLAGS (how the PNG is taken)",
        "SESSION FLAGS (every session verb)",
        "OUTPUT FLAGS (every verb that sends a request)",
    ] {
        assert!(text.contains(heading), "{heading} missing from:\n{text}");
    }
    assert!(!text.contains("MORE FLAGS"), "{text}");
}

#[test]
fn the_primer_says_json_prints_the_response_line() {
    assert!(!primer().contains("--json prints the protocol's result"));
}

#[test]
fn the_scale_flag_states_the_servers_range() {
    let text = verb(VERBS.iter().find(|s| s.name == "run").unwrap());
    assert!(text.contains("1 to 16"), "{text}");
}
