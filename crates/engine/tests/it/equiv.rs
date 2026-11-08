//! The A/B equivalence check over three builds of one tiny game
//! (`roms/equiv.inc`): the original, a correct optimized rewrite and the
//! rewrite with an off-by-one in its right-edge clamp. Every expected frame is
//! derived from the program and the key timeline's rule (an event at `@n` is
//! applied at the vblank ending frame n, before the ROM's vblank handler),
//! never read off the emulator.

use crate::common;

use common::machine;
use gbcli_core::{ApuWriteEffect, StepUnit};
use gbcli_engine::equiv::{
    AudioCompare, Difference, EquivError, EquivSpec, RamRange, StartField, Verdict, compare,
};
use gbcli_engine::{LoadOptions, Symbols, mem};
use gbcli_protocol::{StepKind, StopReason, parse_keys};

const A: &[u8] = include_bytes!("../roms/equiv_a.gb");
const A_SYM: &str = include_str!("../roms/equiv_a.sym");
const FAST: &[u8] = include_bytes!("../roms/equiv_fast.gb");
const FAST_SYM: &str = include_str!("../roms/equiv_fast.sym");
const BROKEN: &[u8] = include_bytes!("../roms/equiv_broken.gb");
const BROKEN_SYM: &str = include_str!("../roms/equiv_broken.sym");
const HALT: &[u8] = include_bytes!("../roms/halt.gb");

/// The frame-end stop every compared frame should have.
const FRAME_DONE: StopReason = StopReason::StepDone {
    unit: StepKind::Frame,
};

/// RIGHT is held from frame 10, so the block reaches column 19 at the end of
/// frame 29 and the broken clamp lets it reach column 20 at the end of frame 30.
const HOLD_RIGHT: &str = "RIGHT@10";

fn addr(sym: &str, name: &str) -> u16 {
    Symbols::parse(sym).unwrap().get(name).unwrap().addr
}

fn wx() -> Vec<RamRange> {
    vec![RamRange {
        at: "wX".into(),
        len: 1,
    }]
}

fn diverged(verdict: Verdict) -> Box<gbcli_engine::equiv::Divergence> {
    match verdict {
        Verdict::Diverged(d) => d,
        other => panic!("expected a divergence, got {other:?}"),
    }
}

#[test]
fn a_broken_clamp_diverges_in_ram_at_the_frame_it_first_runs() {
    let (mut a, mut b) = (machine(A, Some(A_SYM)), machine(BROKEN, Some(BROKEN_SYM)));
    let start = a.core().now().0;
    let keys = parse_keys(HOLD_RIGHT).unwrap();
    let ram = wx();
    let spec = EquivSpec {
        keys: &keys,
        ram: &ram,
        ..EquivSpec::new(120)
    };
    let d = diverged(compare(&mut a, &mut b, &spec).unwrap());
    assert_eq!(d.frame, 30);
    // Each side found wX through its own .sym.
    assert_eq!(
        d.differences,
        vec![Difference::Ram {
            range: "wX".into(),
            offset: 0,
            a_at: gbcli_protocol::Location {
                addr: addr(A_SYM, "wX"),
                bank: None,
                symbol: None,
            },
            b_at: gbcli_protocol::Location {
                addr: addr(BROKEN_SYM, "wX"),
                bank: None,
                symbol: None,
            },
            a: 19,
            b: 20,
            count: 1,
        }]
    );
    assert_ne!(addr(A_SYM, "wX"), addr(BROKEN_SYM, "wX"));
    assert_eq!(d.a.stop.reason, FRAME_DONE);
    assert_eq!(d.b.stop.reason, FRAME_DONE);
    assert_eq!(d.a.stop.time.ticks, d.b.stop.time.ticks);
    assert_eq!(d.ticks, d.a.stop.time.ticks - start);
}

#[test]
fn without_ram_ranges_the_broken_clamp_shows_on_screen_a_frame_later() {
    // The vblank handler after frame 30 erases column 19 and draws column 20,
    // off screen, so frame 31 is the first drawn without the block.
    let (mut a, mut b) = (machine(A, Some(A_SYM)), machine(BROKEN, Some(BROKEN_SYM)));
    let keys = parse_keys(HOLD_RIGHT).unwrap();
    let spec = EquivSpec {
        keys: &keys,
        ..EquivSpec::new(120)
    };
    let d = diverged(compare(&mut a, &mut b, &spec).unwrap());
    assert_eq!(d.frame, 31);
    assert_eq!(
        d.differences,
        vec![Difference::Pixels {
            x: 19 * 8,
            y: 0,
            a: 3,
            b: 0,
            count: 64,
        }]
    );
}

