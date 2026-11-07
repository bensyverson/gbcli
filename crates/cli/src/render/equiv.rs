//! Rows for the equivalence check: the verdict, each difference in the
//! divergent frame, and where each side stopped and was saved.

use gbcli_protocol::{ApuWriteMade, DifferenceMade, EquivMade, Location, SideMade, StopReason};

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
            } => row
                .field("kind", "ram")
                .field("range", range)
                .field("offset", offset)
                .field("a", byte(*a))
                .field("b", byte(*b))
                .field("a_addr", place(a_at))
                .field("b_addr", place(b_at))
                .field("count", count),
            DifferenceMade::Apu { index, a, b } => row
                .field("kind", "apu")
                .field("index", index)
                .json("a", &write(a))
                .json("b", &write(b)),
        }
        .line();
    }
    out += &side("a", &d.a);
    out += &side("b", &d.b);
    out
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
