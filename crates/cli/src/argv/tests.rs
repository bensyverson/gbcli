use super::*;
use crate::verbs::Verb;

fn args(line: &[&str]) -> Vec<OsString> {
    line.iter().map(OsString::from).collect()
}

fn invocation(line: &[&str]) -> Invocation {
    match parse(args(line)) {
        Ok(Parsed::Verb(invocation)) => invocation,
        other => panic!("{line:?}: {other:?}"),
    }
}

fn usage(line: &[&str]) -> Usage {
    match parse(args(line)) {
        Err(usage) => usage,
        other => panic!("{line:?} parsed: {other:?}"),
    }
}

#[test]
fn bare_and_dash_help_are_the_primer() {
    assert!(matches!(parse(args(&[])), Ok(Parsed::Primer)));
    assert!(matches!(parse(args(&["--help"])), Ok(Parsed::Primer)));
}

#[test]
fn help_anywhere_before_dashes_is_the_verbs_help_even_with_bad_flags() {
    let Ok(Parsed::VerbHelp(spec)) = parse(args(&["run", "--frame", "3", "-h"])) else {
        panic!()
    };
    assert_eq!(spec.verb, Verb::Run);
}

#[test]
fn flags_keep_their_order_and_values() {
    let inv = invocation(&[
        "run",
        "demo.gb",
        "--poke",
        "wA=1",
        "--write",
        "rLCDC=$91",
        "--poke",
        "wB=2",
        "-s",
        "dx",
        "--json",
    ]);
    assert_eq!(inv.spec.verb, Verb::Run);
    assert_eq!(inv.positionals, vec!["demo.gb"]);
    let order: Vec<_> = inv
        .flags
        .iter()
        .map(|(f, v)| (f.long, v.as_str()))
        .collect();
    assert_eq!(
        order,
        vec![
            ("poke", "wA=1"),
            ("write", "rLCDC=$91"),
            ("poke", "wB=2"),
            ("session", "dx"),
            ("json", ""),
        ]
    );
    assert_eq!(inv.one("session"), Some("dx"));
    assert!(inv.has("json"));
}

#[test]
fn equals_form_works() {
    let inv = invocation(&["run", "--frames=2"]);
    assert_eq!(inv.one("frames"), Some("2"));
}

#[test]
fn sessions_subverbs_are_two_words() {
    assert_eq!(invocation(&["sessions", "rm", "dx"]).spec.verb, Verb::Rm);
    assert_eq!(
        invocation(&["sessions", "prune", "--unused-for", "7d"])
            .spec
            .verb,
        Verb::Prune
    );
    assert_eq!(invocation(&["sessions"]).spec.verb, Verb::Sessions);
}

#[test]
fn an_unknown_command_names_near_misses_and_the_next_command() {
    let u = usage(&["rn"]);
    assert_eq!(u.message, "unknown command `rn`");
    assert_eq!(u.remedy, "did you mean `run`? `gbcli` lists the commands");
    let u = usage(&["rm"]);
    assert!(
        u.remedy.starts_with("did you mean `sessions rm`"),
        "{}",
        u.remedy
    );
    let u = usage(&["--frames"]);
    assert!(u.remedy.contains("the command comes first"), "{}", u.remedy);
}

#[test]
fn an_unknown_flag_names_near_misses_and_the_verbs_help() {
    let u = usage(&["run", "--frame", "2"]);
    assert_eq!(u.message, "`gbcli run` has no flag --frame");
    assert_eq!(
        u.remedy,
        "did you mean `--frames`? `gbcli run --help` lists its flags"
    );
    let u = usage(&["show", "-x"]);
    assert_eq!(u.message, "`gbcli show` has no flag -x");
}

#[test]
fn a_missing_value_a_repeat_and_the_wrong_arity_are_usage_errors() {
    assert_eq!(
        usage(&["run", "--frames"]).message,
        "--frames needs a value (N)"
    );
    assert_eq!(
        usage(&["run", "--frames", "1", "--frames", "2"]).message,
        "--frames is given twice"
    );
    let u = usage(&["break"]);
    assert_eq!(u.message, "`gbcli break` needs ADDR");
    assert_eq!(u.remedy, "usage: gbcli break ADDR [--if EXPR] [flags]");
    assert_eq!(
        usage(&["show", "extra"]).message,
        "`gbcli show` doesn't take `extra`"
    );
}

#[test]
fn a_switch_given_a_value_is_refused() {
    let u = usage(&["show", "--json=yes"]);
    assert!(u.remedy.contains("`gbcli show --help`"), "{}", u.remedy);
}
