//! The guard on the text rows: any list that grows with the frames a run
//! covers, the events it saw or the size of the program it ran prints a
//! summary or capped rows in text, and every item in `--json` (design §12,
//! "Bounded rows"). An agent reads the text whole, so a 600-frame run that
//! printed a row per frame or per write buried the answer: the frame
//! budget's 545 `frame_cost` rows (raycaster DX review C-7) were the third
//! time. A big game's hundreds of raster writer sites were the first sized
//! by the program.
//!
//! Each case drives one such render path with far more items than any cap,
//! then counts the text's lines and the JSON's items. A new per-frame,
//! per-event or per-site list belongs here.

use std::collections::BTreeMap;

use gbcli_core::ApuWriteEffect;
use gbcli_protocol::{
    ApuWriteLine, AudioMade, Button, EventLogLine, FrameBudgetMade, FrameBudgetRow, KeyAction,
    KeyEvent, OamFrameView, OamLogMade, Op, ProfileMade, RasterLineRow, RasterMade, RasterWriteRow,
    ResultBody, Rev, RoutineRow, TimeBase,
};

/// Far more items than any cap.
const N: usize = 1000;
/// The most text lines a case may print however many items it holds.
const MAX_TEXT_LINES: usize = 64;

struct Case {
    name: &'static str,
    result: ResultBody,
    /// Where the items sit in the result's JSON.
    json_list: &'static str,
}

fn frame_budget() -> Case {
    // Two lag frames, then one that made its budget: N / 3 runs.
    let frames = (0..N)
        .map(|i| FrameBudgetRow {
            frame: i as i64,
            active_t: 70224,
            idle_t: 0,
            lag: Some(i % 3 != 2),
            lcd_off: false,
            partial: false,
        })
        .collect();
    Case {
        name: "run --frame-budget",
        result: ResultBody {
            frame_budget: Some(FrameBudgetMade {
                frames,
                lag: (N - N / 3) as u64,
                lcd_off: 0,
                active_t: None,
                busiest: None,
            }),
            ..ResultBody::default()
        },
        json_list: "/frame_budget/frames",
    }
}

fn raster() -> Case {
    let rows = (0..N)
        .map(|i| RasterLineRow {
            frame: (i / 144) as u32,
            line: Some((i % 144) as u8),
            start: None,
            writes: vec![RasterWriteRow {
                reg: "rSCX".into(),
                value: i as u8,
                pc: 0x150,
                ticks_into_line: None,
                mode: None,
            }],
        })
        .collect();
    Case {
        name: "run --raster",
        result: ResultBody {
            raster: Some(RasterMade {
                frames: (N / 144 + 1) as u32,
                lines: N as u64,
                rows,
                drawn: Vec::new(),
                unsupported: Vec::new(),
            }),
            ..ResultBody::default()
        },
        json_list: "/raster/rows",
    }
}

/// An LCD-off stretch is one `line=off` row holding every IO write made
/// while the LCD was off (joypad polls, sound), so its writes need a cap of
/// their own (the rev-cli review's probe).
fn raster_one_line() -> Case {
    let writes = (0..N)
        .map(|i| RasterWriteRow {
            reg: if i % 2 == 0 { "rP1" } else { "rNR12" }.into(),
            value: 0x20,
            pc: 0x200 + (i % 2) as u16,
            ticks_into_line: None,
            mode: None,
        })
        .collect();
    Case {
        name: "run --raster (one LCD-off line)",
        result: ResultBody {
            raster: Some(RasterMade {
                frames: 1,
                lines: 1,
                rows: vec![RasterLineRow {
                    frame: 0,
                    line: None,
                    start: None,
                    writes,
                }],
                drawn: Vec::new(),
                unsupported: Vec::new(),
            }),
            ..ResultBody::default()
        },
        json_list: "/raster/rows/0/writes",
    }
}

