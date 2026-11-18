//! The rows for a session the invocation created: the `.gbcli/` it went
//! into, and its note when it has one.

use gbcli_protocol::{CgbFlag, SessionCreated, model_name};

use super::Row;

/// `session_created …`, then a `note` row for each note.
pub fn rows(c: &SessionCreated) -> String {
    let model = model_name(c.model);
    let mut out = Row::new("session_created")
        .field("name", &c.name)
        .field("model", model)
        .opt("cgb", c.cgb.map(CgbFlag::name))
        .field("bundle", &c.bundle)
        .line();
    if let Some(note) = c.note {
        out += &Row::new("note")
            .word(note.name())
            .field("session", &c.name)
            .opt("cgb", c.cgb.map(CgbFlag::name))
            .field("model", model)
            .field("remedy", note.remedy(&c.name))
            .line();
    }
    out
}
