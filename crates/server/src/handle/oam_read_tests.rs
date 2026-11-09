//! The structured OAM reads (design §7): `oam[N]` prints the entry as
//! fields, `oam[N].y|x|tile|attr` is one byte wherever a value goes.

use gbcli_protocol::{
    BreakArgs, ErrorCode, OamEntryRead, Op, ReadArgs, ReadValue, Response, RunArgs, WriteArg,
    WriteArgs, WriteKind,
};

use gbcli_core::Model;

use super::Handler;
use super::test_support::{handler, request, run_frames};
use crate::fixture::{Fixture, V1};

fn read(h: &mut Handler, exprs: &[&str]) -> Response {
    h.handle(&request(Op::Read(ReadArgs {
        reads: exprs.iter().map(|e| e.to_string()).collect(),
        since: None,
        allow_uninit: false,
    })))
}

fn values(response: Response) -> Vec<ReadValue> {
    response
        .outcome
        .unwrap_or_else(|e| panic!("{e:?}"))
        .reads
        .unwrap()
}

/// A session past the boot ROM with OAM entry 2 set to a known sprite.
fn with_sprite(name: &str) -> (Fixture, Handler) {
    sprite_in(Fixture::new(name))
}

/// The session of `f` past the boot ROM with OAM entry 2 set to a known sprite.
fn sprite_in(f: Fixture) -> (Fixture, Handler) {
    let mut h = handler(&f);
    run_frames(&mut h, 2).outcome.unwrap();
    h.handle(&request(Op::Write(WriteArgs {
        writes: vec![WriteArg {
            kind: WriteKind::Poke,
            target: "oam:8".into(),
            bytes: vec![80, 40, 7, 0b1011_0000],
        }],
    })))
    .outcome
    .unwrap();
    (f, h)
}

const SPRITE: OamEntryRead = OamEntryRead {
    index: 2,
    y: 80,
    x: 40,
    tile: 7,
    attr: 0xB0,
    priority: true,
    yflip: false,
    xflip: true,
    palette: Some(1),
    cgb_bank: None,
    cgb_palette: None,
};

#[test]
fn a_whole_entry_reads_as_its_fields() {
    let (_f, mut h) = with_sprite("oam-entry");
    let reads = values(read(&mut h, &["oam[2]"]));
    assert_eq!(
        reads,
        [ReadValue {
            expr: "oam[2]".into(),
            value: None,
            oam: Some(SPRITE),
            error: None,
        }]
    );
}

/// Review of e65e509: in CGB mode the row printed bit 4 as `palette` beside
/// `cgb_palette`, though CGB mode ignores it (Pan Docs: "Non CGB Mode only").
#[test]
fn in_cgb_mode_an_entry_reads_its_cgb_bank_and_palette_and_no_dmg_palette() {
    let mut rom = V1.to_vec();
    rom[0x143] = 0x80; // runs in CGB mode on a CGB
    rom[0x14D] = rom[0x134..0x14D]
        .iter()
        .fold(0u8, |x, b| x.wrapping_sub(*b).wrapping_sub(1));
    let (_f, mut h) = sprite_in(Fixture::with_model("oam-entry-cgb", &rom, Model::CgbE));
    let reads = values(read(&mut h, &["oam[2]"]));
    assert_eq!(
        reads[0].oam,
        Some(OamEntryRead {
            palette: None,
            cgb_bank: Some(0),
            cgb_palette: Some(0),
            ..SPRITE
        })
    );
}

#[test]
fn a_field_reads_as_one_byte_beside_other_reads() {
    let (_f, mut h) = with_sprite("oam-field");
    let reads = values(read(
        &mut h,
        &["oam[2].y", "oam[2].x", "oam[2].tile", "oam[2].attr & $20"],
    ));
    let got: Vec<Option<i64>> = reads.iter().map(|r| r.value).collect();
    assert_eq!(got, [Some(80), Some(40), Some(7), Some(0x20)]);
    assert!(reads.iter().all(|r| r.oam.is_none()));
}

#[test]
fn a_run_reads_an_entry_and_a_field_after_it_stops() {
    let (_f, mut h) = with_sprite("oam-run");
    let response = h.handle(&request(Op::Run(RunArgs {
        instructions: Some(1),
        read: vec!["oam[2]".into(), "oam[2].x".into()],
        ..RunArgs::default()
    })));
    let reads = values(response);
    let entry = reads[0].oam.expect("fields");
    assert_eq!(reads[1].value, Some(i64::from(entry.x)));
    assert_eq!(values(read(&mut h, &["oam[2]"]))[0].oam, Some(entry));
}

#[test]
fn a_breakpoint_condition_takes_a_field() {
    let (_f, mut h) = with_sprite("oam-break");
    let response = h.handle(&request(Op::Break(BreakArgs {
        at: "$0150".into(),
        condition: Some("oam[2].x >= 40".into()),
    })));
    assert!(response.outcome.is_ok(), "{:?}", response.outcome);
}

#[test]
fn an_entry_past_the_last_is_refused_with_the_last_one() {
    let (_f, mut h) = with_sprite("oam-range");
    for (text, remedy) in [("oam[40]", "oam[39]"), ("oam[40].x", "oam[39].x")] {
        let error = read(&mut h, &[text]).outcome.unwrap_err();
        assert_eq!(error.code, ErrorCode::BadExpression, "{text}");
        assert!(
            error.message.contains("oam[0] to oam[39]"),
            "{}",
            error.message
        );
        assert!(error.remedy.contains(remedy), "{}", error.remedy);
    }
}

#[test]
fn an_unknown_field_is_refused_naming_the_fields() {
    let (_f, mut h) = with_sprite("oam-field-name");
    let error = read(&mut h, &["oam[2].flags"]).outcome.unwrap_err();
    assert_eq!(error.code, ErrorCode::BadExpression);
    assert!(error.remedy.contains("oam[2].attr"), "{}", error.remedy);
}
