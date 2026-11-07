//! Screenshots through the handler: on a run, and the `shot` read verb
//! (design §9, §11 step 6).

use std::fs;
use std::path::Path;

use gbcli_core::VblankKind;
use gbcli_protocol::{
    Crop, ErrorCode, Op, ReadArgs, Response, Rev, RunArgs, ShotArgs, ShotFrame, ShotReadArgs,
    TimeBase, WriteArg, WriteKind,
};

use super::Handler;
use super::test_support::{handler, request, run_frames};
use crate::fixture::Fixture;

fn shot_at(path: &Path) -> ShotArgs {
    ShotArgs {
        path: path.to_string_lossy().into_owned(),
        ..ShotArgs::default()
    }
}

fn boxed(shot: ShotArgs) -> Option<Box<ShotArgs>> {
    Some(Box::new(shot))
}

fn run(h: &mut Handler, args: RunArgs) -> Response {
    h.handle(&request(Op::Run(args)))
}

/// The PNG's width and height from its IHDR chunk.
fn png_size(bytes: &[u8]) -> (u32, u32) {
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "a PNG");
    let be = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap());
    (be(16), be(20))
}

fn contains(haystack: &[u8], needle: &str) -> bool {
    haystack
        .windows(needle.len())
        .any(|w| w == needle.as_bytes())
}

fn reads(h: &mut Handler, exprs: &[&str]) -> Vec<i64> {
    let response = h.handle(&request(Op::Read(ReadArgs {
        reads: exprs.iter().map(|e| e.to_string()).collect(),
        since: None,
        allow_uninit: false,
    })));
    let result = response.outcome.expect("reads");
    result.reads.unwrap().iter().map(|r| r.value).collect()
}

#[test]
fn a_run_with_a_shot_writes_the_png_and_labels_its_frame_from_the_session_base() {
    let f = Fixture::new("shot-run");
    let path = f.dir.join("a.png");
    let mut h = handler(&f);
    let response = run(
        &mut h,
        RunArgs {
            frames: Some(3),
            shot: boxed(ShotArgs {
                scale: Some(2),
                ..shot_at(&path)
            }),
            ..RunArgs::default()
        },
    );
    let result = response.outcome.expect("ran");
    let stop = result.stop.expect("stop");
    let shot = result.shot.expect("a shot");
    let shown = stop.frame_shown.expect("a frame was shown");
    assert_eq!(shot.frame, ShotFrame::Complete(shown));
    assert_eq!(shot.since, TimeBase::Boot);
    assert_eq!(shot.since, stop.time.since);
    // Rebased: from boot, three frames in, not ~90 frames since power-on.
    assert!((0..=3).contains(&shown.n), "{shown:?}");
    assert_eq!((shot.width, shot.height, shot.scale), (320, 288, 2));

    let bytes = fs::read(&path).expect("the PNG was written");
    assert_eq!(png_size(&bytes), (320, 288));
    let label = format!("complete frame {} since boot (vblank normal)", shown.n);
    assert!(contains(&bytes, &label), "the PNG carries `{label}`");
}

#[test]
fn a_crop_is_applied_in_native_pixels_before_the_scale() {
    let f = Fixture::new("shot-crop");
    let path = f.dir.join("c.png");
    let response = run(
        &mut handler(&f),
        RunArgs {
            frames: Some(1),
            shot: boxed(ShotArgs {
                scale: Some(3),
                crop: Some(Crop {
                    x: 8,
                    y: 16,
                    width: 32,
                    height: 24,
                }),
                ..shot_at(&path)
            }),
            ..RunArgs::default()
        },
    );
    let shot = response.outcome.expect("ran").shot.expect("a shot");
    assert_eq!((shot.width, shot.height), (96, 72));
    assert_eq!(png_size(&fs::read(&path).unwrap()), (96, 72));
}

fn lcd_off() -> Vec<WriteArg> {
    vec![WriteArg {
        kind: WriteKind::Bus,
        target: "rLCDC".into(),
        bytes: vec![0],
    }]
}

#[test]
fn a_frame_that_isnt_normal_is_refused_with_exit_1_and_the_any_frame_remedy() {
    let f = Fixture::new("shot-lcd-off");
    let path = f.dir.join("off.png");
    let mut h = handler(&f);
    let response = run(
        &mut h,
        RunArgs {
            frames: Some(2),
            writes: lcd_off(),
            shot: boxed(shot_at(&path)),
            ..RunArgs::default()
        },
    );
    assert_eq!(response.rev, Some(Rev(1)), "the run itself was saved");
    let error = response.outcome.expect_err("an LCD-off frame");
    assert_eq!(error.code, ErrorCode::FrameNotNormal);
    assert_eq!(error.exit, 1);
    assert!(error.remedy.contains("--any-frame"), "{}", error.remedy);
    assert!(error.message.contains("lcd_off"), "{}", error.message);
    assert!(error.stop.is_some(), "the stop rides on the error");
    assert!(!path.exists(), "nothing written");

    let any = run(
        &mut h,
        RunArgs {
            frames: Some(1),
            shot: boxed(ShotArgs {
                any_frame: true,
                ..shot_at(&path)
            }),
            ..RunArgs::default()
        },
    );
    let shot = any.outcome.expect("any frame").shot.unwrap();
    let ShotFrame::Complete(shown) = shot.frame else {
        panic!("{:?}", shot.frame)
    };
    assert_eq!(shown.vblank, VblankKind::LcdOff);
    assert!(path.exists());
}

