//! `equiv` across builds whose timing differs (the raycaster review, C-1 and
//! C-2): side A is `equiv_a`, side B the same program with a longer start-up,
//! so B's frames end at other times. Their RAM matches frame for frame; their
//! screens don't.

use gbcli_protocol::{
    AudioCompareName, ComparedMade, EquivArgs, EquivSide, ErrorCode, Op, RamRangeArg, Response,
    ScreenCompareName, SideName, parse_keys,
};

use super::Handler;
use super::test_support::{handler, request};
use crate::fixture::{EQUIV_A, EQUIV_A_SYM, Fixture, SLOW_START, SLOW_START_SYM};

fn equiv(h: &mut Handler, args: EquivArgs) -> Response {
    h.handle(&request(Op::Equiv(Box::new(args))))
}

fn ram() -> Vec<RamRangeArg> {
    ["wX", "wY"]
        .iter()
        .map(|at| RamRangeArg {
            at: (*at).into(),
            len: 1,
        })
        .collect()
}

/// The slow build as side B, comparing wX and wY under a held d-pad.
fn slow(name: &str, screen: Option<ScreenCompareName>) -> (Fixture, EquivArgs) {
    let f = Fixture::with_rom_sym(name, EQUIV_A, EQUIV_A_SYM);
    std::fs::write(f.dir.join("slow.gb"), SLOW_START).expect("rom b");
    std::fs::write(f.dir.join("slow.sym"), SLOW_START_SYM).expect("sym b");
    let args = EquivArgs {
        b: EquivSide {
            rom: Some("slow.gb".into()),
            ..EquivSide::default()
        },
        frames: 60,
        keys: parse_keys("RIGHT@10 DOWN@20").unwrap(),
        ram: ram(),
        screen,
        ..EquivArgs::default()
    };
    (f, args)
}

#[test]
fn with_the_screen_off_a_ram_compare_stands_alone_and_the_verdict_says_so() {
    let (f, args) = slow("equiv-screen-off", Some(ScreenCompareName::Off));
    let made = equiv(&mut handler(&f), args)
        .outcome
        .expect("the same")
        .equiv
        .expect("a verdict");
    assert_eq!(made.frames, 60);
    assert_eq!(
        made.compared,
        ComparedMade {
            screen: false,
            ram: ram(),
            audio: AudioCompareName::Writes,
        }
    );
}

/// The screens differ only because B's frames end at other times: the
/// divergence carries the offset, the message says it, and the remedy names
/// the compare that doesn't depend on timing.
#[test]
fn a_screen_divergence_between_offset_frames_says_so() {
    let (f, args) = slow("equiv-offset", None);
    let error = equiv(&mut handler(&f), args).outcome.expect_err("diverged");
    assert_eq!(error.code, ErrorCode::PredicateFalse);
    let d = error
        .result
        .as_ref()
        .and_then(|r| r.equiv.as_ref())
        .and_then(|e| e.divergence.as_ref())
        .expect("a divergence");
    let offset = d.frame_offset.expect("the frames are offset");
    // Each side starts at its own boot handoff, which its stop's `t` counts
    // from, so the offset is the difference of the two stop rows' `t`.
    let (ta, tb) = (d.a.stop.time.t, d.b.stop.time.t);
    assert_eq!(offset.t, ta.abs_diff(tb), "{offset:?}");
    let earlier = if tb < ta { SideName::B } else { SideName::A };
    assert_eq!(offset.earlier, earlier);
    let side = if earlier == SideName::B { "B" } else { "A" };
    let says = format!(
        "side {side}'s frame {} ended {} T sooner",
        d.frame, offset.t
    );
    assert!(error.message.contains(&says), "{}", error.message);
    assert!(error.remedy.contains("--screen off"), "{}", error.remedy);
    assert!(error.remedy.contains("--ram"), "{}", error.remedy);
}

#[test]
fn nothing_left_to_compare_is_refused_naming_what_to_turn_on() {
    let (f, mut args) = slow("equiv-nothing", Some(ScreenCompareName::Off));
    args.ram.clear();
    args.audio = Some(AudioCompareName::Off);
    let error = equiv(&mut handler(&f), args.clone())
        .outcome
        .expect_err("refused");
    assert_eq!(error.code, ErrorCode::BadRequest);
    assert!(
        error.message.contains("nothing to compare"),
        "{}",
        error.message
    );
    assert!(error.remedy.contains("--ram"), "{}", error.remedy);
    // The sound writes alone are something to compare.
    args.audio = Some(AudioCompareName::Writes);
    equiv(&mut handler(&f), args).outcome.expect("the same");
}
