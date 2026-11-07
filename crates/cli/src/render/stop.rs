//! The stop record as two rows: where and why it stopped, and what it spent.

use gbcli_protocol::{StopReason, StopRecord, TimeBase};

use super::row::{Row, addr, byte};

pub fn rows(stop: &StopRecord) -> String {
    // Enum names print as the wire spells them, so rows and --json agree.
    let json = serde_json::to_value(stop).expect("a stop record always serializes");
    let wire = |key: &str| json[key].as_str().unwrap_or("?").to_string();
    let mut row = Row::new("stop").word(&wire("reason"));
    let mut text = None;
    row = match &stop.reason {
        StopReason::Breakpoint { id } => row.field("id", id),
        StopReason::Watchpoint {
            id,
            addr: at,
            old,
            new,
            writer_pc,
            ..
        } => row
            .field("id", id)
            .field("access", wire("access"))
            .field("addr", addr(*at))
            .opt("old", old.map(byte))
            .field("new", byte(*new))
            .field("writer_pc", addr(*writer_pc)),
        StopReason::StepDone { .. } => row.field("unit", wire("unit")),
        StopReason::BudgetExhausted { .. } => row.field("kind", wire("kind")),
        StopReason::IllegalOpcode { opcode } => row.field("opcode", byte(*opcode)),
        StopReason::Predicate { text: t, .. } | StopReason::PredicateError { text: t, .. } => {
            text = Some(t.as_str());
            row.field("cadence", wire("cadence"))
        }
        StopReason::HaltForever
        | StopReason::Stopped
        | StopReason::UserBreak
        | StopReason::WallGuard => row,
    };
    let time = &stop.time;
    row = row
        .field("pc", addr(stop.pc))
        .field("bank", stop.bank)
        .opt("symbol", stop.symbol.as_deref())
        .field("frame", time.frame)
        .field("t", time.t)
        .field("since", since(&time.since))
        .field("ticks", time.ticks)
        .field("ly", time.ly)
        .opt("dot", time.dot);
    if let Some(shown) = &stop.frame_shown {
        row = row.field("shown", shown.n).field(
            "vblank",
            json["frame_shown"]["vblank"].as_str().unwrap_or("?"),
        );
    }
    let line = row.opt("text", text).line();

    let budget = &stop.budget;
    let used = |n: u64, limit: Option<u64>| match limit {
        Some(limit) => format!("{n}/{limit}"),
        None => n.to_string(),
    };
    let mut row = Row::new("budget")
        .field("frames", used(budget.frames, budget.limits.frames))
        .field(
            "instructions",
            used(budget.instructions, budget.limits.instructions),
        )
        .field("ticks", used(budget.ticks, budget.limits.ticks));
    if budget.defaulted {
        row = row.word("defaulted");
    }
    line + &row.line()
}

pub fn since(base: &TimeBase) -> String {
    match base {
        TimeBase::Boot => "boot".into(),
        TimeBase::PowerOn => "power-on".into(),
        TimeBase::Checkpoint(name) => name.clone(),
    }
}
