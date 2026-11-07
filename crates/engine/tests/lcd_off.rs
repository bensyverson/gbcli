//! Turning the LCD off outside VBlank can damage a real DMG's screen, so a
//! run or step that does it warns, naming the PC, the line and the frame
//! (leaf jXavVc). The demo is `help traps`'s, crates/gbcli/tests/traps/.

mod common;

use common::machine;
use gbcli_core::{Budget, Region, Registers};
use gbcli_engine::cond::EvalWarning;
use gbcli_engine::{LcdOffOutsideVblank, RunOptions};
use gbcli_protocol::StepKind;

const LCD_OFF: &[u8] = include_bytes!("../../gbcli/tests/traps/lcd_off.gb");
const LCD_OFF_SYM: &str = include_str!("../../gbcli/tests/traps/lcd_off.sym");

/// The demo after a boot that leaves the LCD on, as the real boot ROM does
/// (the tests' quick boot ROM leaves it off, where LY stays 0).
fn demo() -> gbcli_engine::Machine {
    let mut m = machine(LCD_OFF, Some(LCD_OFF_SYM));
    m.core_mut().bus_write(0xFF40, 0x91).unwrap();
    m
}

fn lcd_offs(warnings: &[EvalWarning]) -> Vec<&LcdOffOutsideVblank> {
    warnings
        .iter()
        .filter_map(|w| match w {
            EvalWarning::LcdOffOutsideVblank(off) => Some(off),
            _ => None,
        })
        .collect()
}

#[test]
fn turning_the_lcd_off_mid_frame_warns_and_in_vblank_does_not() {
    let mut m = demo();
    let outcome = m.run(&RunOptions::new(Budget::frames(4))).expect("run");
    let offs = lcd_offs(&outcome.warnings);
    // Two LCD-off writes: line 60 (OffMidFrame) and line 144 (OffInVBlank).
    assert_eq!(offs.len(), 1, "{:?}", outcome.warnings);
    let off = offs[0];
    assert_eq!(off.pc, 0x015C);
    assert_eq!(off.symbol.as_deref(), Some("OffMidFrame"));
    assert_eq!(off.line, 60);
    assert_eq!(off.more, 0);
    assert_eq!(off.frame, 0, "the first frame after the handoff");
    let text = outcome.warnings[0].to_string();
    for part in [
        "the LCD was turned off outside VBlank",
        "$015C (OffMidFrame)",
        "line 60",
        &format!("frame {}", off.frame),
        "wait for LY >= 144 or use the VBlank interrupt",
    ] {
        assert!(text.contains(part), "{part:?} in {text}");
    }
}

#[test]
fn the_warning_is_deterministic() {
    let run = || {
        let mut m = demo();
        m.run(&RunOptions::new(Budget::frames(4)))
            .expect("run")
            .warnings
    };
    assert_eq!(run(), run());
}

#[test]
fn a_step_over_the_write_warns_too() {
    let mut m = demo();
    m.run_to(
        gbcli_engine::addr::parse("OffMidFrame", m.symbols()).unwrap(),
        &RunOptions::new(Budget::frames(4)),
    )
    .expect("run to the write");
    let outcome = m
        .step(StepKind::Instruction, &RunOptions::new(Budget::frames(1)))
        .expect("step");
    let offs = lcd_offs(&outcome.warnings);
    assert_eq!(offs.len(), 1, "{:?}", outcome.warnings);
    assert_eq!((offs[0].pc, offs[0].line), (0x015C, 60));
}

/// A loop in WRAM that turns the LCD on and straight off again, on line 0,
/// every pass: one warning for the run, counting the rest.
#[test]
fn many_in_one_run_are_one_warning_with_a_count() {
    let mut m = machine(LCD_OFF, Some(LCD_OFF_SYM));
    // ld a,$91 / ldh [$40],a / ld a,$11 / ldh [$40],a / jr -10
    let code = [0x3E, 0x91, 0xE0, 0x40, 0x3E, 0x11, 0xE0, 0x40, 0x18, 0xF6];
    m.core_mut().poke(Region::Wram, 0, &code).unwrap();
    // Off before the run starts (not the program's write), so each pass
    // turns it on and then off again on line 0.
    m.core_mut().bus_write(0xFF40, 0x11).unwrap();
    let regs = Registers {
        pc: 0xC000,
        ..m.core().registers()
    };
    m.core_mut().set_registers(&regs);
    let outcome = m
        .run(&RunOptions::new(Budget::instructions(25)))
        .expect("run");
    let offs = lcd_offs(&outcome.warnings);
    assert_eq!(offs.len(), 1, "{:?}", outcome.warnings);
    assert_eq!((offs[0].pc, offs[0].line), (0xC006, 0));
    assert_eq!(offs[0].more, 4, "five passes of five instructions");
    let text = outcome.warnings[0].to_string();
    assert!(text.contains("and 4 more times"), "{text}");
}
