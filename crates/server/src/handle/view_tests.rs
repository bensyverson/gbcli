//! The `view` verb: VRAM and OAM as data, the PNG on request, and the head
//! left where it was.

use gbcli_protocol::{
    AddressingView, ImageOut, MapAreaView, MapSel, Op, ReadArgs, Rev, VideoData, ViewArgs,
    ViewKind, ViewMade,
};

use super::Handler;
use super::test_support::{handler, request, run_frames};
use crate::fixture::Fixture;

fn view(h: &mut Handler, args: ViewArgs) -> ViewMade {
    let response = h.handle(&request(Op::View(args)));
    assert_eq!(response.rev, Some(Rev(1)), "no new head");
    response.outcome.expect("viewed").view.expect("view data")
}

fn reads(h: &mut Handler, exprs: Vec<String>) -> Vec<i64> {
    let response = h.handle(&request(Op::Read(ReadArgs {
        reads: exprs,
        since: None,
        allow_uninit: false,
    })));
    let result = response.outcome.expect("reads");
    result
        .reads
        .unwrap()
        .iter()
        .map(|r| r.value.unwrap())
        .collect()
}

/// A machine past the boot ROM, whose logo is in VRAM.
fn booted(name: &str) -> (Fixture, Handler) {
    let f = Fixture::new(name);
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 2).is_ok());
    (f, h)
}

#[test]
fn tiles_are_data_first_and_skip_the_blank_ones() {
    let (_f, mut h) = booted("view-tiles");
    let before = reads(&mut h, vec!["t".into(), "pc".into()]);
    let made = view(
        &mut h,
        ViewArgs {
            what: ViewKind::Tiles,
            map: None,
            png: None,
        },
    );
    assert_eq!(made.png, None, "no PNG unless asked");
    let VideoData::Tiles { tiles, blank } = made.data else {
        panic!("{:?}", made.data)
    };
    assert!(!tiles.is_empty(), "the boot logo's tiles");
    assert_eq!(tiles.len() as u32 + blank, 384, "one DMG bank");
    for tile in &tiles {
        assert_eq!(tile.rows.len(), 8);
        assert!(tile.rows.iter().all(|r| r.len() == 8));
        assert!(tile.rows.iter().any(|r| r != "00000000"), "{tile:?}");
    }
    assert_eq!(reads(&mut h, vec!["t".into(), "pc".into()]), before);
}

#[test]
fn a_map_row_is_the_bytes_vram_holds() {
    let (_f, mut h) = booted("view-map");
    let made = view(
        &mut h,
        ViewArgs {
            what: ViewKind::Map,
            map: Some(MapSel::Map9800),
            png: None,
        },
    );
    let VideoData::Map {
        area,
        addressing,
        ids,
        attributes,
        ..
    } = made.data
    else {
        panic!("{:?}", made.data)
    };
    assert_eq!(area, MapAreaView::Map9800);
    assert_eq!(
        addressing,
        AddressingView::Unsigned8000,
        "LCDC.4 after boot"
    );
    assert_eq!(attributes, None, "no attributes on a DMG");
    assert_eq!(ids.len(), 32);
    let row = ids
        .iter()
        .position(|r| r.split(' ').any(|id| id != "00"))
        .expect("the logo row");
    let exprs = (0..32)
        .map(|c| format!("[${:04X}]", 0x9800 + 32 * row + c))
        .collect();
    let bytes: Vec<String> = reads(&mut h, exprs)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(ids[row], bytes.join(" "));
}

#[test]
fn oam_with_a_png_writes_the_sheet_beside_the_data() {
    let (f, mut h) = booted("view-oam");
    let path = f.dir.join("oam.png");
    let made = view(
        &mut h,
        ViewArgs {
            what: ViewKind::Oam,
            map: None,
            png: Some(ImageOut {
                path: path.to_string_lossy().into_owned(),
                scale: Some(1),
            }),
        },
    );
    let VideoData::Oam { height, objects } = made.data else {
        panic!("{:?}", made.data)
    };
    assert_eq!(height, 8);
    assert_eq!(objects.len(), 40);
    assert!(
        objects
            .iter()
            .enumerate()
            .all(|(i, o)| o.index as usize == i)
    );
    let png = made.png.expect("asked for");
    assert_eq!(png.scale, 1);
    let bytes = std::fs::read(&path).expect("written");
    assert_eq!(&bytes[1..4], b"PNG");
    let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    assert_eq!(width, png.width);
}
