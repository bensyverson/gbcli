//! The guard on the text rows: any list that grows with the frames a run
//! covers or the events it saw prints a summary or capped rows in text, and
//! every item in `--json` (design §12, "Bounded rows"). An agent reads the
//! text whole, so a 600-frame run that printed a row per frame or per write
//! buried the answer: the frame budget's 545 `frame_cost` rows (raycaster DX
//! review C-7) were the third time.
//!
//! Each case drives one such render path with far more items than any cap,
//! then counts the text's lines and the JSON's items. A new per-frame or
//! per-event list belongs here.

use std::collections::BTreeMap;

use gbcli_core::ApuWriteEffect;
use gbcli_protocol::{
    ApuWriteLine, AudioMade, Button, EventLogLine, FrameBudgetMade, FrameBudgetRow, KeyAction,
    KeyEvent, OamFrameView, OamLogMade, Op, RasterLineRow, RasterMade, RasterWriteRow, ResultBody,
    Rev, TimeBase,
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
                unsupported: Vec::new(),
            }),
            ..ResultBody::default()
        },
        json_list: "/raster/rows",
    }
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
        oam_log(),
        audio_log(),
        keys_unapplied(),
    ]
}

/// Lists still printed a row per item, each with the reason it isn't
/// bounded yet. The test fails when one becomes bounded, so the entry moves
/// to `bounded()` rather than going stale.
fn known_unbounded() -> Vec<(Case, &'static str)> {
    vec![(
        events(),
        "the event log's rows are rendered in render/mod.rs beside show's, \
         another agent's file this leaf; `--limit N` bounds it by hand",
    )]
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
