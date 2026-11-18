//! A shot's pixels as data and their fingerprint, through the handler: the
//! same on a run and the `shot` verb, narrowed by the crop. The fingerprint
//! is always there; the rows only when the request asks (`screen`).

use gbcli_core::Model;
use gbcli_protocol::{
    Crop, ErrorCode, FilmstripArgs, Op, Response, RunArgs, ScreenKind, ShotArgs, ShotMade,
    ShotReadArgs, ShotScreen, WriteArg, WriteKind,
};

use super::Handler;
use super::test_support::{handler, request};
use crate::fixture::{Fixture, GBC_PALETTE};

/// A shot that asks for the rows.
fn shot_args(f: &Fixture, name: &str, crop: Option<Crop>) -> ShotArgs {
    ShotArgs {
        screen: true,
        ..bare(f, name, crop)
    }
}

/// A shot that doesn't ask for the rows.
fn bare(f: &Fixture, name: &str, crop: Option<Crop>) -> ShotArgs {
    ShotArgs {
        path: f.dir.join(name).to_string_lossy().into_owned(),
        scale: Some(1),
        crop,
        ..ShotArgs::default()
    }
}

fn rows(made: &ShotMade) -> &ShotScreen {
    made.screen.as_ref().expect("the rows were asked for")
}

fn run_shot(h: &mut Handler, frames: u64, writes: Vec<WriteArg>, shot: ShotArgs) -> ShotMade {
    let response: Response = h.handle(&request(Op::Run(RunArgs {
        frames: Some(frames),
        writes,
        shot: Some(Box::new(shot)),
        ..RunArgs::default()
    })));
    response.outcome.expect("ran").shot.expect("a shot")
}

fn shot_now(h: &mut Handler, shot: ShotArgs) -> ShotMade {
    let response = h.handle(&request(Op::Shot(ShotReadArgs { shot, since: None })));
    response.outcome.expect("shot").shot.expect("a shot")
}

#[test]
fn shots_of_the_same_screen_at_different_frames_share_a_fingerprint() {
    let f = Fixture::new("shot-pixels-same");
    let mut h = handler(&f);
    let early = run_shot(&mut h, 3, vec![], bare(&f, "a.png", None));
    let late = run_shot(&mut h, 5, vec![], bare(&f, "b.png", None));
    assert_ne!(early.frame, late.frame, "two frames");
    assert_eq!(early.pixels.len(), 16, "{:?}", early.pixels);
    assert_eq!(early.pixels, late.pixels);

    // Every shade changes with BGP inverted, so the fingerprint does too.
    let invert = vec![WriteArg {
        kind: WriteKind::Bus,
        target: "rBGP".into(),
        bytes: vec![0x1B],
    }];
    let inverted = run_shot(&mut h, 1, invert, bare(&f, "c.png", None));
    assert_ne!(inverted.pixels, late.pixels);
}

#[test]
fn the_rows_come_only_when_asked_for_and_the_fingerprint_always() {
    let f = Fixture::new("shot-pixels-opt-in");
    let mut h = handler(&f);
    let crop = Some(Crop {
        x: 8,
        y: 16,
        width: 32,
        height: 24,
    });
    for crop in [None, crop] {
        let plain = run_shot(&mut h, 1, vec![], bare(&f, "plain.png", crop));
        assert_eq!(plain.screen, None, "no rows unless asked: {crop:?}");
        let plain_now = shot_now(&mut h, bare(&f, "now.png", crop));
        assert_eq!(plain_now.screen, None, "the verb too: {crop:?}");

        let asked = shot_now(&mut h, shot_args(&f, "asked.png", crop));
        let screen = rows(&asked);
        let (w, h) = crop.map_or((160, 144), |c| (c.width, c.height));
        assert_eq!((screen.width, screen.height), (w, h), "{crop:?}");
        assert_eq!(screen.rows.len(), h as usize);
        assert_eq!(asked.pixels, plain_now.pixels, "the same fingerprint");
    }
}

#[test]
fn a_dmg_screen_is_shades_and_a_crop_narrows_the_data_and_the_fingerprint() {
    let f = Fixture::new("shot-pixels-dmg");
    let mut h = handler(&f);
    let whole = run_shot(&mut h, 3, vec![], shot_args(&f, "whole.png", None));
    let screen = rows(&whole);
    assert_eq!(screen.kind, ScreenKind::Shades);
    assert_eq!((screen.width, screen.height), (160, 144));
    assert_eq!(screen.rows.len(), 144);
    for row in &screen.rows {
        assert_eq!(row.len(), 160);
        assert!(row.bytes().all(|b| (b'0'..=b'3').contains(&b)), "{row}");
    }
    assert_eq!(whole.pixels, gbcli_view::fingerprint(screen));

    let crop = Crop {
        x: 8,
        y: 16,
        width: 32,
        height: 24,
    };
    let part = shot_now(
        &mut h,
        ShotArgs {
            scale: Some(3),
            ..shot_args(&f, "part.png", Some(crop))
        },
    );
    assert_eq!((part.width, part.height), (96, 72), "the PNG is scaled");
    assert_eq!(
        (rows(&part).width, rows(&part).height),
        (32, 24),
        "the data isn't"
    );
    let expected: Vec<String> = screen.rows[16..40]
        .iter()
        .map(|r| r[8..40].to_string())
        .collect();
    assert_eq!(rows(&part).rows, expected);
    assert_eq!(part.pixels, gbcli_view::fingerprint(rows(&part)));
    assert_ne!(part.pixels, whole.pixels);
}

