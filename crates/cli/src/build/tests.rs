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

fn command(line: &[&str]) -> Result<Command, Usage> {
    let args = line.iter().map(OsString::from).collect();
    match parse(args)? {
        Parsed::Verb(inv) => build(inv, &env()),
        other => panic!("{other:?}"),
    }
}

fn request(line: &[&str]) -> (Target, Op) {
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
    // A topic named like a verb (or `recipes`, `schema`) is shadowed: `help
    // WORD` resolves those first.
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
    assert_eq!(u.message, "--scale only applies with --shot");
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
        "`sprites` isn't tiles, map or oam"
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
