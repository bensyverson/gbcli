//! The verb table: every verb's class, flags, arity, summary and worked
//! example. Parsing, the primer, `--help` and near-miss suggestions all read
//! it, so a new verb is one entry here plus its builder in `build`.

mod flags;
mod table;
mod table_read;

pub use flags::{OUTPUT, SESSION, SHOT_OPTIONS};
pub use table::VERBS;

/// What a verb does, and so what it needs before it can run (design §12).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Read,
    Act,
    Configure,
    Server,
    Help,
}

impl Class {
    /// Every class, in the order the primer lists them.
    pub const ALL: [Class; 5] = [
        Class::Act,
        Class::Read,
        Class::Configure,
        Class::Server,
        Class::Help,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Class::Read => "read",
            Class::Act => "act",
            Class::Configure => "configure",
            Class::Server => "server",
            Class::Help => "help",
        }
    }

    /// What the class needs, as `--help` states it.
    pub fn preconditions(self) -> &'static str {
        match self {
            Class::Read => "never changes the head; needs a session",
            Class::Act => {
                "advances or changes the machine and saves the new head; needs a session \
                 (name one with a ROM to create it)"
            }
            Class::Configure => "changes the session's settings, not the machine; needs a session",
            Class::Server => "speaks the JSON Lines protocol; a session is optional",
            Class::Help => "no preconditions: works with no session, no ROM and no network",
        }
    }
}

/// Every verb gbcli has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verb {
    Run,
    Step,
    Checkpoint,
    Restore,
    Undo,
    Write,
    Poke,
    Patch,
    Show,
    Read,
    Shot,
    View,
    Sym,
    Checkpoints,
    Events,
    Gaps,
    Sessions,
    Prune,
    Rm,
    Break,
    Watch,
    Delete,
    Serve,
    Batch,
    Help,
    Recipes,
    Schema,
    Version,
}

/// One flag. `value` names its argument; `None` makes it a switch.
#[derive(Debug, PartialEq, Eq)]
pub struct FlagSpec {
    pub long: &'static str,
    pub short: Option<char>,
    pub value: Option<&'static str>,
    /// May be given more than once; order is kept.
    pub repeat: bool,
    pub help: &'static str,
}

/// How many positional arguments a verb takes.
#[derive(Clone, Copy, Debug)]
pub struct Arity {
    pub min: usize,
    /// `None`: as many as given.
    pub max: Option<usize>,
    /// How usage names them, for "needs …" errors.
    pub name: &'static str,
}

/// A verb's entry in the table.
#[derive(Debug)]
pub struct VerbSpec {
    pub verb: Verb,
    /// As typed: `run`, or `sessions rm`.
    pub name: &'static str,
    pub class: Class,
    /// One line for the primer.
    pub summary: &'static str,
    /// The argument shape after `gbcli <name>`.
    pub usage: &'static str,
    pub arity: Arity,
    pub flags: &'static [FlagSpec],
    /// Shared flag groups the verb also takes.
    pub groups: &'static [&'static [FlagSpec]],
    /// A paragraph for `--help`.
    pub about: &'static str,
    /// One worked example and what it shows.
    pub example: (&'static str, &'static str),
}

impl VerbSpec {
    /// Every flag the verb accepts: its own, then its groups'.
    pub fn all_flags(&self) -> impl Iterator<Item = &'static FlagSpec> + '_ {
        self.flags
            .iter()
            .chain(self.groups.iter().flat_map(|group| group.iter()))
    }

    pub fn flag(&self, long: &str) -> Option<&'static FlagSpec> {
        self.all_flags().find(|f| f.long == long)
    }

    pub fn short(&self, c: char) -> Option<&'static FlagSpec> {
        self.all_flags().find(|f| f.short == Some(c))
    }
}

/// The verb typed as `name` (`run`, `sessions rm`).
pub fn by_name(name: &str) -> Option<&'static VerbSpec> {
    VERBS.iter().find(|s| s.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_verb_has_exactly_one_entry_and_a_runnable_example() {
        for spec in VERBS {
            assert_eq!(
                VERBS.iter().filter(|s| s.verb == spec.verb).count(),
                1,
                "{:?}",
                spec.verb
            );
            let example = spec.example.0;
            assert!(
                example.starts_with(&format!("gbcli {}", spec.name)),
                "{}: {example}",
                spec.name
            );
            assert!(!spec.summary.is_empty() && !spec.about.is_empty());
        }
    }

    #[test]
    fn no_verb_takes_one_flag_name_twice() {
        for spec in VERBS {
            let names: Vec<_> = spec.all_flags().map(|f| f.long).collect();
            for name in &names {
                assert_eq!(
                    names.iter().filter(|n| *n == name).count(),
                    1,
                    "{} --{name}",
                    spec.name
                );
            }
        }
    }

    #[test]
    fn a_flag_name_means_one_thing_on_every_verb() {
        let all: Vec<&FlagSpec> = VERBS.iter().flat_map(|s| s.all_flags()).collect();
        for a in &all {
            for b in &all {
                if a.long == b.long {
                    assert_eq!(a.value, b.value, "--{} takes one kind of value", a.long);
                    assert_eq!(a.short, b.short, "--{} has one short form", a.long);
                }
            }
        }
    }
}
