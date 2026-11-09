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

/// hUGETracker names notes an octave above scientific pitch, so `E-5` in a
/// song logs as E4 (project/2026-11-08-dx-shooter.md).
#[test]
fn the_music_topic_says_song_notes_are_an_octave_above_scientific_pitch() {
    let text = crate::help::Topic::from_name("music").unwrap().text();
    assert!(text.contains("C-3 sounds as C2 (65 Hz)"), "{text}");
}

/// The audio log's filter and summary, and the OAM log, say what they hold.
#[test]
fn run_help_describes_the_audio_filter_and_the_oam_log() {
    let text = verb(crate::verbs::by_name("run").unwrap());
    assert!(text.contains("--audio-channel N"), "{text}");
    assert!(text.contains("NR50-NR52"), "{text}");
    assert!(text.contains("--oam-log"), "{text}");
    let view = verb(crate::verbs::by_name("view").unwrap());
    assert!(view.contains("palettes"), "{view}");
}

/// The music topic's listening line shows how to enlarge a short clip's
/// spectrogram, which is 64 columns a second at its default scale.
#[test]
fn the_music_topic_shows_scale_for_a_short_clips_spectrogram() {
    let text = crate::help::Topic::from_name("music").unwrap().text();
    assert!(text.contains("--spectrogram spec.png --scale 4"), "{text}");
}

/// run's example reads a variable's byte, not its address (a bare symbol is
/// its address).
#[test]
fn run_example_reads_a_variables_byte() {
    let text = verb(crate::verbs::by_name("run").unwrap());
    assert!(text.contains("--read '[wSpriteX]'"), "{text}");
}

/// A bare symbol is its address in the grammar; the help shows the three
/// forms that read what a variable holds.
#[test]
fn read_help_says_a_bare_symbol_is_its_address() {
    let text = verb(crate::verbs::by_name("read").unwrap());
    assert!(text.contains("A bare symbol is its address"), "{text}");
    for form in ["[wX]", "u8[wX]", "u16[wX]"] {
        assert!(text.contains(form), "{form}: {text}");
    }
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

/// The server draws the piano roll at 2 unless told otherwise, and the rest
/// at 1; the flag states both.
#[test]
fn the_scale_flag_states_each_run_images_default() {
    let text = verb(VERBS.iter().find(|s| s.name == "run").unwrap()).replace('\n', " ");
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(text.contains("default 2 for --piano"), "{text}");
    assert!(text.contains("1 for the others"), "{text}");
}

/// The words of a recipe's command line, as a shell would split them: single
/// quotes group, and a note in parentheses after the command is dropped.
fn shell_words(line: &str) -> Vec<String> {
    let line = line.split(" (").next().unwrap_or(line);
    let (mut words, mut word, mut quoted) = (Vec::new(), String::new(), false);
    for c in line.chars() {
        match c {
            '\'' => quoted = !quoted,
            ' ' if !quoted => {
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
            }
            c => word.push(c),
        }
    }
    assert!(!quoted, "unclosed quote: {line}");
    words.extend((!word.is_empty()).then_some(word));
    words
}

/// The HBlank check both example games invented (examples DX review, item
/// 1): the recipe exists, and each of its commands parses into a request.
#[test]
fn the_hblank_recipes_commands_parse() {
    let title = "Does every scroll or palette write land in HBlank?";
    let block = RECIPES
        .split_once(title)
        .unwrap_or_else(|| panic!("no recipe titled {title:?}"))
        .1;
    let commands: Vec<Vec<String>> = block
        .lines()
        .skip(1)
        .take_while(|l| l.starts_with("  "))
        .filter_map(|l| l.trim().strip_prefix("gbcli "))
        .map(shell_words)
        .collect();
    let verbs: Vec<&str> = commands.iter().map(|c| c[0].as_str()).collect();
    assert_eq!(verbs, ["run", "break", "run"], "{block}");
    assert!(commands[0].iter().any(|w| w == "--raster"), "{commands:?}");
    assert!(
        commands[1].iter().any(|w| w == "([rSTAT] & 3) != 0"),
        "{commands:?}"
    );
    // The run reads its keys file as it parses.
    let cwd = std::env::temp_dir().join(format!("gbcli-hblank-{}", std::process::id()));
    std::fs::create_dir_all(&cwd).unwrap();
    std::fs::write(cwd.join("play.txt"), "A@0 -A@2\n").unwrap();
    let env = crate::Env {
        // As the recipes say: --session on each, or GBCLI_SESSION.
        session: Some("dx".into()),
        actor: None,
        gbcli_dir: None,
        cwd: cwd.clone(),
        path: None,
    };
    for words in commands {
        let args = words.iter().map(std::ffi::OsString::from).collect();
        let parsed = crate::argv::parse(args).unwrap_or_else(|e| panic!("{words:?}: {e:?}"));
        let crate::argv::Parsed::Verb(inv) = parsed else {
            panic!("{words:?}: {parsed:?}")
        };
        let command = crate::build::build(inv, &env).unwrap_or_else(|e| panic!("{words:?}: {e:?}"));
        assert!(
            matches!(command, crate::build::Command::Request { .. }),
            "{words:?}: {command:?}"
        );
    }
    std::fs::remove_dir_all(&cwd).unwrap();
}
