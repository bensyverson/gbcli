//! The raster timeline on `roms/raster.asm`, which writes SCX and BGP on
//! known lines in known modes every frame: the core's raster events, then
//! the engine's timeline built from them.

mod common;

use common::{cgb_machine, machine, quick_dmg};
use gbcli_core::{
    Budget, BudgetKind, Core, CoreConfig, CoreError, Event, EventKinds, EventSink, Feature, Gap,
    IoReg, Model, RunRequest, StopReason, Ticks,
};
use gbcli_engine::profile::{RasterLine, RasterTimeline};
use gbcli_engine::{Backend, Machine, Symbols};

const ROM: &[u8] = include_bytes!("roms/raster.gb");
const SYM: &str = include_str!("roms/raster.sym");

/// Ticks in one line: 456 dots of two ticks, at either CPU speed.
const LINE: u64 = 912;
/// Mode 2 is 80 dots; mode 3 at least 172 and at most 289.
const MODE3_START: u64 = 2 * 80;
const MODE3_LONGEST_END: u64 = 2 * (80 + 289);
const MODE0_EARLIEST: u64 = 2 * (80 + 172);

fn addr(name: &str) -> u16 {
    Symbols::parse(SYM).unwrap().get(name).unwrap().addr
}

#[derive(Default)]
struct Collect {
    modes: bool,
    events: Vec<Event>,
}

impl EventSink for Collect {
    fn kinds(&self) -> EventKinds {
        EventKinds {
            exec: false,
            raster: true,
            ppu_modes: self.modes,
        }
    }
    fn event(&mut self, event: Event) {
        self.events.push(event);
    }
}

fn collect(core: &mut dyn Core, frames: u64) -> Vec<Event> {
    let mut sink = Collect::default();
    let mut req = RunRequest::new(Budget::frames(frames));
    req.sink = Some(&mut sink);
    core.run(&mut req).expect("run");
    sink.events
}

/// (pc, register, value, line) of each write, in order.
fn writes(events: &[Event]) -> Vec<(u16, u8, u8, Option<u8>)> {
    events
        .iter()
        .filter_map(|e| match *e {
            Event::IoWrite {
                pc,
                reg,
                value,
                line,
                ..
            } => Some((pc, reg, value, line)),
            _ => None,
        })
        .collect()
}

/// One frame of the ROM's writes, as (pc, register, value, line).
fn frame_writes() -> Vec<(u16, u8, u8, Option<u8>)> {
    vec![
        (addr("ScxMid"), IoReg::SCX.0, 16, Some(40)),
        (addr("BgpMid"), IoReg::BGP.0, 0x1B, Some(80)),
        (addr("BgpHBlank"), IoReg::BGP.0, 0xE4, Some(120)),
        (addr("ScxVBlank"), IoReg::SCX.0, 0, Some(144)),
        (addr("LycWrite"), IoReg::LYC.0, 30, Some(144)),
        (addr("StatWrite"), IoReg::STAT.0, 0x40, Some(144)),
        (addr("IfWrite"), IoReg::IF.0, 0, Some(144)),
    ]
}

#[test]
fn each_io_write_arrives_with_its_line() {
    let mut core = quick_dmg(ROM);
    let got = writes(&collect(&mut core, 4));
    let lcd_on = addr("LcdOn") + 2; // after `ld a, $91`
    let mut want = vec![(lcd_on, IoReg::LCDC.0, 0x91, None)];
    for _ in 0..3 {
        want.extend(frame_writes());
    }
    assert_eq!(got[..want.len()], want[..]);
}

#[test]
fn the_backend_reports_no_dot() {
    let mut core = quick_dmg(ROM);
    let events = collect(&mut core, 2);
    assert!(events.iter().any(|e| matches!(e, Event::IoWrite { .. })));
    for e in &events {
        if let Event::IoWrite { dot, .. } = e {
            assert_eq!(*dot, None);
        }
    }
}

/// The line starts, in order.
fn line_starts(events: &[Event]) -> Vec<(u8, Ticks)> {
    events
        .iter()
        .filter_map(|e| match *e {
            Event::Scanline { ly, at } => Some((ly, at)),
            _ => None,
        })
        .collect()
}

