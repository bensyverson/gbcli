//! Writes the hardware ignored: a run or step whose program writes VRAM in
//! mode 3, or OAM in modes 2-3, warns once per kind with the count and the
//! first's PC, symbol, line and frame (leaf si5fnG). The VRAM demo is
//! `help traps`'s, crates/gbcli/tests/traps/.

use crate::common;

use common::{cgb_machine, machine, program};
use gbcli_core::{Budget, IgnoredWriteKind};
use gbcli_engine::cond::EvalWarning;
use gbcli_engine::{Machine, RunOptions, WritesIgnored};
use gbcli_protocol::StepKind;

const VRAM_MODE3: &[u8] = include_bytes!("../../../gbcli/tests/traps/vram_mode3.gb");
const VRAM_MODE3_SYM: &str = include_str!("../../../gbcli/tests/traps/vram_mode3.sym");
/// `Dropped` in the demo: its `ld [$8000],a` in mode 3.
const DROPPED: u16 = 0x017A;

/// The demo after a boot that leaves the LCD on, as the real boot ROM does.
fn demo() -> Machine {
    let mut m = machine(VRAM_MODE3, Some(VRAM_MODE3_SYM));
    m.core_mut().bus_write(0xFF40, 0x91).unwrap();
    m
}

fn ignored(warnings: &[EvalWarning]) -> Vec<&WritesIgnored> {
    warnings
        .iter()
        .filter_map(|w| match w {
            EvalWarning::WritesIgnored(found) => Some(found),
            _ => None,
        })
        .collect()
}

#[test]
fn a_vram_write_in_mode_3_warns_with_its_pc_symbol_line_and_frame() {
    let mut m = demo();
    let outcome = m.run(&RunOptions::new(Budget::frames(4))).expect("run");
    let found = ignored(&outcome.warnings);
    assert_eq!(found.len(), 1, "{:?}", outcome.warnings);
    let w = found[0];
    assert_eq!(w.kind, IgnoredWriteKind::Vram);
    assert_eq!(w.count, 1, "the demo's HBlank write lands");
    assert_eq!((w.pc, w.addr), (DROPPED, 0x8000));
    assert_eq!(w.symbol.as_deref(), Some("Dropped"));
    assert_eq!(w.line, 10);
    let text = w.to_string();
    for part in [
        "1 VRAM write ignored in mode 3",
        "first at $017A (Dropped), line 10 of frame ",
        &format!("frame {}", w.frame),
        "write it in HBlank or VBlank, or with the LCD off",
    ] {
        assert!(text.contains(part), "{part:?} in {text}");
    }
}

/// `ld a,$91; ldh [rLCDC],a`, then waits for line 2 and for each `mode` in
/// turn, writing `$FF` to `addr` after each wait; then idles.
fn writes_after(waits: &[(u8, u16)]) -> Vec<u8> {
    let mut code = vec![0x3E, 0x91, 0xE0, 0x40];
    code.extend_from_slice(&[0xF0, 0x44, 0xFE, 0x02, 0x20, 0xFA]); // wait for LY 2
    for &(mode, addr) in waits {
        // .w: ldh a,[rSTAT]; and 3; cp mode; jr nz,.w
        code.extend_from_slice(&[0xF0, 0x41, 0xE6, 0x03, 0xFE, mode, 0x20, 0xF8]);
        let [lo, hi] = addr.to_le_bytes();
        code.extend_from_slice(&[0x3E, 0xFF, 0xEA, lo, hi]); // ld a,$FF; ld [addr],a
    }
    code.extend_from_slice(&[0x18, 0xFE]);
    program(&code)
}

#[test]
fn vram_writes_in_hblank_and_vblank_dont_warn() {
    let mut m = machine(&writes_after(&[(0, 0x9800), (1, 0x9801)]), None);
    let outcome = m.run(&RunOptions::new(Budget::frames(3))).expect("run");
    assert!(
        ignored(&outcome.warnings).is_empty(),
        "{:?}",
        outcome.warnings
    );
    assert_eq!(m.core_mut().bus_read(0x9801).unwrap(), 0xFF, "it ran");
}

#[test]
fn oam_writes_in_modes_2_and_3_warn_the_same_way() {
    // Mode 3 (dropped), HBlank (lands), the next line's mode 2 (dropped).
    let rom = writes_after(&[(3, 0xFE00), (0, 0xFE01), (2, 0xFE02)]);
    let mut m = machine(&rom, None);
    let outcome = m.run(&RunOptions::new(Budget::frames(3))).expect("run");
    let found = ignored(&outcome.warnings);
    assert_eq!(found.len(), 1, "{:?}", outcome.warnings);
    let w = found[0];
    assert_eq!(
        (w.kind, w.count, w.addr, w.line),
        (IgnoredWriteKind::Oam, 2, 0xFE00, 2)
    );
    assert_eq!(w.symbol, None);
    let text = w.to_string();
    assert!(
        text.starts_with(&format!(
            "2 OAM writes ignored in modes 2-3, first at ${:04X}, line 2 of frame ",
            w.pc
        )),
        "{text}"
    );
}

#[test]
fn both_kinds_in_one_run_are_two_warnings_vram_first() {
    let rom = writes_after(&[(3, 0xFE00), (3, 0x9800)]);
    let mut m = machine(&rom, None);
    let outcome = m.run(&RunOptions::new(Budget::frames(3))).expect("run");
    let kinds: Vec<_> = ignored(&outcome.warnings).iter().map(|w| w.kind).collect();
    assert_eq!(kinds, [IgnoredWriteKind::Vram, IgnoredWriteKind::Oam]);
}

#[test]
fn the_warning_is_deterministic() {
    let run = || {
        demo()
            .run(&RunOptions::new(Budget::frames(4)))
            .expect("run")
            .warnings
    };
    assert_eq!(run(), run());
}

#[test]
fn a_step_over_the_write_warns_and_numbers_its_frame_as_a_stop_record_does() {
    for (name, mut m) in [
        ("dmg", demo()),
        ("cgb", {
            let mut m = cgb_machine(VRAM_MODE3, Some(VRAM_MODE3_SYM));
            m.core_mut().bus_write(0xFF40, 0x91).unwrap();
            m
        }),
    ] {
        let at = gbcli_engine::addr::parse("Dropped", m.symbols()).unwrap();
        let stop = m
            .run_to(at, &RunOptions::new(Budget::frames(4)))
            .expect("run to the write")
            .stop;
        let outcome = m
            .step(StepKind::Instruction, &RunOptions::new(Budget::frames(1)))
            .expect("step");
        let found = ignored(&outcome.warnings);
        assert_eq!(found.len(), 1, "{name}: {:?}", outcome.warnings);
        assert_eq!((found[0].pc, found[0].count), (DROPPED, 1), "{name}");
        assert_eq!(found[0].frame, stop.time.frame, "{name}");
    }
}