#[test]
fn a_live_shot_is_labeled_partial_with_the_line_reached() {
    let f = Fixture::new("shot-live");
    let path = f.dir.join("live.png");
    let response = run(
        &mut handler(&f),
        RunArgs {
            until: Some("ly == 67".into()),
            frames: Some(2),
            shot: boxed(ShotArgs {
                live: true,
                scale: Some(1),
                ..shot_at(&path)
            }),
            ..RunArgs::default()
        },
    );
    let result = response.outcome.expect("ran");
    let stop = result.stop.unwrap();
    let shot = result.shot.unwrap();
    let ShotFrame::LivePartial { ppu, after, .. } = &shot.frame else {
        panic!("{:?}", shot.frame)
    };
    assert_eq!(ppu.ly, 67);
    assert_eq!(*after, stop.frame_shown.map(|s| s.n));
    let bytes = fs::read(&path).unwrap();
    assert!(contains(&bytes, "live partial frame: LY 67"));
}

#[test]
fn the_shot_verb_writes_the_head_without_advancing_it() {
    let f = Fixture::new("shot-verb");
    let path = f.dir.join("head.png");
    let mut h = handler(&f);
    let ran = run_frames(&mut h, 2);
    let shown = ran.outcome.expect("ran").stop.unwrap().frame_shown;
    let before = reads(&mut h, &["t", "frame", "pc", "u16[$C000]"]);

    let response = h.handle(&request(Op::Shot(ShotReadArgs {
        shot: shot_at(&path),
        since: None,
    })));
    assert_eq!(response.rev, Some(Rev(1)), "no new head");
    let shot = response.outcome.expect("shot").shot.unwrap();
    assert_eq!(shot.frame, ShotFrame::Complete(shown.unwrap()));
    assert!(path.exists());
    assert_eq!(reads(&mut h, &["t", "frame", "pc", "u16[$C000]"]), before);

    // And from a cold load of the head, not only the warm machine.
    let mut cold = handler(&f);
    assert_eq!(
        reads(&mut cold, &["t", "frame", "pc", "u16[$C000]"]),
        before
    );
    let meta = f.bundle.session("demo").unwrap().read_meta().unwrap();
    assert_eq!(meta.rev, Rev(1));
}

#[test]
fn a_cold_handler_can_shoot_the_head_as_a_stateless_cli_does() {
    let f = Fixture::new("shot-cold");
    let shown = run_frames(&mut handler(&f), 2)
        .outcome
        .expect("ran")
        .stop
        .unwrap()
        .frame_shown;
    let path = f.dir.join("cold.png");
    let mut cold = handler(&f);
    let response = cold.handle(&request(Op::Shot(ShotReadArgs {
        shot: shot_at(&path),
        since: None,
    })));
    assert_eq!(response.rev, Some(Rev(1)), "no new head");
    let shot = response.outcome.expect("shot").shot.unwrap();
    assert_eq!(shot.frame, ShotFrame::Complete(shown.unwrap()));
    assert_eq!(png_size(&fs::read(&path).expect("written")), (640, 576));
}

#[test]
fn a_bad_scale_or_a_missing_directory_is_refused_before_the_run_moves() {
    let f = Fixture::new("shot-bad");
    let mut h = handler(&f);
    for shot in [
        ShotArgs {
            scale: Some(0),
            ..shot_at(&f.dir.join("a.png"))
        },
        ShotArgs {
            scale: Some(17),
            ..shot_at(&f.dir.join("a.png"))
        },
        shot_at(&f.dir.join("no-such-dir").join("a.png")),
    ] {
        let response = run(
            &mut h,
            RunArgs {
                frames: Some(1),
                shot: boxed(shot.clone()),
                ..RunArgs::default()
            },
        );
        assert_eq!(response.rev, Some(Rev(0)), "{shot:?}");
        let error = response.outcome.expect_err("refused");
        assert_eq!(error.code, ErrorCode::BadRequest, "{shot:?}");
        assert!(error.stop.is_none(), "{shot:?}");
    }
    // A crop outside the frame is found only once there is a frame.
    let response = run(
        &mut h,
        RunArgs {
            frames: Some(1),
            shot: boxed(ShotArgs {
                crop: Some(Crop {
                    x: 150,
                    y: 0,
                    width: 20,
                    height: 8,
                }),
                ..shot_at(&f.dir.join("a.png"))
            }),
            ..RunArgs::default()
        },
    );
    let error = response.outcome.expect_err("outside");
    assert_eq!(error.code, ErrorCode::BadRequest);
    assert!(error.remedy.contains("160x144"), "{}", error.remedy);
}
