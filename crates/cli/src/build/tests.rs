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
    assert_eq!(create.model, Model::DmgB);
}

#[test]
fn the_environment_supplies_session_and_actor_and_flags_win() {
    let (target, _) = request(&["show"]);
    assert_eq!(target.session.as_deref(), Some("envsession"));
    assert_eq!(target.actor.as_deref(), Some("envactor"));
    let (target, _) = request(&["show", "--session", "dx", "--as", "me", "--rev", "s3"]);
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
        command(&["show", "--rev", "6"]).unwrap_err().message,
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
