//! The help as a library caller reads it: no argv, no `Env`, no process.
//! Each accessor is checked against what the CLI prints for the same help,
//! so the two can't drift apart.

use gbcli_cli::help::{self, RECIPES, SCHEMA, Topic, Verb};
use gbcli_cli::{Env, Io};

/// What `gbcli ARGS` prints on stdout, run in-process.
fn cli(args: &[&str]) -> String {
    let mut stdin: &[u8] = &[];
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = gbcli_cli::run(
        args.iter().map(Into::into).collect(),
        &Env::default(),
        Io {
            stdin: &mut stdin,
            out: &mut out,
            err: &mut err,
        },
    );
    assert_eq!(code, 0, "gbcli {args:?}: {}", String::from_utf8_lossy(&err));
    String::from_utf8(out).unwrap()
}

#[test]
fn the_primer_is_what_bare_gbcli_prints() {
    let primer = help::primer();
    assert!(primer.starts_with("gbcli — "), "{primer}");
    assert_eq!(primer, cli(&[]));
}

#[test]
fn a_topic_is_found_by_its_typed_name_and_reads_as_gbcli_help_topic() {
    assert_eq!(Topic::from_name("time"), Some(Topic::Time));
    assert_eq!(Topic::from_name("no-such-topic"), None);
    assert!(Topic::Time.text().starts_with("Time — "));
    for topic in Topic::ALL {
        assert_eq!(topic.text(), cli(&["help", topic.name()]), "{topic:?}");
    }
}

#[test]
fn every_verb_has_help_that_reads_as_its_dash_dash_help() {
    assert_eq!(Verb::from_name("run"), Some(Verb::Run));
    assert_eq!(Verb::from_name("sessions rm"), Some(Verb::Rm));
    assert_eq!(Verb::from_name("no-such-verb"), None);
    assert_eq!(Verb::Rm.name(), "sessions rm");
    assert!(help::verb(Verb::Run).starts_with("gbcli run — "));
    let mut count = 0;
    for verb in Verb::all() {
        let name = verb.name();
        let mut args: Vec<&str> = name.split(' ').collect();
        args.push("--help");
        assert_eq!(help::verb(verb), cli(&args), "{name}");
        assert_eq!(Verb::from_name(name), Some(verb));
        count += 1;
    }
    assert!(count > 40, "only {count} verbs");
}

#[test]
fn recipes_and_the_schema_are_what_their_verbs_print() {
    assert!(RECIPES.contains("gbcli "), "{RECIPES}");
    assert_eq!(RECIPES, cli(&["recipes"]));
    assert!(SCHEMA.contains("\"op\""), "{SCHEMA}");
    assert_eq!(SCHEMA, cli(&["schema"]));
}
