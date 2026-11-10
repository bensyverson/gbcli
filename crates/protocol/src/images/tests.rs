//! Round trips of the image results' wire form (the DMG types; the GBC
//! ones are tested in `gbc.rs`).

use super::*;

#[test]
fn a_problem_spells_its_limit_snake_case_and_drops_what_it_lacks() {
    let p = ImageProblem {
        image: Some("assets/bg/town.png".into()),
        limit: ImageLimit::BgTilesMap,
        severity: Severity::Error,
        count: 291,
        max: 256,
        tiles: vec![TileAt { x: 3, y: 4 }],
        line: None,
        column: None,
        message: "too many".into(),
        remedy: "fewer".into(),
    };
    let json = serde_json::to_string(&p).unwrap();
    assert_eq!(
        json,
        r#"{"image":"assets/bg/town.png","limit":"bg_tiles_map","severity":"error","count":291,"max":256,"tiles":[{"x":3,"y":4}],"message":"too many","remedy":"fewer"}"#
    );
    assert_eq!(serde_json::from_str::<ImageProblem>(&json).unwrap(), p);
}

#[test]
fn a_shared_shade_warning_round_trips_as_shared_shade() {
    let p = ImageProblem {
        image: Some("assets/bg/cave.png".into()),
        limit: ImageLimit::SharedShade,
        severity: Severity::Warning,
        count: 2,
        max: 1,
        tiles: vec![TileAt { x: 1, y: 0 }],
        line: None,
        column: None,
        message: "one shade".into(),
        remedy: "distinct".into(),
    };
    let json = serde_json::to_string(&p).unwrap();
    assert_eq!(
        json,
        r#"{"image":"assets/bg/cave.png","limit":"shared_shade","severity":"warning","count":2,"max":1,"tiles":[{"x":1,"y":0}],"message":"one shade","remedy":"distinct"}"#
    );
    assert_eq!(serde_json::from_str::<ImageProblem>(&json).unwrap(), p);
}

#[test]
fn a_palette_file_problem_carries_its_line() {
    let p = ImageProblem {
        image: Some("assets/palettes.txt".into()),
        limit: ImageLimit::PaletteFile,
        severity: Severity::Error,
        count: 2,
        max: 4,
        tiles: Vec::new(),
        line: Some(3),
        column: None,
        message: "not a color".into(),
        remedy: "one #rrggbb a line".into(),
    };
    let json = serde_json::to_string(&p).unwrap();
    assert_eq!(
        json,
        r#"{"image":"assets/palettes.txt","limit":"palette_file","severity":"error","count":2,"max":4,"line":3,"message":"not a color","remedy":"one #rrggbb a line"}"#
    );
    assert_eq!(serde_json::from_str::<ImageProblem>(&json).unwrap(), p);
}

#[test]
fn a_pixel_text_problem_carries_its_line_and_column() {
    let p = ImageProblem {
        image: Some("assets/bg/meadow.pixels".into()),
        limit: ImageLimit::PixelsText,
        severity: Severity::Error,
        count: 1,
        max: 1,
        tiles: Vec::new(),
        line: Some(52),
        column: std::num::NonZeroU32::new(7),
        message: "no tile gx".into(),
        remedy: "define gx".into(),
    };
    let json = serde_json::to_string(&p).unwrap();
    assert_eq!(
        json,
        r#"{"image":"assets/bg/meadow.pixels","limit":"pixels_text","severity":"error","count":1,"max":1,"line":52,"column":7,"message":"no tile gx","remedy":"define gx"}"#
    );
    assert_eq!(serde_json::from_str::<ImageProblem>(&json).unwrap(), p);
    assert_eq!(ImageLimit::PixelsText.name(), "pixels_text");
}

#[test]
fn the_shade_rule_round_trips_with_its_known_palette() {
    let made = ImagesMade {
        shades: Some(ShadeChoice {
            rule: ShadeRule::Known,
            palette: Some(KnownPalette::PeaGreens),
        }),
        ..ImagesMade::default()
    };
    let json = serde_json::to_string(&made).unwrap();
    assert_eq!(
        json,
        r#"{"images":[],"shades":{"rule":"known","palette":"pea_greens"}}"#
    );
    assert_eq!(serde_json::from_str::<ImagesMade>(&json).unwrap(), made);
    for rule in [
        ShadeRule::Declared,
        ShadeRule::Known,
        ShadeRule::Stretched,
        ShadeRule::Nearest,
    ] {
        assert_eq!(serde_json::to_value(rule).unwrap(), rule.name());
    }
    for palette in KnownPalette::ALL {
        assert_eq!(serde_json::to_value(palette).unwrap(), palette.name());
    }
    let names: Vec<&str> = KnownPalette::ALL.iter().map(|p| p.name()).collect();
    assert_eq!(
        names,
        [
            "dmg_grays",
            "pea_greens",
            "gb_studio_greens",
            "pocket_grays"
        ]
    );
}

#[test]
fn every_limit_name_matches_the_wire() {
    use ImageLimit::*;
    for limit in [
        BgTilesMap,
        VramTiles,
        ObjTiles,
        DmgPalette,
        SizeMultiple8,
        ObjPair8x16,
        ObjNoTransparency,
        ObjPerLine,
        ObjTotal,
        FrameGrid,
        FrameSpan,
        RomxBank,
        Alpha,
        SharedShade,
        PaletteFile,
        ShadeStretched,
        GbcPalettes,
        GbcTileColors,
        DmgOverrideSize,
        ShadesUnpinned,
    ] {
        assert_eq!(serde_json::to_value(limit).unwrap(), limit.name());
    }
    assert_eq!(GbcPalettes.name(), "gbc_palettes");
    assert_eq!(GbcTileColors.name(), "gbc_tile_colors");
    assert_eq!(DmgOverrideSize.name(), "dmg_override_size");
    assert_eq!(ShadesUnpinned.name(), "shades_unpinned");
    assert_eq!(ImageKind::Tiles.dir(), "tiles");
    for kind in [ImageKind::Bg, ImageKind::Sprites, ImageKind::Tiles] {
        assert_eq!(serde_json::to_value(kind).unwrap(), kind.dir());
    }
}

/// The build runs no rgbgfx since it encodes images itself (c47f60d), so
/// the limit that checked rgbgfx's output is gone from the wire (Ben's
/// ruling, 2026-11-09), and `alpha` still reads.
#[test]
fn rgbgfx_agrees_is_no_limit() {
    assert!(serde_json::from_str::<ImageLimit>("\"rgbgfx_agrees\"").is_err());
    assert_eq!(
        serde_json::from_str::<ImageLimit>("\"alpha\"").unwrap(),
        ImageLimit::Alpha
    );
}
