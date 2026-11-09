//! argv → the verb, its flags in order, and its positional arguments, checked
//! against the verb table. What the flags mean is `build`'s business.

use std::ffi::OsString;

use lexopt::prelude::*;

use crate::usage::{Usage, did_you_mean, near};
use crate::verbs::{Class, FlagSpec, VERBS, VerbSpec, by_name};

/// What the command line asks for, before its flags are interpreted.
#[derive(Debug)]
pub enum Parsed {
    /// Bare `gbcli`: the primer.
    Primer,
    /// `gbcli <verb> --help`.
    VerbHelp(&'static VerbSpec),
    Verb(Invocation),
}

/// A verb with its flags, in the order given, and its positional arguments.
#[derive(Debug)]
pub struct Invocation {
    pub spec: &'static VerbSpec,
    pub flags: Vec<(&'static FlagSpec, String)>,
    pub positionals: Vec<String>,
}

impl Invocation {
    /// Whether a switch, or a flag with a value, was given.
    pub fn has(&self, long: &str) -> bool {
        self.flags.iter().any(|(f, _)| f.long == long)
    }

    /// The value of a flag that may be given once.
    pub fn one(&self, long: &str) -> Option<&str> {
        self.flags
            .iter()
            .find(|(f, _)| f.long == long)
            .map(|(_, v)| v.as_str())
    }

    /// Every value of a flag, in order.
    pub fn all<'a>(&'a self, long: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.flags
            .iter()
            .filter(move |(f, _)| f.long == long)
            .map(|(_, v)| v.as_str())
    }
}

/// `sessions prune` and `sessions rm` are typed as two words.
const SUBVERBS: &[&str] = &["prune", "rm"];

/// So are `font edit`, `font save` and the other authoring verbs.
const FONT_SUBVERBS: &[&str] = &["edit", "save", "import", "export", "new", "diff"];

pub fn parse(args: Vec<OsString>) -> Result<Parsed, Usage> {
    let args = args
        .into_iter()
        .map(|a| {
            a.into_string().map_err(|a| {
                Usage::new(
                    format!("the argument {a:?} isn't UTF-8"),
                    "pass paths and expressions as UTF-8 text",
                )
            })
        })
        .collect::<Result<Vec<String>, Usage>>()?;
    let Some(first) = args.first() else {
        return Ok(Parsed::Primer);
    };
    if matches!(first.as_str(), "--help" | "-h") {
        return Ok(Parsed::Primer);
    }
    let first = if first == "--version" {
        "version"
    } else {
        first.as_str()
    };
    let (spec, rest) = match (first, args.get(1).map(String::as_str)) {
        ("sessions", Some(sub)) if SUBVERBS.contains(&sub) => {
            (by_name(&format!("sessions {sub}")), &args[2..])
        }
        ("font", Some(sub)) if FONT_SUBVERBS.contains(&sub) => {
            (by_name(&format!("font {sub}")), &args[2..])
        }
        _ => (by_name(first), &args[1..]),
    };
    let Some(spec) = spec else {
        return Err(unknown_verb(first));
    };
    let before_dashes = rest.iter().take_while(|a| *a != "--");
    if before_dashes
        .into_iter()
        .any(|a| a == "--help" || a == "-h")
    {
        return Ok(Parsed::VerbHelp(spec));
    }
    let invocation = flags(spec, rest)?;
    check_arity(&invocation)?;
    Ok(Parsed::Verb(invocation))
}

fn unknown_verb(word: &str) -> Usage {
    if word.starts_with('-') {
        return Usage::new(
            format!("`{word}` before a command"),
            "the command comes first: `gbcli <command> [flags]`; `gbcli` lists the commands",
        );
    }
    let names = VERBS.iter().map(|s| s.name);
    let mut near = near(word, names);
    if SUBVERBS.contains(&word) {
        near.insert(
            0,
            if word == "rm" {
                "sessions rm"
            } else {
                "sessions prune"
            },
        );
        near.dedup();
    }
    Usage::new(
        format!("unknown command `{word}`"),
        format!("{}`gbcli` lists the commands", did_you_mean(&near, "")),
    )
}

fn flags(spec: &'static VerbSpec, rest: &[String]) -> Result<Invocation, Usage> {
    let mut invocation = Invocation {
        spec,
        flags: Vec::new(),
        positionals: Vec::new(),
    };
    let help = format!("`gbcli {} --help` lists its flags", spec.name);
    let lexopt_error = |e: lexopt::Error| Usage::new(e.to_string(), help.clone());
    let mut parser = lexopt::Parser::from_args(rest.iter().cloned());
    while let Some(arg) = parser.next().map_err(lexopt_error)? {
        let flag = match arg {
            Value(value) => {
                let value = value
                    .into_string()
                    .expect("arguments were checked as UTF-8");
                invocation.positionals.push(value);
                continue;
            }
            Long(name) => spec
                .flag(name)
                .ok_or_else(|| unknown_flag(spec, &format!("--{name}"), name))?,
            Short(c) => spec
                .short(c)
                .ok_or_else(|| unknown_flag(spec, &format!("-{c}"), &c.to_string()))?,
        };
        if !flag.repeat && invocation.has(flag.long) {
            return Err(Usage::new(
                format!("--{} is given twice", flag.long),
                format!("give it once; {help}"),
            ));
        }
        let value = match flag.value {
            Some(name) => parser
                .value()
                .map_err(|_| {
                    Usage::new(
                        format!("--{} needs a value ({name})", flag.long),
                        format!("`--{} {name}`; {help}", flag.long),
                    )
                })?
                .into_string()
                .expect("arguments were checked as UTF-8"),
            None => String::new(),
        };
        invocation.flags.push((flag, value));
    }
    Ok(invocation)
}

fn unknown_flag(spec: &VerbSpec, typed: &str, name: &str) -> Usage {
    if name == "rev" && matches!(spec.class, Class::Read | Class::Project) {
        let why = if spec.class == Class::Read {
            "changes nothing"
        } else {
            "works on the whole project, not one session's head"
        };
        return Usage::new(
            format!("`gbcli {}` {why}, so --rev has nothing to guard", spec.name),
            "drop --rev; it guards the verbs that change a session (run, step, write, \
             checkpoint, break, watch, delete, …)",
        );
    }
    let names = spec.all_flags().map(|f| f.long);
    let near = near(name, names);
    Usage::new(
        format!("`gbcli {}` has no flag {typed}", spec.name),
        format!(
            "{}`gbcli {} --help` lists its flags",
            did_you_mean(&near, "--"),
            spec.name
        ),
    )
}

fn check_arity(invocation: &Invocation) -> Result<(), Usage> {
    let spec = invocation.spec;
    let arity = spec.arity;
    let given = invocation.positionals.len();
    let usage = format!("usage: gbcli {} {}", spec.name, spec.usage);
    if given < arity.min {
        return Err(Usage::new(
            format!("`gbcli {}` needs {}", spec.name, arity.name),
            usage,
        ));
    }
    if let Some(max) = arity.max
        && given > max
    {
        let extra = &invocation.positionals[max];
        return Err(Usage::new(
            format!("`gbcli {}` doesn't take `{extra}`", spec.name),
            usage,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
