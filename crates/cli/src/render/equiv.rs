//! Rows for the equivalence check: the verdict and what it compared, each
//! difference in the divergent frame, and where each side stopped and was
//! saved.

use gbcli_protocol::{
    ApuWriteMade, AudioCompareName, DifferenceMade, EquivMade, FrameOffsetMade, Location,
    RamRangeArg, SideMade, SideName, StopReason,
};

use super::row::{Row, addr, byte};
use super::stop;

pub fn rows(e: &EquivMade) -> String {
    let mut out = Row::new("equiv")
        .field("frames", e.frames)
        .opt(
            "ended",
            e.ended
                .as_ref()
                .map(|r| reason(r)["kind"].as_str().unwrap_or("?").to_string()),
        )
        .opt("diverged_at", e.divergence.as_ref().map(|d| d.frame))
        .opt("ticks", e.divergence.as_ref().map(|d| d.ticks))
        .flag("screen", e.compared.screen)
        .opt("ram", ranges(&e.compared.ram))
        .field("audio", audio(e.compared.audio))
        .line();
    let Some(d) = &e.divergence else {
        return out;
    };
    for diff in &d.differences {
        let row = Row::new("difference");
        out += &match diff {
            DifferenceMade::Stop { a, b } => row
                .field("kind", "stop")
                .json("a", &reason(a))
                .json("b", &reason(b)),
            DifferenceMade::Pixels { x, y, a, b, count } => row
                .field("kind", "pixels")
                .field("x", x)
                .field("y", y)
                .field("a", a)
                .field("b", b)
                .field("count", count),
            DifferenceMade::Ram {
                range,
                offset,
                a_at,
                b_at,
                a,
                b,
                count,
                never_written,
            } => row
                .field("kind", "ram")
                .field("range", range)
                .field("offset", offset)
                .field("a", byte(*a))
                .field("b", byte(*b))
                .field("a_addr", place(a_at))
                .field("b_addr", place(b_at))
                .field("count", count)
                .opt("never_written", sides(never_written)),
            DifferenceMade::Apu { index, a, b } => row
                .field("kind", "apu")
                .field("index", index)
                .json("a", &write(a))
                .json("b", &write(b)),
        }
        .line();
    }
    if let Some(o) = &d.frame_offset {
        out += &frame_offset(d.frame, o);
    }
    out += &side("a", &d.a);
    out += &side("b", &d.b);
    out
}

/// The ranges in `--ram`'s own spelling (`wFrame,2`, or `wX` for one byte),
/// space-separated, so each can be passed back as given; `None` for none.
fn ranges(of: &[RamRangeArg]) -> Option<String> {
    let spelled: Vec<String> = of
        .iter()
        .map(|r| match r.len {
            1 => r.at.clone(),
            len => format!("{},{len}", r.at),
        })
        .collect();
    (!spelled.is_empty()).then(|| spelled.join(" "))
}

fn audio(mode: AudioCompareName) -> String {
    serde_json::to_value(mode)
        .expect("serializes")
        .as_str()
        .unwrap_or("?")
        .to_string()
}

/// `frame_offset frame=11 earlier=b t=20356 ticks=40712`: the divergent
/// frame ended sooner on one side, counted from each side's start.
fn frame_offset(frame: u64, o: &FrameOffsetMade) -> String {
    Row::new("frame_offset")
        .field("frame", frame)
        .field("earlier", side_word(o.earlier))
        .field("t", o.t)
        .field("ticks", o.ticks)
        .line()
}

fn side_word(side: SideName) -> &'static str {
    match side {
        SideName::A => "a",
        SideName::B => "b",
    }
}

/// `a`, `b` or `a,b`; `None` for no side.
fn sides(of: &[SideName]) -> Option<String> {
    let names: Vec<&str> = of.iter().map(|&side| side_word(side)).collect();
    (!names.is_empty()).then(|| names.join(","))
}

fn place(at: &Location) -> String {
    match at.bank {
        Some(bank) => format!("{bank:02X}:{}", &addr(at.addr)[1..]),
        None => addr(at.addr),
    }
}

fn side(which: &str, s: &SideMade) -> String {
    Row::new("side")
        .word(which)
        .field("checkpoint", &s.checkpoint.name)
        .field("path", &s.checkpoint.path)
        .opt("frame", s.checkpoint.frame)
        .line()
        + &stop::rows(&s.stop)
}

fn reason(r: &StopReason) -> serde_json::Value {
    serde_json::to_value(r).expect("serializes")
}

fn write(w: &Option<ApuWriteMade>) -> serde_json::Value {
    serde_json::to_value(w).expect("serializes")
}
