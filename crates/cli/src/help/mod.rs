//! The primer, each verb's `--help`, the topics, recipes and the schema.
//! None of it needs a session, a ROM or the network (cli-design ▸ Grammar).

mod hardware;
mod music;
mod recipes;
mod schema;
mod topics;
mod traps;

pub use recipes::RECIPES;
pub use schema::SCHEMA;
pub use topics::Topic;

use crate::verbs::{CHANGE, Class, OUTPUT, SESSION, SHOT_OPTIONS, VERBS, VerbSpec};

/// Bare `gbcli`: what it is, the verbs by class, the first command and
/// where help lives. At most 40 lines (design §12).
pub fn primer() -> String {
    let mut out = String::from(
        "gbcli — a headless, deterministic Game Boy emulator for agents.\n\
         \n\
         Each command opens a named session (.gbcli/sessions/NAME/), does one thing to\n\
         its machine, saves it and prints rows; --json prints the response line.\n\
         \n",
    );
    for class in Class::ALL {
        let names: Vec<&str> = VERBS
            .iter()
            .filter(|s| s.class == class)
            .map(|s| s.name)
            .collect();
        // Wrapped to the 80 columns the primer keeps, under the verb column.
        let verbs = wrap(&names.join(", "), 80 - 13).replace('\n', &format!("\n{:13}", ""));
        out += &format!(
            "  {:<11}{}\n  {:<11}{}\n",
            class.name(),
            verbs,
            "",
            title(class)
        );
    }
    out += "\n\
        First:  gbcli run game.gb --session demo --frames 120 --shot first.png\n\
        Then:   gbcli show --session demo    gbcli read pc frame --session demo\n\
        New:    gbcli doctor, then gbcli new game (a starter that builds and runs)\n\
        Learn:  gbcli help hardware (the DMG on one page), gbcli help traps,\n\
        \x20       gbcli opcode 'jr nz' (bytes, cycles, flags)\n\
        \n\
        Help:   gbcli VERB --help (one worked example each)\n\
        \x20       gbcli recipes (goal → command)\n\
        \x20       gbcli help TOPIC: addresses expressions time checkpointing writes\n\
        \x20                         budgets exit-codes hardware traps optimize rgbds\n\
        \x20                         music\n\
        \x20       gbcli schema (the JSON request grammar for serve and batch)\n\
        Exit:   0 ok, 1 no, 2 usage, 3 budget/conflict, 4 unreadable, 5 environment\n";
    out
}

/// What a class's verbs have in common, in a few words.
fn title(class: Class) -> &'static str {
    match class {
        Class::Act => "advance or change the machine; the stop becomes the head",
        Class::Read => "look without changing the head",
        Class::Configure => "change the session's settings",
        Class::Build => "RGBDS from PATH: edit, build, restore, measure; song files",
        Class::Server => "the JSON Lines protocol, for harnesses",
        Class::Help => "no session, ROM or network needed",
    }
}

/// `gbcli VERB --help`: what it does, its class, its flags and one worked
/// example.
pub fn verb(spec: &VerbSpec) -> String {
    let mut out = format!(
        "gbcli {} — {}\n\nUSAGE\n  gbcli {} {}\n\n{}\n\nCLASS\n  {}: {}.\n",
        spec.name,
        spec.summary,
        spec.name,
        spec.usage,
        wrap(spec.about, 78),
        spec.class.name(),
        spec.class.preconditions(),
    );
    let own: Vec<_> = spec.flags.iter().collect();
    if !own.is_empty() {
        out += "\nFLAGS\n";
        out += &flag_lines(own.into_iter());
    }
    for group in spec.groups {
        // By content: a const slice has no stable address to compare.
        let heading = if *group == SESSION {
            "SESSION FLAGS (every session verb)"
        } else if *group == CHANGE {
            "CHANGE FLAGS (every verb that changes the session)"
        } else if *group == SHOT_OPTIONS {
            "SHOT FLAGS (how the PNG is taken)"
        } else if *group == OUTPUT {
            "OUTPUT FLAGS (every verb that sends a request)"
        } else {
            "MORE FLAGS"
        };
        out += &format!("\n{heading}\n");
        out += &flag_lines(group.iter());
    }
    out += &format!(
        "\nEXAMPLE\n  {}\n{}\n",
        spec.example.0,
        indent(&wrap(spec.example.1, 74), "    ")
    );
    out
}

fn flag_lines<'a>(flags: impl Iterator<Item = &'a crate::verbs::FlagSpec>) -> String {
    let mut out = String::new();
    for f in flags {
        let short = f.short.map(|c| format!("-{c}, ")).unwrap_or_default();
        let value = f.value.map(|v| format!(" {v}")).unwrap_or_default();
        let repeat = if f.repeat { "…" } else { "" };
        let head = format!("  {short}--{}{value}{repeat}", f.long);
        if head.len() < 26 {
            out += &format!("{head:<26}{}\n", f.help);
        } else {
            out += &format!("{head}\n{:<26}{}\n", "", f.help);
        }
    }
    out
}

fn indent(text: &str, by: &str) -> String {
    text.lines()
        .map(|l| format!("{by}{l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Wraps a paragraph at `width` columns.
fn wrap(text: &str, width: usize) -> String {
    let mut lines = vec![String::new()];
    for word in text.split_whitespace() {
        let line = lines.last_mut().expect("never empty");
        if !line.is_empty() && line.len() + 1 + word.len() > width {
            lines.push(word.to_string());
        } else {
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests;
