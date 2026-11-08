//! Documents the build never makes are refused on read (the review of
//! 2efed71, "review-wire13"): palette slots and counts past what an image
//! or the shared OBJ palettes hold, and a sheet's shape. Beside each
//! refusal, the nearest document the build does make still reads.

use gbcli_protocol::{GbcImagesMade, ImageAsset, ImagesMade, NamedPalettes};

fn asset_json(kind: &str, (width, height): (u32, u32), tiles: u32, map: &str, gbc: &str) -> String {
    format!(
        r#"{{"kind":"{kind}","source":"assets/x.png","label":"X","width":{width},"height":{height},
            "tiles":{tiles},"preview":"p.png","tile_map":{map}{gbc}}}"#
    )
}

const FOREST: &str = "[32767,9009,6534,2176]";

#[test]
fn nine_obj_palettes_are_refused() {
    let nine = [FOREST; 9].join(",");
    let json = format!(r#"{{"obj_palettes":[{nine}]}}"#);
    assert!(
        serde_json::from_str::<GbcImagesMade>(&json).is_err(),
        "nine OBJ palettes read: {json}"
    );
}

#[test]
fn indexed_bg_slot_past_its_one_palette_is_refused() {
    let gbc = format!(r#","gbc":{{"art":"indexed","palette":"forest","palettes":[{FOREST}]}}"#);
    let json = asset_json("bg", (1, 1), 1, r#"[{"index":0,"palette":5}]"#, &gbc);
    assert!(
        serde_json::from_str::<ImageAsset>(&json).is_err(),
        "an indexed bg's entry names slot 5 of 1 palette: {json}"
    );
}

#[test]
fn indexed_bg_without_its_palette_is_refused() {
    let gbc = r#","gbc":{"art":"indexed","palette":"forest"}"#;
    let json = asset_json("bg", (1, 1), 1, r#"[{"index":0,"palette":0}]"#, gbc);
    assert!(
        serde_json::from_str::<ImageAsset>(&json).is_err(),
        "an indexed bg with no palettes: {json}"
    );
}

#[test]
fn sheet_with_bg_palettes_is_refused() {
    let gbc = format!(r#","gbc":{{"art":"indexed","palette":"forest","palettes":[{FOREST}]}}"#);
    let json = asset_json("sprites", (1, 2), 2, r#"[{"index":0,"palette":0}]"#, &gbc);
    assert!(
        serde_json::from_str::<ImageAsset>(&json).is_err(),
        "a sheet carrying BG palettes: {json}"
    );
}

#[test]
fn indexed_sheet_with_two_slots_is_refused() {
    let gbc = r#","gbc":{"art":"indexed","palette":"forest"}"#;
    let json = asset_json(
        "sprites",
        (2, 2),
        2,
        r#"[{"index":0,"palette":0},{"index":0,"palette":3}]"#,
        gbc,
    );
    assert!(
        serde_json::from_str::<ImageAsset>(&json).is_err(),
        "one indexed sheet in two OBJ slots: {json}"
    );
}

#[test]
fn sheet_slot_past_obj_palettes_is_refused() {
    let sheet = asset_json(
        "sprites",
        (1, 2),
        2,
        r#"[{"index":0,"palette":6}]"#,
        r#","gbc":{"art":"indexed","palette":"forest"}"#,
    );
    let json = format!(r#"{{"images":[{sheet}],"gbc":{{"obj_palettes":[{FOREST}]}}}}"#);
    assert!(
        serde_json::from_str::<ImagesMade>(&json).is_err(),
        "a sheet's slot 6 with 1 OBJ palette: {json}"
    );
}

#[test]
fn odd_sheet_height_is_refused() {
    let json = asset_json("sprites", (1, 3), 2, r#"[{"index":0}]"#, "");
    assert!(
        serde_json::from_str::<ImageAsset>(&json).is_err(),
        "a sheet 3 tiles high: {json}"
    );
}

#[test]
fn hostile_inputs_do_not_panic() {
    for json in [
        asset_json(
            "sprites",
            (u32::MAX, u32::MAX),
            u32::MAX,
            r#"[{"index":65535}]"#,
            "",
        ),
        asset_json("bg", (0, 0), 0, r#"[{"index":0}]"#, ""),
        asset_json("sprites", (0, 1), 0, "[null]", ""),
    ] {
        let _ = serde_json::from_str::<ImageAsset>(&json);
    }
    for json in [
        r#"{"assignments":{"a":"b","a":"c"},"assignments":{}}"#,
        r#"{"dmg":[0,0,0,0]}"#,
        r#"{"dmg":[0,1,2,3],"dmg":[0,1,2,3]}"#,
    ] {
        let _ = serde_json::from_str::<NamedPalettes>(json);
    }
}

/// What the build makes beside the refused shapes: a color sheet whose
/// units take two OBJ slots, both within the shared palettes, and a color
/// background naming its second palette.
#[test]
fn the_shapes_the_build_makes_still_read() {
    let sheet = asset_json(
        "sprites",
        (2, 2),
        2,
        r#"[{"index":0,"palette":0},{"index":0,"palette":1}]"#,
        r#","gbc":{"art":"color"}"#,
    );
    let json = format!(r#"{{"images":[{sheet}],"gbc":{{"obj_palettes":[{FOREST},{FOREST}]}}}}"#);
    serde_json::from_str::<ImagesMade>(&json).expect(&json);
    let gbc = format!(r#","gbc":{{"art":"color","palettes":[{FOREST},{FOREST}]}}"#);
    let bg = asset_json("bg", (1, 1), 1, r#"[{"index":0,"palette":1}]"#, &gbc);
    serde_json::from_str::<ImageAsset>(&bg).expect(&bg);
    // A DMG image: a tile map without slots, and no GBC section.
    let dmg = asset_json("sprites", (1, 2), 2, r#"[{"index":0,"flip":"x"}]"#, "");
    let json = format!(r#"{{"images":[{dmg}]}}"#);
    serde_json::from_str::<ImagesMade>(&json).expect(&json);
    let eight = [FOREST; 8].join(",");
    let json = format!(r#"{{"obj_palettes":[{eight}]}}"#);
    serde_json::from_str::<GbcImagesMade>(&json).expect(&json);
}

/// One rule refuses an image assigned twice, for the file's lines
/// (`NamedPalettes::assign`) and the wire's repeated keys alike.
#[test]
fn an_image_assigned_twice_is_one_rule_for_file_and_wire() {
    let mut named = NamedPalettes::default();
    named.assign("bg/t.png".into(), "a".into()).unwrap();
    let fault = named.assign("bg/t.png".into(), "b".into()).unwrap_err();
    assert_eq!(fault.to_string(), "bg/t.png is assigned a palette again");
    assert_eq!(named.assignments["bg/t.png"], "a");
    let json = r#"{"palettes":[{"name":"a","colors":[0,0,0,0]}],
                   "assignments":{"bg/t.png":"a","bg/t.png":"a"}}"#;
    let err = serde_json::from_str::<NamedPalettes>(json).unwrap_err();
    assert!(err.to_string().contains(&fault.to_string()), "{err}");
}

/// An image with a GBC version needs the shared GBC section beside it.
#[test]
fn a_gbc_image_without_the_shared_gbc_section_is_refused() {
    let sheet = asset_json(
        "sprites",
        (1, 2),
        2,
        r#"[{"index":0,"palette":0}]"#,
        r#","gbc":{"art":"color"}"#,
    );
    let json = format!(r#"{{"images":[{sheet}]}}"#);
    assert!(serde_json::from_str::<ImagesMade>(&json).is_err(), "{json}");
}