fn raster_of(name: &'static str, rows: Vec<RasterLineRow>) -> Case {
    Case {
        name,
        result: ResultBody {
            raster: Some(RasterMade {
                frames: (N / 144 + 1) as u32,
                lines: N as u64,
                rows,
                drawn: Vec::new(),
                unsupported: vec!["ppu_dot".into(), "ppu_mode_events".into()],
            }),
            ..ResultBody::default()
        },
        json_list: "/raster/rows",
    }
}

/// A line of `writes` IO writes, the `n`th at `pc(n)`.
fn raster_line(i: usize, writes: usize, pc: impl Fn(usize) -> u16) -> RasterLineRow {
    RasterLineRow {
        frame: (i / 144) as u32,
        line: Some((i % 144) as u8),
        start: None,
        writes: (0..writes)
            .map(|n| RasterWriteRow {
                reg: "rSCX".into(),
                value: n as u8,
                pc: pc(n),
                ticks_into_line: None,
                mode: None,
            })
            .collect(),
    }
}

/// The writer summary is sized by the program, not the run: a big game
/// writes its scroll and palette registers from hundreds of places (Ben,
/// 2026-11-09, "adventure games will go way past the bounds").
fn raster_writer_sites() -> Case {
    let rows = (0..N)
        .map(|i| raster_line(i, 3, |n| 0x4000 + (i * 3 + n) as u16))
        .collect();
    raster_of("run --raster (hundreds of writer sites)", rows)
}

/// A raster effect that sets SCX, SCY and BGP on every line printed 85
/// lines under the nested caps (review of ae7261d, rev-cli2's probe).
fn raster_three_per_line() -> Case {
    let rows = (0..N)
        .map(|i| raster_line(i, 3, |n| 0x150 + n as u16))
        .collect();
    raster_of("run --raster (three writes on every line)", rows)
}

/// Every line holds more writes than a line's cap: 20 lines of 20 writes
/// each was 440 rows.
fn raster_busy_lines() -> Case {
    let rows = (0..N)
        .map(|i| raster_line(i, 30, |n| 0x150 + (n % 3) as u16))
        .collect();
    raster_of("run --raster (many writes on every line)", rows)
}

fn oam_log() -> Case {
    let frames = (0..N)
        .map(|i| OamFrameView {
            frame: i as i64,
            lcd_off: false,
            height: 8,
            objects: Vec::new(),
        })
        .collect();
    Case {
        name: "run --oam-log",
        result: ResultBody {
            oam_log: Some(OamLogMade {
                since: TimeBase::Boot,
                frames,
            }),
            ..ResultBody::default()
        },
        json_list: "/oam_log/frames",
    }
}

fn audio_log() -> Case {
    let log = (0..N)
        .map(|i| ApuWriteLine {
            ticks: i as u64 * 100,
            elapsed: i as i64 * 100,
            frame: (i / 10) as i64,
            ly: Some(144),
            pc: 0x1C5,
            addr: 0xFF21,
            value: 0xA1,
            effect: ApuWriteEffect::Applied,
            text: "NR42 $A1: noise envelope 10 down, pace 1".into(),
        })
        .collect();
    Case {
        name: "run --audio-log",
        result: ResultBody {
            audio: Some(AudioMade {
                since: TimeBase::Boot,
                channels: Vec::new(),
                log: Some(log),
                summary: None,
                wavs: Vec::new(),
                piano: None,
                spectrogram: None,
            }),
            ..ResultBody::default()
        },
        json_list: "/audio/log",
    }
}

fn keys_unapplied() -> Case {
    let keys = (0..N)
        .map(|i| KeyEvent {
            at: i as u64,
            button: Button::A,
            action: if i % 2 == 0 {
                KeyAction::Press
            } else {
                KeyAction::Release
            },
        })
        .collect();
    Case {
        name: "run --keys (keys_unapplied)",
        result: ResultBody {
            keys_unapplied: Some(keys),
            ..ResultBody::default()
        },
        json_list: "/keys_unapplied",
    }
}