#[test]
fn either_side_restores_from_the_divergence() {
    let (mut a, mut b) = (machine(A, Some(A_SYM)), machine(BROKEN, Some(BROKEN_SYM)));
    let keys = parse_keys(HOLD_RIGHT).unwrap();
    let ram = wx();
    let spec = EquivSpec {
        keys: &keys,
        ram: &ram,
        ..EquivSpec::new(120)
    };
    let d = diverged(compare(&mut a, &mut b, &spec).unwrap());
    let restored = |rom, sym, cp| {
        let mut m = machine(rom, Some(sym));
        m.restore(cp, &LoadOptions::default()).unwrap();
        m
    };
    let ra = restored(A, A_SYM, &d.a.checkpoint);
    let rb = restored(BROKEN, BROKEN_SYM, &d.b.checkpoint);
    assert_eq!(mem::peek(ra.core(), addr(A_SYM, "wX")).unwrap(), 19);
    assert_eq!(mem::peek(rb.core(), addr(BROKEN_SYM, "wX")).unwrap(), 20);
    assert_eq!(ra.core().now().0, d.a.stop.time.ticks);
}

/// 600 frames of walking into every edge, reversing, and pressing every
/// button, A at several columns (so several notes).
const SCRIPT: &str = "RIGHT@10 A@12 -A@13 DOWN@20 -DOWN@45 A@35 -A@37 -RIGHT@50 \
    UP@60 -UP@70 LEFT@80 A@90 -A@91 -LEFT@100 RIGHT@110 LEFT@115 -LEFT@120 \
    -RIGHT@170 A@175 -A@180 B@200 -B@210 START@220 -START@225 SELECT@230 \
    -SELECT@232 DOWN@240 -DOWN@300 A@310 -A@311 A@312 -A@313 UP@320 RIGHT@330 \
    -UP@400 A@410 -A@415 -RIGHT@420 LEFT@450 -LEFT@500 A@550 -A@560 A@590 -A@599";

#[test]
fn a_known_equivalent_rewrite_passes_600_frames_of_scripted_input() {
    let (mut a, mut b) = (machine(A, Some(A_SYM)), machine(FAST, Some(FAST_SYM)));
    let keys = parse_keys(SCRIPT).unwrap();
    let ram = [
        RamRange {
            at: "wX".into(),
            len: 1,
        },
        RamRange {
            at: "wY".into(),
            len: 1,
        },
        RamRange {
            at: "wFrame".into(),
            len: 2,
        },
        RamRange {
            at: "wHeld".into(),
            len: 2,
        },
    ];
    let spec = EquivSpec {
        keys: &keys,
        ram: &ram,
        ..EquivSpec::new(600)
    };
    assert_eq!(
        compare(&mut a, &mut b, &spec).unwrap(),
        Verdict::Same {
            frames: 600,
            ended: None
        }
    );
    // The same script catches the broken build: it walks into the right edge.
    let mut broken = machine(BROKEN, Some(BROKEN_SYM));
    let d = diverged(compare(&mut a, &mut broken, &spec).unwrap());
    // RIGHT from 10: column 19 at the end of frame 29, 20 (broken) at 30.
    assert_eq!(d.frame, 30);
}

#[test]
fn a_patched_note_diverges_in_the_sound_writes_of_the_frame_that_plays_it() {
    // A at frame 40 with the block at column 0: the main loop of frame 41
    // writes NR21, NR22, then NR23 = NoteTable[0], then NR24.
    let mut a = machine(FAST, Some(FAST_SYM));
    let mut b = machine(FAST, Some(FAST_SYM));
    let table = u32::from(addr(FAST_SYM, "NoteTable"));
    b.patch(table, &[0x07]).unwrap();
    let keys = parse_keys("A@40 -A@42").unwrap();
    let spec = EquivSpec {
        keys: &keys,
        ..EquivSpec::new(60)
    };
    let d = diverged(compare(&mut a, &mut b, &spec).unwrap());
    assert_eq!(d.frame, 41);
    let [
        Difference::Apu {
            index: 2,
            a: Some(wa),
            b: Some(wb),
        },
    ] = d.differences.as_slice()
    else {
        panic!("expected the third write to differ: {:?}", d.differences);
    };
    assert_eq!((wa.addr, wa.value), (0xFF18, 0x06));
    assert_eq!((wb.addr, wb.value), (0xFF18, 0x07));
    assert_eq!(wa.effect, ApuWriteEffect::Applied);
}

