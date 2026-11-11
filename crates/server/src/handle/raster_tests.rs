//! `run --raster` through the handler (design §11): the writes by frame and
//! line, the gaps that reach the ledger from a run that succeeds, how the
//! run ended, and the STOP warning.

use gbcli_protocol::{Op, PpuModeView, RasterMade, Response, RunArgs, StopReason};

use super::Handler;
use super::test_support::{handler, request};
use crate::fixture::{Fixture, ILLEGAL, RASTER, RASTER_SYM, SCROLL_LINES, STOP_WAKE};

fn run(h: &mut Handler, args: RunArgs) -> Response {
    h.handle(&request(Op::Run(args)))
}

fn raster_run(f: &Fixture, frames: u64) -> Response {
    run(
        &mut handler(f),
        RunArgs {
            frames: Some(frames),
            raster: true,
            ..RunArgs::default()
        },
    )
}

fn sym(name: &str) -> u16 {
    RASTER_SYM
        .lines()
        .find_map(|l| {
            let (at, label) = l.split_once(' ')?;
            (label == name).then(|| u16::from_str_radix(&at[3..], 16).unwrap())
        })
        .unwrap_or_else(|| panic!("no {name} in raster.sym"))
}

/// (line, [(register, value)]) of a row.
type WrittenLine<'a> = (Option<u8>, Vec<(&'a str, u8)>);

fn made(response: Response) -> RasterMade {
    response.outcome.expect("ran").raster.expect("a timeline")
}

/// LoadHigh's ask: the scroll each line was drawn with, around a write in
/// each mode (`roms/scroll_lines.asm`).
#[test]
fn run_raster_gives_the_scroll_each_visible_line_began_with() {
    let f = Fixture::with_rom("raster-scroll", SCROLL_LINES);
    let t = made(raster_run(&f, 4));
    let whole: Vec<_> = t
        .drawn
        .iter()
        .filter(|d| d.first_line == 0 && d.scx.len() == 144)
        .collect();
    assert!(whole.len() >= 2, "{:?}", t.drawn);
    // The frame after the LCD comes on is whole: SameBoy reports no start for
    // its line 0, the path this ROM toggles the LCD to reach.
    let lit = t
        .rows
        .iter()
        .find(|r| r.writes.iter().any(|w| w.reg == "rLCDC" && w.value == 0x91))
        .expect("the ROM turns the LCD on");
    let after = t
        .drawn
        .iter()
        .find(|d| d.frame > lit.frame)
        .expect("a frame");
    assert_eq!((after.first_line, after.scx.len()), (0, 144));
    // The ROM's three writes landed in the modes the expectation rests on.
    let mode_of = |line: u8, reg: &str| {
        t.rows
            .iter()
            .filter(|r| r.line == Some(line))
            .flat_map(|r| &r.writes)
            .find(|w| w.reg == reg)
            .and_then(|w| w.mode)
    };
    assert_eq!(mode_of(20, "rSCY"), Some(PpuModeView::OamScan));
    assert_eq!(mode_of(40, "rSCX"), Some(PpuModeView::Drawing));
    assert_eq!(mode_of(60, "rSCX"), Some(PpuModeView::Hblank));
    for d in whole {
        assert_eq!(d.scy.len(), 144);
        let mut scx = vec![0u8; 41];
        scx.resize(61, 16);
        scx.resize(144, 32);
        assert_eq!(d.scx, scx, "frame {}", d.frame);
        let mut scy = vec![0u8; 20];
        scy.resize(144, 7);
        assert_eq!(d.scy, scy, "frame {}", d.frame);
    }
}

