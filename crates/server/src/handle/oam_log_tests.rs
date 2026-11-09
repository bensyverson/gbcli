//! `run --oam-log`: OAM at every vblank the run passes, so a multiplexer's
//! coverage is one run rather than a process per frame.

use gbcli_protocol::{Op, RunArgs, TimeBase, WriteArg, WriteKind};

use super::test_support::{handler, request, run_frames};
use crate::fixture::Fixture;

/// Twelve objects on lines 64-71 (the last two dropped by the 10-per-line
/// limit), the rest of OAM off screen.
fn crowded_oam() -> Vec<u8> {
    let mut oam = vec![0u8; 160];
    for i in 0..12u8 {
        let at = 4 * usize::from(i);
        oam[at..at + 4].copy_from_slice(&[80, 8 + 8 * i, i, 0]);
    }
    oam
}

#[test]
fn the_oam_log_lists_each_frames_objects_on_screen_and_its_drops() {
    let f = Fixture::new("oam-log");
    let mut h = handler(&f);
    run_frames(&mut h, 2).outcome.expect("booted");
    let response = h.handle(&request(Op::Run(RunArgs {
        frames: Some(3),
        oam_log: true,
        writes: vec![WriteArg {
            kind: WriteKind::Poke,
            target: "oam:0".into(),
            bytes: crowded_oam(),
        }],
        ..RunArgs::default()
    })));
    let result = response.outcome.expect("ran");
    let stop = result.stop.expect("stop");
    let log = result.oam_log.expect("an OAM log");
    assert_eq!(log.since, TimeBase::Boot);
    assert_eq!(log.frames.len(), 3, "one per vblank the run passed");
    let numbers: Vec<i64> = log.frames.iter().map(|f| f.frame).collect();
    let last = stop.frame_shown.expect("a frame").n;
    assert_eq!(numbers, vec![last - 2, last - 1, last]);
    for frame in &log.frames {
        assert_eq!(frame.height, 8);
        assert!(!frame.lcd_off);
        assert_eq!(frame.objects.len(), 12, "off-screen objects are left out");
        assert!(frame.objects.iter().all(|o| o.on_screen));
        assert_eq!(frame.dropped(), vec![10, 11]);
        assert_eq!(frame.objects[3].tile, 3);
        assert_eq!(frame.objects[3].screen_x, 24);
    }
}
