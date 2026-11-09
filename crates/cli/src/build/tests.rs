use std::ffi::OsString;
use std::path::PathBuf;

use gbcli_protocol::{Start, TimeBase, WriteArg};

use super::*;
use crate::argv::{Parsed, parse};

fn env() -> Env {
    Env {
        session: Some("envsession".into()),
        actor: Some("envactor".into()),
        gbcli_dir: None,
        cwd: PathBuf::from("/work"),
        path: None,
    }
}

pub(super) fn command(line: &[&str]) -> Result<Command, Usage> {
    let args = line.iter().map(OsString::from).collect();
    match parse(args)? {
        Parsed::Verb(inv) => build(inv, &env()),
        other => panic!("{other:?}"),
    }
}

pub(super) fn request(line: &[&str]) -> (Target, Op) {
    match command(line) {
        Ok(Command::Request { target, op }) => (target, *op),
        other => panic!("{line:?}: {other:?}"),
    }
}

#[test]
fn run_applies_writes_in_the_order_given_across_kinds() {
    let (target, op) = request(&[
        "run",
        "demo.gb",
        "--poke",
        "wA=1",
        "--write",
        "rLCDC=$91",
        "--patch",
        "01:4000=0",
        "--frames",
        "2",
        "--read",
        "[wA]",
        "--read",
        "pc",
        "--from",
        "power-on",
        "--since",
        "f30",
    ]);
    let Op::Run(args) = op else { panic!() };
    let kinds: Vec<WriteKind> = args.writes.iter().map(|w: &WriteArg| w.kind).collect();
    assert_eq!(
        kinds,
        vec![WriteKind::Poke, WriteKind::Bus, WriteKind::Patch]
    );
    assert_eq!(args.frames, Some(2));
    assert_eq!(args.read, vec!["[wA]", "pc"]);
    assert_eq!(args.from, Some(Start::PowerOn));
    assert_eq!(args.since, Some(TimeBase::Checkpoint("f30".into())));
    let create = target.create.expect("the ROM argument creates");
    assert_eq!(create.rom, PathBuf::from("/work/demo.gb"));
    assert_eq!(create.model, None, "not given: the default, and unchecked");
}

#[test]
fn run_takes_expectations_in_order_and_a_keys_base() {
    let (_, op) = request(&[
        "run",
        "--expect",
        "[wA] == 1",
        "--read",
        "pc",
        "--expect",
        "u16[wB]",
        "--keys",
        "A@3",
        "--keys-since",
        "boot",
    ]);
    let Op::Run(args) = op else { panic!() };
    assert_eq!(args.expect, vec!["[wA] == 1", "u16[wB]"]);
    assert_eq!(args.read, vec!["pc"]);
    assert_eq!(args.keys_since, Some(TimeBase::Boot));
    for (word, base) in [
        ("power-on", TimeBase::PowerOn),
        ("floor5", TimeBase::Checkpoint("floor5".into())),
    ] {
        let (_, op) = request(&["run", "--keys", "A@3", "--keys-since", word]);
        let Op::Run(args) = op else { panic!() };
        assert_eq!(args.keys_since, Some(base));
    }
    let (_, op) = request(&["run", "--keys", "A@3"]);
    let Op::Run(args) = op else { panic!() };
    assert_eq!(args.keys_since, None, "the run's start");
}

#[test]
fn keys_since_without_keys_is_refused() {
    let alone = command(&["run", "--keys-since", "boot"]).unwrap_err();
    assert_eq!(
        alone.message, "--keys-since only applies with --keys or --keys-file",
        "{alone:?}"
    );
    let with_file = command(&["run", "--keys-file", "/nonexistent", "--keys-since", "boot"]);
    assert!(
        !with_file.is_err_and(|u| u.message.contains("--keys-since")),
        "a keys file is keys too"
    );
}

