//! The GBC result types against the review of 81cf3d2 (leaf jJjaK9): each
//! test began as a reviewer's probe that passed on the bug and now asserts
//! the fix.

use std::collections::BTreeMap;

use gbcli_protocol::{
    BuildMade, BuildTarget, Flip, GbcImage, GbcPalette, ImageArt, ImageAsset, ImageKind,
    NamedPalette, NamedPalettes, ObjectSize, PaletteFault, PaletteSlot, Rgb555, TileEntry,
};

/// A BuildMade recorded before `target` existed reads as a DMG build.
#[test]
fn an_older_build_made_without_target_reads_as_dmg() {
    let old = r#"{"rgbds":"1.0.4","sources":[],"outputs":[],"diagnostics":[]}"#;
    let made: BuildMade = serde_json::from_str(old).unwrap();
    assert_eq!(made.target, BuildTarget::Dmg);
}

fn forest() -> GbcPalette {
    GbcPalette([
        Rgb555::WHITE,
        Rgb555::from_rgb888(0x88, 0xCC, 0x44),
        Rgb555::from_rgb888(0x33, 0x66, 0x33),
        Rgb555::BLACK,
    ])
}

fn named(palettes: &[&str], assignments: &[(&str, &str)]) -> NamedPalettes {
    NamedPalettes {
        dmg: None,
        palettes: palettes
            .iter()
            .map(|n| NamedPalette {
                name: n.to_string(),
                colors: forest(),
            })
            .collect(),
        assignments: assignments
            .iter()
            .map(|(i, n)| (i.to_string(), n.to_string()))
            .collect::<BTreeMap<_, _>>(),
        pins: Vec::new(),
    }
}

/// The wire refuses what `assets/palettes.txt` refuses: `dmg` as a GBC
/// palette, a duplicate or non-label name, an assignment to `dmg` or to a
/// palette that isn't defined, and an image path that escapes `assets/` or
/// isn't spelled as `find` spells it.
#[test]
fn named_palettes_on_the_wire_pass_the_file_s_rules() {
    let refused: Vec<(NamedPalettes, PaletteFault)> = vec![
        (
            named(&["dmg"], &[]),
            PaletteFault::ReservedName { index: 0 },
        ),
        (
            named(&["forest", "Forest Two"], &[]),
            PaletteFault::BadName {
                index: 1,
                name: "Forest Two".into(),
            },
        ),
        (
            named(&["forest", "ember", "forest"], &[]),
            PaletteFault::Duplicate {
                index: 2,
                first: 0,
                name: "forest".into(),
            },
        ),
        (
            named(&["forest"], &[("bg/a.png", "dmg")]),
            PaletteFault::AssignedDmg {
                image: "bg/a.png".into(),
            },
        ),
        (
            named(&["forest"], &[("bg/b.png", "nowhere")]),
            PaletteFault::UnknownPalette {
                image: "bg/b.png".into(),
                name: "nowhere".into(),
            },
        ),
    ];
    for (n, fault) in refused {
        assert_eq!(n.check(), Err(fault.clone()), "{n:?}");
        let json = serde_json::to_string(&n).unwrap();
        let e = serde_json::from_str::<NamedPalettes>(&json).unwrap_err();
        assert!(e.to_string().contains(&fault.to_string()), "{e}");
    }
    for path in [
        "../../etc",
        "../bg/a.png",
        "./bg/a.png",
        "bg/./a.png",
        "bg//a.png",
        "bg\\a.png",
        "assets/bg/a.png",
        "/bg/a.png",
        "bg/a.txt",
        "bg/.png",
        "bg/sub/a.png",
        "music/a.png",
        "bg",
        "",
    ] {
        let n = named(&["forest"], &[(path, "forest")]);
        assert_eq!(
            n.check(),
            Err(PaletteFault::BadImage { image: path.into() }),
            "{path:?}"
        );
        let json = serde_json::to_string(&n).unwrap();
        assert!(
            serde_json::from_str::<NamedPalettes>(&json).is_err(),
            "{path:?}"
        );
    }
    for path in ["bg/a.png", "sprites/hero.16x16.png", "tiles/walls.PNG"] {
        let n = named(&["forest"], &[(path, "forest")]);
        assert_eq!(n.check(), Ok(()), "{path:?}");
        let json = serde_json::to_string(&n).unwrap();
        assert_eq!(serde_json::from_str::<NamedPalettes>(&json).unwrap(), n);
    }
}