#[test]
fn line_starts_are_exactly_one_line_apart() {
    let mut core = quick_dmg(ROM);
    let starts = line_starts(&collect(&mut core, 4));
    let mut pairs = 0;
    for pair in starts.windows(2) {
        let ((a, at_a), (b, at_b)) = (pair[0], pair[1]);
        if b == a + 1 {
            assert_eq!(at_b.0 - at_a.0, LINE, "line {a} to {b}");
            pairs += 1;
        }
    }
    assert!(pairs > 400, "only {pairs} consecutive line pairs");
}

#[test]
fn line_starts_fall_on_one_grid_while_the_cpu_runs_or_halts() {
    // SameBoy hands the display smaller chunks during HALT (lines 145 to 30)
    // than while instructions run, so a stamp taken at a chunk's end would
    // drift off the grid there.
    let mut core = quick_dmg(ROM);
    let starts = line_starts(&collect(&mut core, 4));
    let phase = starts[0].1.0 % LINE;
    for (ly, at) in starts {
        assert_eq!(at.0 % LINE, phase, "line {ly} at {}", at.0);
    }
}

#[test]
fn every_line_but_153_starts_in_a_full_frame() {
    let mut core = quick_dmg(ROM);
    let starts = line_starts(&collect(&mut core, 4));
    // From the second line 0, which starts a frame SameBoy fully reports.
    let zeros: Vec<usize> = (0..starts.len()).filter(|&i| starts[i].0 == 0).collect();
    let frame: Vec<u8> = starts[zeros[0]..zeros[1]].iter().map(|s| s.0).collect();
    assert_eq!(frame, (0..=152).collect::<Vec<u8>>());
}

#[test]
fn ppu_mode_events_are_refused_with_a_reason() {
    let mut core = quick_dmg(ROM);
    let mut sink = Collect {
        modes: true,
        ..Collect::default()
    };
    let mut req = RunRequest::new(Budget::frames(1));
    req.sink = Some(&mut sink);
    let err = core.run(&mut req).unwrap_err();
    assert!(
        matches!(err, CoreError::Unsupported { feature: Feature::PpuModeEvents, reason } if !reason.is_empty()),
        "{err:?}"
    );
}

#[test]
fn a_raster_sink_leaves_the_emulation_unchanged() {
    let mut plain = quick_dmg(ROM);
    let mut tapped = quick_dmg(ROM);
    common::run(&mut plain, Budget::frames(3));
    collect(&mut tapped, 3);
    assert_eq!(plain.now(), tapped.now());
    assert_eq!(plain.save(), tapped.save());
}

fn timeline(frames: u32) -> RasterTimeline {
    let mut m = machine(ROM, Some(SYM));
    m.raster(frames).expect("raster").timeline
}

/// The rows that hold writes, as (frame, line, [(register, value)]).
/// (frame, line, [(register, value)]) of a row.
type WrittenRow = (u32, Option<u8>, Vec<(u8, u8)>);

fn written_rows(t: &RasterTimeline) -> Vec<WrittenRow> {
    t.lines
        .iter()
        .filter(|row| !row.writes.is_empty())
        .map(|row| {
            let w = row.writes.iter().map(|w| (w.reg.0, w.value)).collect();
            (row.frame, row.line, w)
        })
        .collect()
}

#[test]
fn the_timeline_places_each_write_on_its_line() {
    let t = timeline(4);
    let rows = written_rows(&t);
    let mut want = vec![(0, None, vec![(IoReg::LCDC.0, 0x91)])];
    for frame in 1..=2 {
        want.push((frame, Some(40), vec![(IoReg::SCX.0, 16)]));
        want.push((frame, Some(80), vec![(IoReg::BGP.0, 0x1B)]));
        want.push((frame, Some(120), vec![(IoReg::BGP.0, 0xE4)]));
        want.push((
            frame,
            Some(144),
            vec![
                (IoReg::SCX.0, 0),
                (IoReg::LYC.0, 30),
                (IoReg::STAT.0, 0x40),
                (IoReg::IF.0, 0),
            ],
        ));
    }
    assert_eq!(rows[..want.len()], want[..]);
}

fn row(t: &RasterTimeline, frame: u32, line: u8) -> &RasterLine {
    t.lines
        .iter()
        .find(|r| r.frame == frame && r.line == Some(line))
        .unwrap_or_else(|| panic!("no row for line {line} of frame {frame}"))
}