/// Routines are sized by the program: a big game has hundreds.
fn profile() -> Case {
    let routines = (0..N)
        .map(|i| RoutineRow {
            name: Some(format!("Routine{i}")),
            bank: 1,
            addr: 0x4000 + i as u16,
            calls: 1,
            exclusive_t: (N - i) as u64,
            idle_t: 0,
            inclusive_t: (N - i) as u64,
        })
        .collect();
    Case {
        name: "run --profile",
        result: ResultBody {
            profile: Some(ProfileMade {
                routines,
                omitted: 0,
                total_t: 0,
                idle_t: 0,
                unwound: 0,
                mismatched: 0,
                ret_jumps: 0,
                open: 0,
            }),
            ..ResultBody::default()
        },
        json_list: "/profile/routines",
    }
}

fn events() -> Case {
    let events = (0..N)
        .map(|i| EventLogLine {
            at: 1_793_016_000 + i as u64,
            actor: None,
            op: Op::Show,
            allow_rom_change: false,
            ok: true,
            code: None,
            rev_before: Rev(i as u64),
            rev_after: Rev(i as u64),
            gaps: BTreeMap::new(),
            stop: None,
        })
        .collect();
    Case {
        name: "events",
        result: ResultBody {
            events: Some(events),
            ..ResultBody::default()
        },
        json_list: "/events",
    }
}

/// The lists that must be bounded.
fn bounded() -> Vec<Case> {
    vec![
        frame_budget(),
        raster(),
        raster_one_line(),
        raster_writer_sites(),
        raster_three_per_line(),
        raster_busy_lines(),
        oam_log(),
        audio_log(),
        keys_unapplied(),
    ]
}

/// Lists still printed a row per item, each with the reason it isn't
/// bounded yet. The test fails when one becomes bounded, so the entry moves
/// to `bounded()` rather than going stale.
fn known_unbounded() -> Vec<(Case, &'static str)> {
    vec![
        (
            events(),
            "the event log's rows are rendered in render/mod.rs beside show's, \
             another agent's file this leaf; `--limit N` bounds it by hand",
        ),
        (
            profile(),
            "`--top N` bounds it by hand, and the server counts what it leaves \
             out (`omitted=`); a default cap is a change to the run handler, \
             another agent's files when the rule widened to the program's size",
        ),
    ]
}

fn text_lines(c: &Case) -> usize {
    super::rows(&c.result).lines().count()
}

fn json_items(c: &Case) -> usize {
    let json = serde_json::to_value(&c.result).expect("serializes");
    json.pointer(c.json_list)
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("{}: no list at {}", c.name, c.json_list))
        .len()
}

#[test]
fn per_frame_and_per_event_lists_are_bounded_in_text_and_whole_in_json() {
    let mut flooded = Vec::new();
    for c in bounded() {
        assert_eq!(
            json_items(&c),
            N,
            "{}: --json must carry every item",
            c.name
        );
        let lines = text_lines(&c);
        assert!(
            lines > 0,
            "{}: printed nothing, so it tests nothing",
            c.name
        );
        if lines > MAX_TEXT_LINES {
            flooded.push(format!("{} printed {lines} lines for {N} items", c.name));
        }
    }
    assert!(
        flooded.is_empty(),
        "text output must summarize or cap a per-frame or per-event list \
         (at most {MAX_TEXT_LINES} lines): {flooded:#?}"
    );
}

#[test]
fn the_known_unbounded_lists_are_still_unbounded() {
    for (c, why) in known_unbounded() {
        assert_eq!(json_items(&c), N, "{}", c.name);
        assert!(
            text_lines(&c) > MAX_TEXT_LINES,
            "{} is bounded now ({why}): move it to bounded()",
            c.name
        );
    }
}

/// `run --help` and the recipes say "at most 20" for these lists.
#[test]
fn the_cap_is_the_one_the_help_states() {
    assert_eq!(
        super::row::CAP,
        20,
        "update the run flags' help and the recipes"
    );
}
