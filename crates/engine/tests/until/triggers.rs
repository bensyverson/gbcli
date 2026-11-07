//! Predicates that need more than one trigger, and watches that hit in the
//! same instruction as the write a predicate is waiting for.

use super::*;

#[test]
fn frame_and_ly_stop_at_the_vblank_with_the_lcd_on() {
    // `ly < 200` always holds: the stop is the vblank that makes `frame`
    // true, which is not a scanline change.
    for frame in 1..=2 {
        let mut m = machine(BUSY, None);
        let text = format!("frame >= {frame} && ly < 200");
        let outcome = check(&mut m, &text, &opts(), 200_000, Cadence::Scanline);
        assert_eq!(outcome.stop.time.frame, frame);
    }
}

#[test]
fn frame_and_ly_stop_with_the_lcd_off() {
    // With the LCD off LY stays 0 while frames still end, so only the vblank
    // trigger can see `frame` change. `xor a; ldh [rLCDC], a; .loop: jr .loop`
    let mut m = machine(&program(&[0xAF, 0xE0, 0x40, 0x18, 0xFE]), None);
    step_to(&mut m, 0x0153);
    let outcome = check(
        &mut m,
        "frame >= 3 && ly == 0",
        &opts(),
        30_000,
        Cadence::Scanline,
    );
    assert_eq!(outcome.stop.time.frame, 3);
}

#[test]
fn a_false_read_watch_does_not_hide_the_write_of_the_same_instruction() {
    // `inc [hl]` reads, then writes, $C000: the read watch's hit must not
    // stand in for the write. `ld hl, $C000; ld [hl], 0; .loop: inc [hl];
    // jr .loop`
    let rom = program(&[0x21, 0x00, 0xC0, 0x36, 0x00, 0x34, 0x18, 0xFD]);
    let mut m = machine(&rom, None);
    step_to(&mut m, 0x0155);
    m.add_watch(WatchAccess::Read, at(0xC000), Some("new == 200"), false)
        .unwrap();
    check(&mut m, "[$C000] == 5", &lenient(), 100, Cadence::Writes);
}

#[test]
fn a_false_write_watch_does_not_hide_the_next_write_of_the_same_instruction() {
    // `push bc` writes B to $C00F and C to $C00E. Whichever comes second, a
    // false watch on the other must not hide it: the predicate (any write
    // re-evaluates it) and a watch that holds both see it. `ld sp, $C010;
    // ld bc, 0; .loop: inc b; inc c; inc c; push bc; pop bc; jr .loop`
    #[rustfmt::skip]
    let rom = program(&[
        0x31, 0x10, 0xC0, 0x01, 0x00, 0x00, // ld sp, $C010; ld bc, 0
        0x04, 0x0C, 0x0C, 0xC5, 0xC1,       // $0156 .loop
        0x18, 0xF9,                         // jr .loop
    ]);
    for (watched, other, text, cond) in [
        (0xC00F, 0xC00E, "[$C00E] == 10", "new == 10"),
        (0xC00E, 0xC00F, "[$C00F] == 5", "new == 5"),
    ] {
        let mut m = machine(&rom, None);
        step_to(&mut m, 0x0156);
        m.add_watch(WatchAccess::Write, at(watched), Some("new == 201"), false)
            .unwrap();
        let mut watched = m.try_clone().unwrap();
        check(&mut m, text, &lenient(), 100, Cadence::Writes);

        let id = watched
            .add_watch(WatchAccess::Write, at(other), Some(cond), false)
            .unwrap();
        let stop = watched.run(&lenient()).unwrap().stop;
        assert!(
            matches!(stop.reason, StopReason::Watchpoint { id: hit, addr, .. } if hit == id && addr == other),
            "{:?}",
            stop.reason
        );
        assert_eq!(stop.pc, m.core().registers().pc, "the same instruction");
    }
}

#[test]
fn a_watch_that_holds_still_wins_over_the_predicate() {
    // The same instruction satisfies both; the watch reports, as at a
    // breakpoint.
    let rom = program(&[0x21, 0x00, 0xC0, 0x36, 0x00, 0x34, 0x18, 0xFD]);
    let mut m = machine(&rom, None);
    step_to(&mut m, 0x0155);
    let id = m
        .add_watch(WatchAccess::Read, at(0xC000), Some("new == 4"), false)
        .unwrap();
    let stop = until(&mut m, "[$C000] == 5", &lenient()).stop;
    assert!(
        matches!(stop.reason, StopReason::Watchpoint { id: hit, access: gbcli_core::Access::Read, .. } if hit == id),
        "{:?}",
        stop.reason
    );
}