#[test]
fn a_write_sits_in_its_line_where_its_mode_says() {
    let t = timeline(4);
    for frame in 1..=2 {
        for line in [40, 80] {
            let r = row(&t, frame, line);
            let into = r.ticks_into_line(&r.writes[0]).expect("line start");
            assert!(
                (MODE3_START..MODE3_LONGEST_END).contains(&into),
                "mode-3 write {into} ticks into line {line}"
            );
        }
        let r = row(&t, frame, 120);
        let into = r.ticks_into_line(&r.writes[0]).expect("line start");
        assert!(
            (MODE0_EARLIEST..LINE).contains(&into),
            "hblank write {into} ticks into line 120"
        );
    }
}

#[test]
fn rows_carry_the_writer_and_no_dot() {
    let t = timeline(2);
    let r = row(&t, 1, 40);
    assert_eq!(r.writes[0].pc, addr("ScxMid"));
    assert_eq!(r.writes[0].dot, None);
}

#[test]
fn each_frame_lists_its_lines_in_order() {
    let t = timeline(4);
    let lines: Vec<u8> = t
        .lines
        .iter()
        .filter(|r| r.frame == 2)
        .map(|r| r.line.expect("LCD on"))
        .collect();
    assert_eq!(lines, (0..=152).collect::<Vec<u8>>());
    assert!(
        t.lines
            .iter()
            .filter(|r| r.frame == 2)
            .all(|r| r.start.is_some())
    );
}

#[test]
fn the_timeline_names_what_the_core_cannot_report() {
    let t = timeline(1);
    let features: Vec<Feature> = t.gaps.iter().map(|g: &Gap| g.feature).collect();
    assert_eq!(features, vec![Feature::PpuDot, Feature::PpuModeEvents]);
    assert!(t.gaps.iter().all(|g| !g.reason.is_empty()));
}

#[test]
fn the_timeline_moves_the_machine_as_a_run_does() {
    let mut tapped = machine(ROM, Some(SYM));
    let mut plain = machine(ROM, Some(SYM));
    tapped.raster(2).unwrap();
    common::run(plain.core_mut(), Budget::frames(2));
    assert!(tapped.core().now() > Ticks(0));
    assert_eq!(tapped.core().now(), plain.core().now());
    assert_eq!(tapped.core_mut().save(), plain.core_mut().save());
}

#[test]
fn a_cgb_timeline_places_the_writes_on_the_same_lines() {
    let mut m = cgb_machine(ROM, Some(SYM));
    let t = m.raster(4).expect("raster").timeline;
    assert_eq!(written_rows(&t), written_rows(&timeline(4)));
    let starts: Vec<Ticks> = t.lines.iter().filter_map(|r| r.start).collect();
    let phase = starts[0].0 % LINE;
    assert!(starts.iter().all(|s| s.0 % LINE == phase));
}

/// cgb-acid-hell through the real CGB boot ROM (the quick one leaves it
/// spinning with the LCD off): it writes BCPD on the very tick some lines
/// start, which SameBoy's display reaches before it moves to the next line.
fn acid_hell_timeline() -> RasterTimeline {
    let rom = common::corpus_rom("cgb-acid-hell/cgb-acid-hell.gbc");
    let config = CoreConfig::new(Model::CgbE, rom);
    let mut m = Machine::new(Backend::sameboy(), config, None).expect("machine");
    m.raster(400).expect("raster").timeline
}

#[test]
fn a_write_on_a_lines_first_tick_joins_that_line() {
    let t = acid_hell_timeline();
    let mut at_zero = 0;
    for r in &t.lines {
        for w in &r.writes {
            let Some(into) = r.ticks_into_line(w) else {
                continue;
            };
            assert!(
                into < LINE,
                "{:?} pc={:#06x} at {} is {into} ticks into line {:?} of frame {}",
                w.reg,
                w.pc,
                w.at.0,
                r.line,
                r.frame
            );
            at_zero += usize::from(into == 0);
        }
    }
    assert!(at_zero > 0, "no write landed on a line's first tick");
}

#[test]
fn the_timeline_says_how_its_run_ended() {
    let mut m = machine(ROM, Some(SYM));
    let run = m.raster(2).expect("raster");
    assert_eq!(run.stop.reason, StopReason::Budget(BudgetKind::Frames));
    assert_eq!(run.stop.at, m.core().now());

    // An illegal opcode at $0150 locks the CPU before the first frame ends.
    let mut m = machine(&common::program(&[0x00, 0xD3]), None);
    let run = m.raster(10).expect("raster");
    assert_eq!(run.stop.reason, StopReason::Locked { opcode: 0xD3 });
    assert_eq!(run.stop.pc, 0x0151);
}