#[test]
fn run_raster_lists_each_write_on_its_line() {
    let f = Fixture::with_rom("raster-rows", RASTER);
    let t = made(raster_run(&f, 4));
    // Frames 1 and 2 are whole; each has the ROM's writes on four lines.
    for frame in 1..=2 {
        let lines: Vec<WrittenLine> = t
            .rows
            .iter()
            .filter(|r| r.frame == frame)
            .map(|r| {
                let w = r.writes.iter().map(|w| (w.reg.as_str(), w.value)).collect();
                (r.line, w)
            })
            .collect();
        assert_eq!(
            lines,
            vec![
                (Some(40), vec![("rSCX", 16)]),
                (Some(80), vec![("rBGP", 0x1B)]),
                (Some(120), vec![("rBGP", 0xE4)]),
                (
                    Some(144),
                    vec![("rSCX", 0), ("rLYC", 30), ("rSTAT", 0x40), ("rIF", 0)]
                ),
            ],
            "frame {frame}"
        );
    }
    let scx = t
        .rows
        .iter()
        .find(|r| r.frame == 1 && r.line == Some(40))
        .unwrap();
    assert_eq!(scx.writes[0].pc, sym("ScxMid"));
    let into = scx.writes[0].ticks_into_line.expect("line 40 has a start");
    // Mode 3 of line 40: after mode 2's 80 dots, before 80 + 289.
    assert!((160..738).contains(&into), "{into}");
    assert!(scx.start.is_some());
    // Every line the run passed, not only the written ones.
    assert!(t.lines > 3 * 153, "{}", t.lines);
    assert!(t.frames >= 4, "{}", t.frames);
    assert_eq!(t.unsupported, vec!["ppu_dot", "ppu_mode_events"]);
}

#[test]
fn a_raster_runs_gaps_reach_the_ledger_though_it_succeeds() {
    let f = Fixture::with_rom("raster-gaps", RASTER);
    let response = raster_run(&f, 1);
    assert!(response.is_ok(), "{:?}", response.error());
    let mut gaps: Vec<(String, u64)> = f
        .bundle
        .gap_counts()
        .unwrap()
        .into_iter()
        .map(|g| (g.feature, g.count))
        .collect();
    gaps.sort();
    assert_eq!(
        gaps,
        vec![("ppu_dot".into(), 1), ("ppu_mode_events".into(), 1)]
    );
    let rows = f.bundle.session("demo").unwrap().events(None).unwrap();
    let line = rows.last().unwrap();
    assert!(line.ok);
    assert_eq!(line.gaps.get("ppu_dot"), Some(&1));

    // A run without --raster adds nothing.
    let plain = run(
        &mut handler(&f),
        RunArgs {
            frames: Some(1),
            ..RunArgs::default()
        },
    );
    assert!(plain.is_ok());
    assert_eq!(f.bundle.gap_counts().unwrap().len(), 2);
    assert!(f.bundle.gap_counts().unwrap().iter().all(|g| g.count == 1));
}

#[test]
fn a_raster_run_cut_short_says_how_it_ended() {
    let f = Fixture::with_rom("raster-locked", ILLEGAL);
    let response = raster_run(&f, 10);
    let error = response.error().cloned();
    let (stop, raster) = match response.outcome {
        Ok(result) => (result.stop, result.raster),
        Err(body) => (body.stop.clone(), None),
    };
    let stop = stop.unwrap_or_else(|| panic!("no stop: {error:?}"));
    assert_eq!(stop.reason, StopReason::IllegalOpcode { opcode: 0xD3 });
    assert!(stop.budget.frames < 10, "{:?}", stop.budget);
    let raster = raster.expect("the timeline rides with the stop");
    assert!(raster.rows.iter().all(|r| r.writes.is_empty()));
}

#[test]
fn a_run_ending_in_stop_warns_that_only_a_key_wakes_it() {
    let f = Fixture::with_rom("stop-warning", STOP_WAKE);
    let response = run(
        &mut handler(&f),
        RunArgs {
            frames: Some(2),
            ..RunArgs::default()
        },
    );
    let warnings = response.warnings.clone();
    let stop = response.outcome.expect("ran").stop.expect("a stop");
    assert!(
        matches!(stop.reason, StopReason::BudgetExhausted { .. }),
        "{:?}",
        stop.reason
    );
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("STOP") && w.contains("run --keys")),
        "{warnings:?}"
    );
}