/// A second expression after `--read` lands where the ROM goes; the error
/// said the session runs another ROM (review item E-3).
#[test]
fn an_expression_taken_as_the_rom_is_refused_with_the_repeat_remedy() {
    for stray in ["[wMiss]", "u16[wNow]", "s8[wX]", "[wA] == 3"] {
        let refused = command(&["run", "--read", "u16[wNow]", stray]).unwrap_err();
        assert!(refused.message.contains(stray), "{stray}: {refused:?}");
        assert!(
            refused.remedy.contains("repeat --read"),
            "{stray}: {refused:?}"
        );
    }
    // What must still work: ROM paths, a missing one with a ROM's
    // extension (its own error comes later), and every expression repeated.
    for rom in ["demo.gb", "build/rhythm.gb", "Tetris [!].gb", "game.gbc"] {
        let (target, _) = request(&["run", rom, "--read", "[wA]"]);
        assert!(target.create.is_some(), "{rom}");
    }
    let (_, op) = request(&["run", "--read", "u16[wNow]", "--read", "[wMiss]"]);
    let Op::Run(args) = op else { panic!() };
    assert_eq!(args.read, vec!["u16[wNow]", "[wMiss]"]);
}

#[test]
fn an_existing_rom_named_like_an_expression_still_runs() {
    let dir = std::env::temp_dir().join(format!("gbcli-rom-named-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("[odd].gb"), b"").unwrap();
    std::fs::write(dir.join("[odd]"), b"").unwrap();
    let env = Env {
        cwd: dir.clone(),
        ..env()
    };
    for rom in ["[odd].gb", "[odd]"] {
        let args = ["run", rom].iter().map(OsString::from).collect();
        let Parsed::Verb(inv) = parse(args).unwrap() else {
            panic!()
        };
        let built = build(inv, &env);
        assert!(built.is_ok(), "{rom}: {built:?}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_environment_supplies_session_and_actor_and_flags_win() {
    let (target, _) = request(&["show"]);
    assert_eq!(target.session.as_deref(), Some("envsession"));
    assert_eq!(target.actor.as_deref(), Some("envactor"));
    let (target, _) = request(&["step", "--session", "dx", "--as", "me", "--rev", "s3"]);
    assert_eq!(target.session.as_deref(), Some("dx"));
    assert_eq!(target.actor.as_deref(), Some("me"));
    assert_eq!(target.rev, Some(Rev(3)));
    let request = target.request(Op::Show);
    assert_eq!(request.session.as_deref(), Some("dx"));
    assert_eq!(request.rev, Some(Rev(3)));
}

#[test]
fn the_three_write_verbs_are_one_op_with_three_kinds() {
    for (verb, kind) in [
        ("write", WriteKind::Bus),
        ("poke", WriteKind::Poke),
        ("patch", WriteKind::Patch),
    ] {
        let (_, op) = request(&[verb, "$C000=1", "$C001=2,3"]);
        let Op::Write(args) = op else { panic!() };
        assert_eq!(args.writes.len(), 2);
        assert!(args.writes.iter().all(|w| w.kind == kind));
        assert_eq!(args.writes[1].bytes, vec![2, 3]);
    }
}

#[test]
fn each_verb_builds_its_op() {
    assert!(matches!(
        request(&["step", "over"]).1,
        Op::Step(StepArgs {
            unit: StepKind::Over
        })
    ));
    assert!(matches!(
        request(&["step"]).1,
        Op::Step(StepArgs {
            unit: StepKind::Instruction
        })
    ));
    let Op::Watch(w) = request(&["watch", "write", "rDMA", "--once", "--if", "new > 0"]).1 else {
        panic!()
    };
    assert_eq!(
        (w.access, w.at.as_str(), w.once),
        (WatchAccess::Write, "rDMA", true)
    );
    assert_eq!(w.condition.as_deref(), Some("new > 0"));
    assert!(matches!(
        request(&["delete", "3"]).1,
        Op::Delete(DeleteArgs { id: 3 })
    ));
    assert!(matches!(
        request(&["sessions", "rm", "dx", "--with-checkpoints"]).1,
        Op::Rm(RmArgs {
            with_checkpoints: true,
            ..
        })
    ));
    assert!(matches!(
        request(&["events", "--limit", "2"]).1,
        Op::Events(EventsArgs { limit: Some(2) })
    ));
    assert!(matches!(request(&["undo"]).1, Op::Undo));
    assert!(matches!(request(&["gaps"]).1, Op::Gaps));
}

#[test]
fn creation_flags_without_a_rom_are_refused_not_ignored() {
    let u = command(&["show", "--model", "cgb-e"]).unwrap_err();
    assert_eq!(
        u.message,
        "--model only applies when a ROM creates a session"
    );
    let u = command(&["run", "a.gb", "--rom", "b.gb"]).unwrap_err();
    assert!(u.message.contains("given twice"), "{}", u.message);
}

#[test]
fn bad_values_name_the_flag() {
    assert_eq!(
        command(&["watch", "poke", "wA"]).unwrap_err().message,
        "`poke` isn't read, write or access"
    );
    assert_eq!(
        command(&["delete", "one"]).unwrap_err().message,
        "`one` isn't a breakpoint or watch id"
    );
    assert_eq!(
        command(&["step", "--rev", "6"]).unwrap_err().message,
        "--rev: a revision looks like s6, not \"6\""
    );
    assert!(
        command(&["sessions", "prune"])
            .unwrap_err()
            .message
            .contains("--unused-for")
    );
    assert!(command(&["batch"]).unwrap_err().message.contains("-F FILE"));
}

#[test]
fn help_routes_topics_verbs_recipes_and_schema() {
    assert!(matches!(command(&["help"]), Ok(Command::Primer)));
    assert!(matches!(command(&["help", "run"]), Ok(Command::VerbHelp(s)) if s.verb == Verb::Run));
    assert!(matches!(command(&["help", "schema"]), Ok(Command::Schema)));
    assert!(matches!(
        command(&["help", "recipes"]),
        Ok(Command::Recipes)
    ));
    assert!(matches!(
        command(&["help", "time"]),
        Ok(Command::Topic(Topic::Time))
    ));
    let u = command(&["help", "adresses"]).unwrap_err();
    assert_eq!(u.message, "no help topic `adresses`");
    assert!(
        u.remedy.starts_with("did you mean `gbcli help addresses`?"),
        "{}",
        u.remedy
    );
}

#[test]
fn every_help_topic_is_reachable_by_gbcli_help() {
    // A topic named `recipes` or `schema` would be shadowed: `help WORD`
    // resolves those first. A topic named like a verb wins over the verb.
    for topic in Topic::ALL {
        let routed = command(&["help", topic.name()]);
        assert!(
            matches!(routed, Ok(Command::Topic(t)) if t == topic),
            "`gbcli help {}` doesn't print the topic",
            topic.name()
        );
    }
}

#[test]
fn the_help_verb_names_every_topic() {
    let about = by_name("help").unwrap().about;
    for topic in Topic::ALL {
        assert!(about.contains(topic.name()), "{} missing", topic.name());
    }
}

#[test]
fn run_takes_a_shot_with_its_options_and_refuses_them_without_one() {
    let Op::Run(args) = request(&[
        "run",
        "--frames",
        "3",
        "--shot",
        "a.png",
        "--scale",
        "2",
        "--crop",
        "0,8,16,24",
        "--live",
        "--any-frame",
    ])
    .1
    else {
        panic!()
    };
    let shot = args.shot.expect("a shot");
    assert_eq!(shot.path, "a.png");
    assert_eq!(shot.scale, Some(2));
    assert_eq!(
        shot.crop,
        Some(gbcli_protocol::Crop {
            x: 0,
            y: 8,
            width: 16,
            height: 24
        })
    );
    assert!(shot.live && shot.any_frame);
    let Op::Run(args) = request(&["run", "--shot", "b.png"]).1 else {
        panic!()
    };
    assert_eq!(
        args.shot.unwrap().scale,
        None,
        "the server owns the default"
    );
    let u = command(&["run", "--scale", "2"]).unwrap_err();
    assert_eq!(
        u.message,
        "--scale only applies with --shot, --filmstrip, --piano or --spectrogram"
    );
    let u = command(&["run", "--shot", "a.png", "--crop", "1,2,3"]).unwrap_err();
    assert_eq!(u.message, "--crop takes X,Y,W,H, not `1,2,3`");
}

#[test]
fn keys_and_a_keys_file_concatenate_and_a_bad_token_is_a_usage_error() {
    let file = std::env::temp_dir().join(format!("gbcli-keys-{}.txt", std::process::id()));
    std::fs::write(&file, "START@10 # menu\n-START@12\n").unwrap();
    let path = file.display().to_string();
    let Op::Run(args) = request(&["run", "--keys", "A@0 -A@2", "--keys-file", &path]).1 else {
        panic!()
    };
    let text: Vec<String> = args.keys.iter().map(|k| k.to_string()).collect();
    assert_eq!(text, vec!["A@0", "-A@2", "START@10", "-START@12"]);
    std::fs::remove_file(&file).unwrap();
    let u = command(&["run", "--keys", "Q@1"]).unwrap_err();
    assert!(u.message.contains("`Q@1`"), "{}", u.message);
    assert!(u.remedy.contains("A@0"), "{}", u.remedy);
    let u = command(&["run", "--keys-file", "/nonexistent/keys.txt"]).unwrap_err();
    assert!(
        u.message.starts_with("can't read /nonexistent/keys.txt"),
        "{}",
        u.message
    );
}

#[test]
fn the_read_verbs_for_shots_views_symbols_and_checkpoints_build_their_ops() {
    let Op::Shot(args) = request(&[
        "shot",
        "s.png",
        "--scale",
        "1",
        "--any-frame",
        "--since",
        "power-on",
    ])
    .1
    else {
        panic!()
    };
    assert_eq!(args.shot.path, "s.png");
    assert_eq!(args.shot.scale, Some(1));
    assert!(args.shot.any_frame);
    assert_eq!(args.since, Some(TimeBase::PowerOn));
    let Op::View(args) = request(&[
        "view", "map", "--map", "9c00", "--png", "m.png", "--scale", "2",
    ])
    .1
    else {
        panic!()
    };
    assert_eq!(args.what, gbcli_protocol::ViewKind::Map);
    assert_eq!(args.map, Some(gbcli_protocol::MapSel::Map9C00));
    let png = args.png.unwrap();
    assert_eq!((png.path.as_str(), png.scale), ("m.png", Some(2)));
    let Op::View(args) = request(&["view", "oam"]).1 else {
        panic!()
    };
    assert_eq!((args.map, args.png), (None, None));
    assert_eq!(
        command(&["view", "sprites"]).unwrap_err().message,
        "`sprites` isn't tiles, map, oam, palettes or memory"
    );
    assert_eq!(
        command(&["view", "tiles", "--scale", "2"])
            .unwrap_err()
            .message,
        "--scale only applies with --png"
    );
    let Op::Sym(args) = request(&["sym", "Main*"]).1 else {
        panic!()
    };
    assert_eq!(args.pattern, "Main*");
    assert!(matches!(request(&["checkpoints"]).1, Op::Checkpoints));
}

fn run_audio(line: &[&str]) -> Option<Box<gbcli_protocol::AudioArgs>> {
    match request(line).1 {
        Op::Run(args) => args.audio,
        other => panic!("{other:?}"),
    }
}

#[test]
fn run_collects_audio_only_when_asked() {
    assert_eq!(run_audio(&["run", "--frames", "2"]), None);
    let audio = run_audio(&["run", "--audio-log", "--piano", "p.png"]).unwrap();
    assert!(audio.log && audio.wav.is_none());
    assert_eq!(audio.piano.unwrap().path, "p.png");
}

#[test]
fn run_audio_filters_by_channel_by_number_or_name_and_summarizes() {
    use gbcli_protocol::AudioChannel;
    let audio = run_audio(&["run", "--audio-summary"]).unwrap();
    assert!(audio.summary && !audio.log && audio.channels.is_empty());
    let audio = run_audio(&[
        "run",
        "--audio-log",
        "--audio-channel",
        "4",
        "--audio-channel",
        "square2",
    ])
    .unwrap();
    assert_eq!(
        audio.channels,
        vec![AudioChannel::Noise, AudioChannel::Square2]
    );
    for (word, channel) in [
        ("1", AudioChannel::Square1),
        ("2", AudioChannel::Square2),
        ("3", AudioChannel::Wave),
        ("wave", AudioChannel::Wave),
        ("noise", AudioChannel::Noise),
    ] {
        let audio = run_audio(&["run", "--audio-log", "--audio-channel", word]).unwrap();
        assert_eq!(audio.channels, vec![channel], "{word}");
    }
    let bad = command(&["run", "--audio-log", "--audio-channel", "5"]).unwrap_err();
    assert!(bad.message.contains("`5`"), "{bad:?}");
    assert!(bad.remedy.contains("noise"), "{bad:?}");
}

#[test]
fn run_oam_log_is_a_switch() {
    let oam_log = |line: &[&str]| match request(line).1 {
        Op::Run(args) => args.oam_log,
        other => panic!("{other:?}"),
    };
    assert!(oam_log(&["run", "--frames", "3", "--oam-log"]));
    assert!(!oam_log(&["run", "--frames", "3"]));
}

#[test]
fn view_memory_takes_a_start_a_length_and_its_own_flags() {
    let Op::View(args) = request(&[
        "view",
        "memory",
        "wMap+$10",
        "16",
        "--out",
        "m.bin",
        "--allow-uninit",
    ])
    .1
    else {
        panic!()
    };
    assert_eq!(args.what, gbcli_protocol::ViewKind::Memory);
    assert_eq!(args.addr.as_deref(), Some("wMap+$10"));
    assert_eq!(args.len, Some(16));
    assert_eq!(args.out.as_deref(), Some("m.bin"));
    assert!(args.allow_uninit);
    let Op::View(args) = request(&["view", "memory", "$C000", "$400"]).1 else {
        panic!()
    };
    assert_eq!(
        (args.len, args.out, args.allow_uninit),
        (Some(0x400), None, false)
    );
    let png = command(&["view", "memory", "wMap", "4", "--png", "m.png"]).unwrap_err();
    assert!(png.remedy.contains("--out"), "{png:?}");

    let missing = command(&["view", "memory", "wMap"]).unwrap_err();
    assert!(missing.message.contains("LEN"), "{missing:?}");
    let bad_len = command(&["view", "memory", "wMap", "lots"]).unwrap_err();
    assert!(bad_len.message.contains("`lots`"), "{bad_len:?}");
    let extra = command(&["view", "tiles", "wMap"]).unwrap_err();
    assert!(extra.message.contains("`wMap`"), "{extra:?}");
    assert!(extra.remedy.contains("view memory"), "{extra:?}");
    let out = command(&["view", "oam", "--out", "o.bin"]).unwrap_err();
    assert!(out.message.contains("--out"), "{out:?}");
    let uninit = command(&["view", "oam", "--allow-uninit"]).unwrap_err();
    assert!(uninit.message.contains("--allow-uninit"), "{uninit:?}");
    // The VRAM views still take theirs.
    assert!(matches!(
        request(&["view", "map", "--map", "window"]).1,
        Op::View(_)
    ));
}

#[test]
fn view_palettes_is_a_view() {
    let Op::View(args) = request(&["view", "palettes"]).1 else {
        panic!()
    };
    assert_eq!(args.what, gbcli_protocol::ViewKind::Palettes);
}

#[test]
fn run_wav_takes_its_channels_and_refuses_them_alone() {
    use gbcli_protocol::WavChannels;
    let audio = run_audio(&["run", "--wav", "m.wav"]).unwrap();
    assert_eq!(audio.wav.unwrap().channels, WavChannels::Mixed);
    let audio = run_audio(&["run", "--wav", "m.wav", "--channels", "each"]).unwrap();
    assert_eq!(audio.wav.unwrap().channels, WavChannels::Each);
    let alone = command(&["run", "--channels", "each"]).unwrap_err();
    assert!(alone.message.contains("--wav"), "{alone:?}");
    let bad = command(&["run", "--wav", "m.wav", "--channels", "all"]).unwrap_err();
    assert!(bad.remedy.contains("each"), "{bad:?}");
}

#[test]
fn run_filmstrip_takes_every_and_refuses_it_alone() {
    let strip = |line: &[&str]| match request(line).1 {
        Op::Run(args) => args.filmstrip,
        other => panic!("{other:?}"),
    };
    let made = strip(&["run", "--filmstrip", "f.png", "--every", "5"]).unwrap();
    assert_eq!((made.path.as_str(), made.every), ("f.png", 5));
    assert_eq!(strip(&["run", "--filmstrip", "f.png"]).unwrap().every, 1);
    let alone = command(&["run", "--every", "5"]).unwrap_err();
    assert!(alone.message.contains("--filmstrip"), "{alone:?}");
}
