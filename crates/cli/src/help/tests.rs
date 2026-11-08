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

/// A verb's CLASS line is its class's, so a class that says "needs a
/// session" holds only verbs that take --session (`gaps` and `sessions`
/// read the whole project).
#[test]
fn a_class_that_needs_a_session_holds_only_verbs_that_take_one() {
    for spec in VERBS {
        if spec.class.preconditions().contains("needs a session") {
            assert!(
                spec.flag("session").is_some(),
                "`gbcli {}` takes no --session, but its class says it needs one",
                spec.name
            );
        }
    }
}

/// A session runs the ROM file at its path, so a rebuild over that file
/// moves the baseline too: the optimize loop builds each try to another
/// file, measures it in its own session and compares it with `equiv`
/// against the baseline session (leaf k22J86's agent eval).
#[test]
fn the_optimize_topic_keeps_the_baseline_and_builds_what_it_compares() {
    let text = Topic::from_name("optimize").unwrap().text();
    for needle in [
        "gbcli build --out build/fast.gb",
        "gbcli run build/fast.gb --session fast",
        "build/fast.sym",
        "gbcli equiv build/fast.gb --session base",
        "--keys",
    ] {
        assert!(text.contains(needle), "{needle} missing:\n{text}");
    }
    assert!(!text.contains("equiv fast.gb"), "a fast.gb nobody built");
    assert!(
        !text.contains("Rebuild with"),
        "a rebuild over the baseline"
    );
}

/// The recipes' optimize line builds the ROM it compares, too.
#[test]
fn the_recipes_compare_a_build_they_made() {
    assert!(!RECIPES.contains("equiv fast.gb"), "{RECIPES}");
    assert!(
        RECIPES.contains("gbcli build --out build/fast.gb"),
        "{RECIPES}"
    );
    assert!(RECIPES.contains("gbcli equiv build/fast.gb"), "{RECIPES}");
}

/// `--ram` shows its two forms worked: a hex address with a length, and a
/// symbol.
#[test]
fn equiv_ram_shows_an_address_with_a_length_and_a_symbol() {
    let text = verb(crate::verbs::by_name("equiv").unwrap());
    assert!(text.contains("--ram '$C0A0,4'"), "{text}");
    assert!(text.contains("--ram wPlayerX"), "{text}");
}

/// The log starts at the run's start state: a sound engine's setup writes,
/// made before it, aren't in it.
#[test]
fn the_audio_log_says_it_holds_only_this_runs_writes() {
    let text = verb(crate::verbs::by_name("run").unwrap());
    assert!(text.contains("writes made before the run"), "{text}");
}

#[test]
fn measure_help_offers_the_key_timeline() {
    let text = verb(crate::verbs::by_name("measure").unwrap());
    assert!(text.contains("--keys TOKENS"), "{text}");
    assert!(text.contains("--keys-file FILE"), "{text}");
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

/// A short clip's audio views are small at their native size; --scale
/// reaches them, and the help says so.
#[test]
fn the_scale_flag_names_the_run_images_it_scales() {
    let text = verb(VERBS.iter().find(|s| s.name == "run").unwrap());
    assert!(
        text.contains("--filmstrip, --piano and --spectrogram"),
        "{text}"
    );
}