#[test]
fn a_gbc_screen_is_the_colors_the_core_output() {
    let f = Fixture::with_model("shot-pixels-gbc", GBC_PALETTE, Model::CgbE);
    let mut h = handler(&f);
    let whole = run_shot(&mut h, 30, vec![], shot_args(&f, "gbc.png", None));
    let screen = rows(&whole);
    assert_eq!(screen.kind, ScreenKind::Colors);
    assert_eq!((screen.width, screen.height), (160, 144));
    let first = screen.rows[0][..6].to_string();
    // $021F (red 31, green 16, blue 0) through the core's conversion.
    let channel = |i: usize| u8::from_str_radix(&first[i..i + 2], 16).unwrap();
    assert!(
        channel(0) > 0xC0 && channel(4) < 0x40,
        "orange, not {first}"
    );
    for row in &screen.rows {
        assert_eq!(row, &first.repeat(160));
    }

    let crop = Crop {
        x: 0,
        y: 0,
        width: 2,
        height: 1,
    };
    let part = shot_now(&mut h, shot_args(&f, "gbc-part.png", Some(crop)));
    assert_eq!(rows(&part).rows, [first.repeat(2)]);
    assert_ne!(part.pixels, whole.pixels);
}

/// A PNG's pixels as 0xRRGGBB, and its width.
fn decode_png(path: &str) -> (Vec<u32>, u32) {
    let file = std::fs::File::open(path).expect("written");
    let mut decoder = png::Decoder::new(file);
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    buf.truncate(info.buffer_size());
    let px = buf
        .chunks(3)
        .map(|c| u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2]))
        .collect();
    (px, info.width)
}

/// On the color models the rows are the PNG's pixels, the SGB's border
/// included (256x224).
#[test]
fn on_color_models_the_rows_are_the_pngs_pixels() {
    for (name, rom, model, size) in [
        ("shot-png-gbc", GBC_PALETTE, Model::CgbE, (160, 144)),
        ("shot-png-sgb", crate::fixture::V1, Model::Sgb, (256, 224)),
    ] {
        let f = Fixture::with_model(name, rom, model);
        let made = run_shot(&mut handler(&f), 30, vec![], shot_args(&f, "a.png", None));
        let screen = rows(&made);
        assert_eq!(screen.kind, ScreenKind::Colors, "{name}");
        assert_eq!((screen.width, screen.height), size, "{name}");
        let (png, width) = decode_png(&made.path);
        assert_eq!(width, screen.width, "{name}");
        let from_rows: Vec<u32> = screen
            .rows
            .iter()
            .flat_map(|r| {
                (0..r.len() / 6).map(|i| u32::from_str_radix(&r[i * 6..i * 6 + 6], 16).unwrap())
            })
            .collect();
        assert_eq!(from_rows, png, "{name}: the rows and the PNG disagree");
    }
}

/// A crop reaching the screen's right and bottom edges is taken; one pixel
/// further is refused, on a shot and on a filmstrip.
#[test]
fn a_crop_may_reach_the_screens_edge_but_not_pass_it() {
    let f = Fixture::new("shot-crop-edge");
    let mut h = handler(&f);
    let edge = Crop {
        x: 152,
        y: 136,
        width: 8,
        height: 8,
    };
    let made = run_shot(&mut h, 3, vec![], shot_args(&f, "e.png", Some(edge)));
    assert_eq!(rows(&made).rows.len(), 8);
    let past = Crop { x: 153, ..edge };
    let refused = h.handle(&request(Op::Shot(ShotReadArgs {
        shot: shot_args(&f, "p.png", Some(past)),
        since: None,
    })));
    assert_eq!(refused.error().map(|e| e.code), Some(ErrorCode::BadRequest));
    let strip = h.handle(&request(Op::Run(RunArgs {
        frames: Some(2),
        filmstrip: Some(Box::new(FilmstripArgs {
            path: f.dir.join("s.png").to_string_lossy().into_owned(),
            every: 1,
            scale: None,
            crop: Some(past),
        })),
        ..RunArgs::default()
    })));
    assert_eq!(strip.error().map(|e| e.code), Some(ErrorCode::BadRequest));
}
