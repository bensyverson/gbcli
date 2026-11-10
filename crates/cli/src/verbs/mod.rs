//! The verb table: every verb's class, flags, arity, summary and worked
//! example. Parsing, the primer, `--help` and near-miss suggestions all read
//! it, so a new verb is one entry here plus its builder in `build`.

mod flags;
mod table;
mod table_battery;
mod table_build;
mod table_font;
mod table_opcode;
mod table_pixels;
mod table_read;
mod table_song;
mod table_version;

pub use flags::{CHANGE, GUARD, OUTPUT, SESSION, SHOT_OPTIONS};
pub use table::VERBS;

/// What a verb does, and so what it needs before it can run (design §12).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Read,
    Act,
    Configure,
    /// The project's `.gbcli/` as a whole: its sessions and gap ledger.
    Project,
    Build,
    Server,
    Help,
}

impl Class {
    /// Every class, in the order the primer lists them.
    pub const ALL: [Class; 7] = [
        Class::Act,
        Class::Read,
        Class::Configure,
        Class::Project,
        Class::Build,
        Class::Server,
        Class::Help,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Class::Read => "read",
            Class::Act => "act",
            Class::Configure => "configure",
            Class::Project => "project",
            Class::Build => "build",
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
            Class::Project => {
                "lists or removes the sessions in this project's .gbcli/, or reads what they \
                 share; takes no session"
            }
            Class::Build => {
                "runs RGBDS from PATH, checked against the pinned version (`gbcli doctor`); \
                 a session only to reload the build into; `new`, `song`, `pixels`, the `font` \
                 verbs and `assets` need none of it (gbcli encodes images itself)"
            }
            Class::Server => "speaks the JSON Lines protocol; a session is optional",
            Class::Help => "no preconditions: works with no session, no ROM and no network",
        }
    }
}

/// Every verb gbcli has. Public as [`crate::help::Verb`], the typed name
/// a library caller asks for help by.
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
    Measure,
    Equiv,
    Events,
    Gaps,
    Sessions,
    Prune,
    Rm,
    Break,
    Watch,
    Delete,
    New,
    Build,
    Assets,
    Song,
    Pixels,
    Font,
    FontEdit,
    FontSave,
    FontImport,
    FontExport,
    FontNew,
    FontDiff,
    Serve,
    Batch,
    Help,
    Recipes,
    Schema,
    Doctor,
    Opcode,
    Version,
    PowerCycle,
    Sav,
}

impl Verb {
    /// Every verb, in the verb table's order.
    pub fn all() -> impl Iterator<Item = Verb> {
        VERBS.iter().map(|spec| spec.verb)
    }

    /// As typed: `run`, or `sessions rm`.
    pub fn name(self) -> &'static str {
        self.spec().name
    }

    /// The verb typed as `name` (`run`, `sessions rm`).
    pub fn from_name(name: &str) -> Option<Verb> {
        by_name(name).map(|spec| spec.verb)
    }

    pub(crate) fn spec(self) -> &'static VerbSpec {
        VERBS
            .iter()
            .find(|spec| spec.verb == self)
            .expect("every verb has an entry in the table")
    }
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
mod remedy_tests;

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

    /// Every `Verb` variant, as a match no variant can be left out of:
    /// adding one stops this compiling until it is listed here, and
    /// `every_variant_has_a_table_entry` then holds it to the table. (The
    /// table test above walks the table, so it never meets a variant the
    /// table lacks, and `Verb::spec` would panic for it.)
    fn listed(verb: Verb) -> Verb {
        match verb {
            Verb::Run
            | Verb::Step
            | Verb::Checkpoint
            | Verb::Restore
            | Verb::Undo
            | Verb::Write
            | Verb::Poke
            | Verb::Patch
            | Verb::Show
            | Verb::Read
            | Verb::Shot
            | Verb::View
            | Verb::Sym
            | Verb::Checkpoints
            | Verb::Measure
            | Verb::Equiv
            | Verb::Events
            | Verb::Gaps
            | Verb::Sessions
            | Verb::Prune
            | Verb::Rm
            | Verb::Break
            | Verb::Watch
            | Verb::Delete
            | Verb::New
            | Verb::Build
            | Verb::Assets
            | Verb::Song
            | Verb::Pixels
            | Verb::Font
            | Verb::FontEdit
            | Verb::FontSave
            | Verb::FontImport
            | Verb::FontExport
            | Verb::FontNew
            | Verb::FontDiff
            | Verb::Serve
            | Verb::Batch
            | Verb::Help
            | Verb::Recipes
            | Verb::Schema
            | Verb::Doctor
            | Verb::Opcode
            | Verb::Version
            | Verb::PowerCycle
            | Verb::Sav => verb,
        }
    }

    #[test]
    fn every_variant_has_a_table_entry() {
        const EVERY: [Verb; 46] = [
            Verb::Run,
            Verb::Step,
            Verb::Checkpoint,
            Verb::Restore,
            Verb::Undo,
            Verb::Write,
            Verb::Poke,
            Verb::Patch,
            Verb::Show,
            Verb::Read,
            Verb::Shot,
            Verb::View,
            Verb::Sym,
            Verb::Checkpoints,
            Verb::Measure,
            Verb::Equiv,
            Verb::Events,
            Verb::Gaps,
            Verb::Sessions,
            Verb::Prune,
            Verb::Rm,
            Verb::Break,
            Verb::Watch,
            Verb::Delete,
            Verb::New,
            Verb::Build,
            Verb::Assets,
            Verb::Song,
            Verb::Pixels,
            Verb::Font,
            Verb::FontEdit,
            Verb::FontSave,
            Verb::FontImport,
            Verb::FontExport,
            Verb::FontNew,
            Verb::FontDiff,
            Verb::Serve,
            Verb::Batch,
            Verb::Help,
            Verb::Recipes,
            Verb::Schema,
            Verb::Doctor,
            Verb::Opcode,
            Verb::Version,
            Verb::PowerCycle,
            Verb::Sav,
        ];
        for verb in EVERY {
            assert!(
                VERBS.iter().any(|s| s.verb == listed(verb)),
                "{verb:?} has no entry in VERBS"
            );
            assert!(!verb.name().is_empty());
        }
        assert_eq!(EVERY.len(), VERBS.len(), "a table entry for no variant");
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
