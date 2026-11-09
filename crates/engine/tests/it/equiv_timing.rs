//! The equivalence check across builds whose timing differs: the original
//! (`roms/equiv_a.asm`) against the same program with a longer start-up
//! (`roms/equiv_slow_start.asm`, 105,008 T more before the LCD comes on).
//! Their RAM matches frame for frame, but every vblank lands at another time,
//! so their screens differ: the check says the frames are offset, and with
//! the screen off a RAM compare stands alone.

use crate::common;

use common::machine;
use gbcli_engine::equiv::{
    AudioCompare, Difference, EquivError, EquivSpec, FrameOffset, RamRange, ScreenCompare, Side,
    Verdict, compare,
};
use gbcli_protocol::parse_keys;

const A: &[u8] = include_bytes!("../roms/equiv_a.gb");
const A_SYM: &str = include_str!("../roms/equiv_a.sym");
const SLOW: &[u8] = include_bytes!("../roms/equiv_slow_start.gb");
const SLOW_SYM: &str = include_str!("../roms/equiv_slow_start.sym");
const HALT: &[u8] = include_bytes!("../roms/halt.gb");

/// The slow build's extra start-up: `ld bc` (12 T), then 3,750 passes of a
/// 28 T loop whose last `jr` falls through (8 T, not 12).
const DELAY_T: u64 = 12 + 3_750 * 28 - 4;
/// A frame with the LCD on.
const FRAME_T: u64 = 70_224;

fn ram() -> Vec<RamRange> {
    ["wX", "wY"]
        .iter()
        .map(|at| RamRange {
            at: (*at).into(),
            len: 1,
        })
        .collect()
}

#[test]
fn a_longer_start_up_diverges_on_screen_and_says_the_frames_are_offset() {
    let (mut a, mut b) = (machine(A, Some(A_SYM)), machine(SLOW, Some(SLOW_SYM)));
    let starts = (a.core().now().0, b.core().now().0);
    let keys = parse_keys("RIGHT@10 DOWN@20").unwrap();
    let ram = ram();
    let spec = EquivSpec {
        keys: &keys,
        ram: &ram,
        ..EquivSpec::new(60)
    };
    let d = match compare(&mut a, &mut b, &spec).unwrap() {
        Verdict::Diverged(d) => d,
        other => panic!("expected a divergence, got {other:?}"),
    };
    assert!(
        d.differences
            .iter()
            .all(|x| matches!(x, Difference::Pixels { .. })),
        "only the screen differs: {:?}",
        d.differences
    );
    // B's LCD comes on DELAY_T later, so its vblanks land at other times. How
    // far apart depends on how the core counts frames while the LCD is off,
    // so the offset is checked against the two stops: the difference their
    // times since each side's start show, and the side that got there first.
    let offset = d.frame_offset.expect("the frames are offset");
    let elapsed = (
        d.a.stop.time.ticks - starts.0,
        d.b.stop.time.ticks - starts.1,
    );
    assert_eq!(offset.ticks, elapsed.0.abs_diff(elapsed.1), "{offset:?}");
    let earlier = if elapsed.0 < elapsed.1 {
        Side::A
    } else {
        Side::B
    };
    assert_eq!(offset.earlier, earlier, "{offset:?}");
    assert_eq!(offset.ticks, offset.t * 2, "single speed: 2 ticks a T");
    assert!(offset.t < DELAY_T + FRAME_T, "{offset:?}");
}

#[test]
fn with_the_screen_off_the_ram_compare_stands_alone() {
    let (mut a, mut b) = (machine(A, Some(A_SYM)), machine(SLOW, Some(SLOW_SYM)));
    let keys = parse_keys("RIGHT@10 DOWN@20").unwrap();
    let ram = ram();
    let spec = EquivSpec {
        keys: &keys,
        ram: &ram,
        screen: ScreenCompare::Off,
        ..EquivSpec::new(60)
    };
    assert_eq!(
        compare(&mut a, &mut b, &spec).unwrap(),
        Verdict::Same {
            frames: 60,
            ended: None
        }
    );
}

#[test]
fn identical_builds_diverge_with_no_frame_offset() {
    let (mut a, mut b) = (machine(A, Some(A_SYM)), machine(A, Some(A_SYM)));
    let ram = vec![RamRange {
        at: "wX".into(),
        len: 1,
    }];
    let keys = parse_keys("RIGHT@2").unwrap();
    // Side B's right-edge clamp (`cp 19`, at Move+11) becomes `cp 0`, so the
    // block never moves right on B.
    let addr = gbcli_engine::Symbols::parse(A_SYM)
        .unwrap()
        .get("Move")
        .unwrap()
        .addr;
    b.patch(u32::from(addr) + 11, &[0]).unwrap();
    let spec = EquivSpec {
        keys: &keys,
        ram: &ram,
        ..EquivSpec::new(10)
    };
    let d = match compare(&mut a, &mut b, &spec).unwrap() {
        Verdict::Diverged(d) => d,
        other => panic!("expected a divergence, got {other:?}"),
    };
    assert_eq!(d.frame_offset, None::<FrameOffset>);
}

/// From the rev-cli review's probe: side B halts forever mid-frame, so its
/// frame never ended and no frame offset (nor the start-up-timing remedy
/// built on it) describes the divergence; the stop does.
#[test]
fn a_side_that_stopped_mid_frame_has_no_frame_offset() {
    let (mut a, mut b) = (machine(A, Some(A_SYM)), machine(HALT, None));
    let d = match compare(&mut a, &mut b, &EquivSpec::new(10)).unwrap() {
        Verdict::Diverged(d) => d,
        other => panic!("expected a divergence, got {other:?}"),
    };
    assert!(matches!(d.differences[0], Difference::Stop { .. }));
    // The pixels differ too, which is when the remedy would be offered.
    assert!(
        d.differences
            .iter()
            .any(|x| matches!(x, Difference::Pixels { .. }))
    );
    assert_eq!(d.frame_offset, None::<FrameOffset>);
}

#[test]
fn nothing_left_to_compare_is_refused() {
    let (mut a, mut b) = (machine(A, Some(A_SYM)), machine(A, Some(A_SYM)));
    let spec = EquivSpec {
        screen: ScreenCompare::Off,
        audio: AudioCompare::Off,
        ..EquivSpec::new(10)
    };
    assert_eq!(
        compare(&mut a, &mut b, &spec),
        Err(EquivError::NothingCompared)
    );
    // The screen alone, or a sound compare alone, is enough.
    for spec in [
        EquivSpec {
            audio: AudioCompare::Off,
            ..EquivSpec::new(10)
        },
        EquivSpec {
            screen: ScreenCompare::Off,
            ..EquivSpec::new(10)
        },
    ] {
        assert!(compare(&mut a, &mut b, &spec).is_ok(), "{spec:?}");
    }
}