/// Every fault is listed, palettes first, so the file parser can name the
/// earliest line.
#[test]
fn every_fault_is_listed_in_order() {
    let n = named(&["Bad", "ok"], &[("bg/a.png", "dmg"), ("bg/b.png", "ok")]);
    assert_eq!(
        n.faults(),
        vec![
            PaletteFault::BadName {
                index: 0,
                name: "Bad".into()
            },
            PaletteFault::AssignedDmg {
                image: "bg/a.png".into()
            },
        ]
    );
}

#[test]
fn a_palette_slot_is_0_to_7() {
    assert_eq!(PaletteSlot::new(7).map(PaletteSlot::get), Some(7));
    assert_eq!(PaletteSlot::new(8), None);
    assert_eq!(serde_json::to_string(&PaletteSlot::new(3)).unwrap(), "3");
    assert!(serde_json::from_str::<PaletteSlot>("8").is_err());
    assert!(serde_json::from_str::<PaletteSlot>("200").is_err());
}

/// A flip is the OAM attribute bits the frame lists write.
#[test]
fn a_flip_is_its_oam_bits() {
    assert_eq!(Flip::None.oam_bits(), 0);
    assert_eq!(Flip::X.oam_bits(), 0x20);
    assert_eq!(Flip::Y.oam_bits(), 0x40);
    assert_eq!(Flip::Xy.oam_bits(), 0x60);
    assert_eq!(serde_json::to_string(&Flip::Xy).unwrap(), r#""xy""#);
}

fn sheet(map: Vec<Option<TileEntry>>, gbc: Option<GbcImage>) -> ImageAsset {
    ImageAsset {
        kind: ImageKind::Sprites,
        source: "assets/sprites/hero.png".into(),
        label: "Hero".into(),
        width: 3,
        height: 2,
        tiles: 2,
        frames: Some(3),
        objects: Some(1),
        preview: "p".into(),
        tile_map: map,
        gbc,
        object_size: Some(ObjectSize::Tall),
        obp: None,
    }
}

/// A sheet of 8x8 objects (leaf GQFHVi): one entry an object, each its own
/// tile, so an odd index and a sheet one tile high are whole objects; the
/// size is on the wire as the LCDC names it.
#[test]
fn an_8x8_sheets_entries_are_one_tile_an_object() {
    let small = ImageAsset {
        height: 1,
        tiles: 3,
        object_size: Some(ObjectSize::Square),
        ..sheet(
            vec![
                entry(0, Flip::None, None),
                entry(1, Flip::X, None),
                entry(2, Flip::None, None),
            ],
            None,
        )
    };
    let json = serde_json::to_string(&small).unwrap();
    assert!(json.contains(r#""object_size":"8x8""#), "{json}");
    assert_eq!(serde_json::from_str::<ImageAsset>(&json).unwrap(), small);
    // 8x16: the same odd index is refused, and a document from before the
    // size was sent reads as 8x16.
    let tall = ImageAsset {
        object_size: Some(ObjectSize::Tall),
        ..small.clone()
    };
    let json = serde_json::to_string(&tall).unwrap();
    assert!(json.contains(r#""object_size":"8x16""#), "{json}");
    assert!(serde_json::from_str::<ImageAsset>(&json).is_err());
    let before = json.replace(r#","object_size":"8x16""#, "");
    assert!(!before.contains("object_size"), "{before}");
    assert!(serde_json::from_str::<ImageAsset>(&before).is_err());
    // Only a sheet has objects.
    let bg = r#"{"kind":"bg","source":"a.png","label":"A","width":1,"height":1,"tiles":1,"object_size":"8x8"}"#;
    assert!(serde_json::from_str::<ImageAsset>(bg).is_err());
}

fn entry(index: u32, flip: Flip, slot: Option<u8>) -> Option<TileEntry> {
    Some(TileEntry {
        index,
        flip,
        palette: slot.map(|s| PaletteSlot::new(s).unwrap()),
    })
}

/// A sheet's entry carries the unit's flip, and its index is the unit's
/// top tile (even, from the image's own first tile).
#[test]
fn a_sheet_entry_carries_its_flip_and_slot() {
    let indexed = GbcImage {
        preview: String::new(),
        art: ImageArt::Indexed {
            palette: "ember".into(),
        },
        palettes: Vec::new(),
    };
    let a = sheet(
        vec![
            entry(0, Flip::None, Some(2)),
            entry(0, Flip::X, Some(2)),
            None,
        ],
        Some(indexed),
    );
    let json = serde_json::to_string(&a).unwrap();
    assert!(
        json.contains(
            r#""tile_map":[{"index":0,"palette":2},{"index":0,"flip":"x","palette":2},null]"#
        ),
        "{json}"
    );
    assert_eq!(serde_json::from_str::<ImageAsset>(&json).unwrap(), a);
    let dmg = ImageAsset {
        tiles: 4,
        ..sheet(vec![entry(2, Flip::Xy, None), None, None], None)
    };
    let json = serde_json::to_string(&dmg).unwrap();
    assert!(
        json.contains(r#""tile_map":[{"index":2,"flip":"xy"},null,null]"#),
        "{json}"
    );
    assert_eq!(serde_json::from_str::<ImageAsset>(&json).unwrap(), dmg);
}

/// The slot rides in the entry, so the two can't disagree in length or
/// in where a unit is transparent; what is left to check on read is that
/// a GBC image's entries all have a slot and a DMG image's none, and that
/// only a sheet flips.
#[test]
fn entries_that_disagree_with_the_target_are_refused_on_read() {
    let color = || GbcImage {
        preview: String::new(),
        art: ImageArt::Color,
        palettes: vec![forest()],
    };
    // 2x1 tiles: two entries.
    let bg = |map: Vec<Option<TileEntry>>, gbc: Option<GbcImage>| ImageAsset {
        kind: ImageKind::Bg,
        width: 2,
        height: 1,
        frames: None,
        objects: None,
        object_size: None,
        ..sheet(map, gbc)
    };
    let tile = |slot| entry(0, Flip::None, slot);
    for (what, a) in [
        (
            "a GBC entry without a slot",
            bg(
                vec![entry(0, Flip::None, Some(0)), entry(1, Flip::None, None)],
                Some(color()),
            ),
        ),
        (
            "a DMG entry with a slot",
            bg(vec![tile(Some(0)), tile(None)], None),
        ),
        (
            "a background's flip",
            bg(
                vec![entry(0, Flip::X, Some(0)), tile(Some(0))],
                Some(color()),
            ),
        ),
        (
            "a background's missing tile",
            bg(vec![None, tile(Some(0))], Some(color())),
        ),
    ] {
        let json = serde_json::to_string(&a).unwrap();
        assert!(
            serde_json::from_str::<ImageAsset>(&json).is_err(),
            "{what}: {json}"
        );
    }
    let slot_past_7 = r#"{"kind":"bg","source":"s","label":"S","width":1,"height":1,"tiles":1,
        "preview":"p","tile_map":[{"index":0,"palette":8}],"gbc":{"art":"color"}}"#;
    assert!(serde_json::from_str::<ImageAsset>(slot_past_7).is_err());
    let fine = bg(vec![tile(Some(0)), tile(Some(0))], Some(color()));
    let json = serde_json::to_string(&fine).unwrap();
    assert_eq!(serde_json::from_str::<ImageAsset>(&json).unwrap(), fine);
}

/// The file refuses a `dmg` line with two colors one in RGB555
/// (`two_dmg_colors_one_in_rgb555_are_an_error_at_the_second`), since they
/// can't name two shades; so does the wire. A named palette may repeat.
#[test]
fn a_dmg_palette_with_a_repeated_color_is_refused_on_the_wire() {
    let json = r#"{"dmg":[32767,0,10570,0],"palettes":[]}"#;
    let e = serde_json::from_str::<NamedPalettes>(json).unwrap_err();
    let fault = PaletteFault::RepeatedDmgColor { shade: 3, first: 1 };
    assert!(e.to_string().contains(&fault.to_string()), "{e}");
    let n = NamedPalettes {
        dmg: Some(GbcPalette([
            Rgb555::WHITE,
            Rgb555::BLACK,
            Rgb555::new(10, 10, 10).unwrap(),
            Rgb555::BLACK,
        ])),
        ..NamedPalettes::default()
    };
    assert_eq!(n.check(), Err(fault));
    let twin = r#"{"palettes":[{"name":"twin","colors":[0,0,32767,32767]}]}"#;
    assert!(serde_json::from_str::<NamedPalettes>(twin).is_ok());
}

/// The file refuses an image assigned twice; on the wire that is a
/// repeated key, refused rather than read last-wins.
#[test]
fn an_image_assigned_twice_is_refused_on_the_wire() {
    let colors = "[32767,9009,6534,2176]";
    let json = format!(
        r#"{{"palettes":[{{"name":"a","colors":{colors}}},{{"name":"b","colors":{colors}}}],
            "assignments":{{"bg/t.png":"a","bg/t.png":"b"}}}}"#
    );
    let e = serde_json::from_str::<NamedPalettes>(&json).unwrap_err();
    assert!(
        e.to_string()
            .contains("bg/t.png is assigned a palette again"),
        "{e}"
    );
    let once = json.replace(r#","bg/t.png":"b""#, "");
    assert!(
        serde_json::from_str::<NamedPalettes>(&once).is_ok(),
        "{once}"
    );
}

/// An image as JSON: `width`x`height` tiles, `tiles` unique, `map` its
/// tile map and `gbc` (with its leading comma) its GBC version.
fn asset_json(kind: &str, (width, height): (u32, u32), tiles: u32, map: &str, gbc: &str) -> String {
    format!(
        r#"{{"kind":"{kind}","source":"assets/x.png","label":"X","width":{width},"height":{height},
            "tiles":{tiles},"preview":"p.png","tile_map":{map}{gbc}}}"#
    )
}

/// The entries' numbers must be ones the build makes: a sheet unit's top
/// tile, even, with its bottom tile among `tiles`; a tile among `tiles`;
/// one entry per tile (per 8x16 unit of a sheet); and a GBC image has
/// entries. A DMG image may leave its map empty.
#[test]
fn entries_the_build_never_makes_are_refused_on_read() {
    let gbc = r#","gbc":{"art":"indexed","palette":"forest"}"#;
    for (what, json) in [
        (
            "an odd unit index",
            asset_json("sprites", (2, 2), 2, r#"[{"index":1},null]"#, ""),
        ),
        (
            "a unit whose bottom tile is past tiles",
            asset_json("sprites", (2, 2), 3, r#"[{"index":2},null]"#, ""),
        ),
        (
            "an index past tiles",
            asset_json(
                "bg",
                (2, 2),
                2,
                r#"[{"index":9},{"index":0},{"index":0},{"index":0}]"#,
                "",
            ),
        ),
        (
            "one entry for a 2x2 background",
            asset_json("bg", (2, 2), 1, r#"[{"index":0}]"#, ""),
        ),
        (
            "a unit per tile of a sheet",
            asset_json("sprites", (1, 2), 2, r#"[{"index":0},null]"#, ""),
        ),
        (
            "a GBC image with no entries",
            asset_json("bg", (2, 2), 1, "[]", gbc),
        ),
    ] {
        assert!(
            serde_json::from_str::<ImageAsset>(&json).is_err(),
            "{what}: {json}"
        );
    }
    for json in [
        asset_json(
            "sprites",
            (2, 2),
            2,
            r#"[{"index":0},{"index":0,"flip":"x"}]"#,
            "",
        ),
        asset_json("sprites", (2, 4), 2, r#"[null,null,null,{"index":0}]"#, ""),
        asset_json("bg", (2, 1), 2, r#"[{"index":1},{"index":0}]"#, ""),
        asset_json("bg", (2, 2), 1, "[]", ""),
    ] {
        assert!(serde_json::from_str::<ImageAsset>(&json).is_ok(), "{json}");
    }
}

/// Indexed art names a palette `NamedPalettes` could hold: never `dmg`,
/// and a name that can become a label.
#[test]
fn indexed_art_names_a_palette_that_can_be_one() {
    let map = r#"[{"index":0,"palette":0}]"#;
    for name in ["dmg", "Not A Label", ""] {
        let gbc = format!(r#","gbc":{{"art":"indexed","palette":"{name}"}}"#);
        let json = asset_json("bg", (1, 1), 1, map, &gbc);
        assert!(serde_json::from_str::<ImageAsset>(&json).is_err(), "{json}");
    }
    // With its one palette, as an indexed background has (wire_refusals.rs).
    let gbc =
        r#","gbc":{"art":"indexed","palette":"forest_2","palettes":[[32767,9009,6534,2176]]}"#;
    let json = asset_json("bg", (1, 1), 1, map, gbc);
    assert!(serde_json::from_str::<ImageAsset>(&json).is_ok(), "{json}");
}