#[test]
fn sound_writes_ignore_when_in_the_frame_they_land_unless_timed() {
    // The rewrite reaches Beep sooner in the frame, with the same writes.
    let (mut a, mut b) = (machine(A, Some(A_SYM)), machine(FAST, Some(FAST_SYM)));
    let keys = parse_keys("A@40 -A@42").unwrap();
    let writes = EquivSpec {
        keys: &keys,
        ..EquivSpec::new(60)
    };
    assert_eq!(
        compare(&mut a, &mut b, &writes).unwrap(),
        Verdict::Same {
            frames: 60,
            ended: None
        }
    );
    let timed = EquivSpec {
        audio: AudioCompare::Timed,
        ..writes
    };
    let d = diverged(compare(&mut a, &mut b, &timed).unwrap());
    assert_eq!(d.frame, 41);
    let [
        Difference::Apu {
            index: 0,
            a: Some(wa),
            b: Some(wb),
        },
    ] = d.differences.as_slice()
    else {
        panic!("expected the first write to differ: {:?}", d.differences);
    };
    assert_eq!((wa.addr, wa.value), (wb.addr, wb.value));
    assert!(wb.at < wa.at, "{wa:?} {wb:?}");
}

#[test]
fn machines_at_different_times_are_refused() {
    let mut a = machine(A, Some(A_SYM));
    let mut b = machine(FAST, Some(FAST_SYM));
    b.core_mut().step(StepUnit::Instruction).unwrap();
    let Err(EquivError::Start(mismatch)) = compare(&mut a, &mut b, &EquivSpec::new(10)) else {
        panic!("expected a start mismatch");
    };
    assert_eq!(mismatch.field, StartField::Ticks);
}

#[test]
fn a_range_that_does_not_resolve_on_one_side_is_refused() {
    let (mut a, mut b) = (machine(A, Some(A_SYM)), machine(FAST, Some(FAST_SYM)));
    let ram = [RamRange {
        at: "VBlank.row".into(), // only the original has it
        len: 1,
    }];
    let spec = EquivSpec {
        ram: &ram,
        ..EquivSpec::new(10)
    };
    let Err(EquivError::Range { side, at, .. }) = compare(&mut a, &mut b, &spec) else {
        panic!("expected a range error");
    };
    assert_eq!(
        (side, at.as_str()),
        (gbcli_engine::equiv::Side::B, "VBlank.row")
    );
}

#[test]
fn breakpoints_on_either_machine_are_ignored() {
    let (mut a, mut b) = (machine(A, Some(A_SYM)), machine(FAST, Some(FAST_SYM)));
    let main_loop = gbcli_engine::addr::parse("MainLoop", a.symbols()).unwrap();
    a.add_breakpoint(main_loop, None).unwrap();
    assert_eq!(
        compare(&mut a, &mut b, &EquivSpec::new(20)).unwrap(),
        Verdict::Same {
            frames: 20,
            ended: None
        }
    );
}

#[test]
fn a_machine_that_stops_otherwise_diverges_and_two_that_stop_alike_end_the_check() {
    // halt.gb halts with IE = 0 in its first instructions, and a run reports
    // HALT forever as soon as it sees it, so frame 1 never ends.
    let (mut a, mut b) = (machine(A, Some(A_SYM)), machine(HALT, None));
    let d = diverged(compare(&mut a, &mut b, &EquivSpec::new(10)).unwrap());
    assert_eq!(d.frame, 1);
    assert_eq!(
        d.differences[0],
        Difference::Stop {
            a: FRAME_DONE,
            b: StopReason::HaltForever
        }
    );
    let (mut a, mut b) = (machine(HALT, None), machine(HALT, None));
    assert_eq!(
        compare(&mut a, &mut b, &EquivSpec::new(10)).unwrap(),
        Verdict::Same {
            frames: 0,
            ended: Some(StopReason::HaltForever)
        }
    );
}
