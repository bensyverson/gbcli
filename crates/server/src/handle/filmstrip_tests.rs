//! Filmstrips through the handler: frames collected during a run (design §9).

use std::fs;
use std::path::Path;

use gbcli_core::VblankKind;
use gbcli_protocol::{
    ErrorCode, FilmstripArgs, Op, Response, RunArgs, ShotArgs, ShotFrame, WriteArg, WriteKind,
};

use super::Handler;
use super::test_support::{handler, request};
use crate::fixture::Fixture;

fn run(h: &mut Handler, args: RunArgs) -> Response {
    h.handle(&request(Op::Run(args)))
}

fn strip(f: &Fixture, every: u64) -> Option<Box<FilmstripArgs>> {
    Some(Box::new(FilmstripArgs {
        path: f.dir.join("strip.png").to_string_lossy().into_owned(),
        every,
        scale: None,
    }))
}

/// A PNG's pixels as RGB rows, and its width.
fn decode(path: &Path) -> (Vec<u8>, usize) {
    let mut decoder = png::Decoder::new(fs::File::open(path).expect("written"));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    assert_eq!(
        info.color_type,
        png::ColorType::Rgb,
        "8-bit RGB after expansion"
    );
    buf.truncate(info.buffer_size());
    (buf, info.width as usize)
}

#[test]
fn a_filmstrips_frames_and_labels_match_frames_captured_one_run_at_a_time() {
    let one = Fixture::new("filmstrip-one");
    let made = run(
        &mut handler(&one),
        RunArgs {
            frames: Some(6),
            filmstrip: strip(&one, 2),
            ..RunArgs::default()
        },
    )
    .outcome
    .expect("ran")
    .filmstrip
    .expect("a filmstrip");
    assert_eq!(made.frames.len(), 3);
    assert_eq!((made.width, made.height, made.scale), (3 * 160 + 2, 144, 1));
    let (strip_pixels, strip_width) = decode(Path::new(&made.path));

    let apart = Fixture::new("filmstrip-apart");
    let mut h = handler(&apart);
    for (i, frame) in made.frames.iter().enumerate() {
        let path = apart.dir.join(format!("{i}.png"));
        let shot = run(
            &mut h,
            RunArgs {
                frames: Some(2),
                shot: Some(Box::new(ShotArgs {
                    path: path.to_string_lossy().into_owned(),
                    scale: Some(1),
                    any_frame: true,
                    ..ShotArgs::default()
                })),
                ..RunArgs::default()
            },
        )
        .outcome
        .expect("ran")
        .shot
        .expect("a shot");
        assert_eq!(*frame, shot.frame, "frame {i}");
        let label = shot.frame.label(&made.since);
        let bytes = fs::read(&made.path).unwrap();
        let text = format!("[{i}] {label}");
        assert!(
            bytes.windows(text.len()).any(|w| w == text.as_bytes()),
            "the PNG labels frame {i} `{text}`"
        );
        let (pixels, width) = decode(&path);
        assert_eq!(width, 160);
        for y in 0..144 {
            let at = (y * strip_width + i * 161) * 3;
            assert_eq!(
                strip_pixels[at..at + 160 * 3],
                pixels[y * 160 * 3..(y + 1) * 160 * 3],
                "frame {i}, row {y}"
            );
        }
    }
}

#[test]
fn frames_that_arent_normal_are_kept_and_labeled() {
    let f = Fixture::new("filmstrip-lcd-off");
    let made = run(
        &mut handler(&f),
        RunArgs {
            frames: Some(2),
            writes: vec![WriteArg {
                kind: WriteKind::Bus,
                target: "rLCDC".into(),
                bytes: vec![0],
            }],
            filmstrip: strip(&f, 1),
            ..RunArgs::default()
        },
    )
    .outcome
    .expect("ran")
    .filmstrip
    .expect("a filmstrip");
    assert_eq!(made.frames.len(), 2);
    for frame in &made.frames {
        let ShotFrame::Complete(shown) = frame else {
            panic!("{frame:?}")
        };
        assert_eq!(shown.vblank, VblankKind::LcdOff);
    }
    let bytes = fs::read(&made.path).unwrap();
    assert!(bytes.windows(13).any(|w| w == b"vblank lcd_of"));
}

#[test]
fn frames_past_the_cap_are_counted_and_warned_about() {
    let f = Fixture::new("filmstrip-cap");
    let response = run(
        &mut handler(&f),
        RunArgs {
            frames: Some(20),
            filmstrip: strip(&f, 1),
            ..RunArgs::default()
        },
    );
    let made = response
        .outcome
        .expect("ran")
        .filmstrip
        .expect("a filmstrip");
    assert_eq!(made.frames.len(), FilmstripArgs::MAX_FRAMES);
    assert_eq!(made.beyond_cap, 4);
    assert!(
        response.warnings.iter().any(|w| w.contains("--every")),
        "{:?}",
        response.warnings
    );
}

#[test]
fn a_filmstrip_every_zero_frames_is_refused_before_the_run() {
    let f = Fixture::new("filmstrip-zero");
    let failure = run(
        &mut handler(&f),
        RunArgs {
            frames: Some(2),
            filmstrip: strip(&f, 0),
            ..RunArgs::default()
        },
    )
    .outcome
    .unwrap_err();
    assert_eq!(failure.code, ErrorCode::BadRequest);
    assert!(failure.stop.is_none());
    assert!(failure.remedy.contains("--every"), "{}", failure.remedy);
}
