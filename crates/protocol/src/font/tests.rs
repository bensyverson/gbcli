use super::*;
use crate::{Op, parse_request};

#[test]
fn a_font_asset_is_glyphs_with_key_width_rows_and_origin() {
    let asset = FontAsset {
        source: "assets/fonts/main.yaff".into(),
        label: "Main".into(),
        fixed: false,
        glyphs: vec![
            FontGlyph {
                key: "i".into(),
                width: 2,
                rows: [0, 128, 0, 128, 128, 128, 128, 0],
                origin: GlyphOrigin::Drawn,
            },
            FontGlyph {
                key: "heart".into(),
                width: 7,
                rows: [0; 8],
                origin: GlyphOrigin::Composed,
            },
        ],
    };
    let json = serde_json::to_string(&asset).unwrap();
    assert_eq!(
        json,
        r#"{"source":"assets/fonts/main.yaff","label":"Main","fixed":false,"glyphs":[{"key":"i","width":2,"rows":[0,128,0,128,128,128,128,0],"origin":"drawn"},{"key":"heart","width":7,"rows":[0,0,0,0,0,0,0,0],"origin":"composed"}]}"#
    );
    assert_eq!(serde_json::from_str::<FontAsset>(&json).unwrap(), asset);
}

#[test]
fn a_font_request_parses_needs_no_session_and_refuses_unknown_fields() {
    let line = r#"{"id":"1","op":"font","args":{"file":"main.yaff","sample":"Hi","fit":40}}"#;
    let op = Op::Font(FontArgs {
        file: "main.yaff".into(),
        sample: Some("Hi".into()),
        fit: Some(40),
        ..FontArgs::default()
    });
    assert_eq!(parse_request(line).expect("parses").op, op);
    assert_eq!(op.name(), "font");
    assert!(!op.needs_bundle() && !op.changes_session() && !op.mutates_head());
    let bad = r#"{"id":"1","op":"font","args":{"file":"a.yaff","kern":true}}"#;
    assert!(parse_request(bad).is_err());
}

#[test]
fn a_sample_carries_its_tiles_as_hex() {
    let s = FontSample {
        text: "AA".into(),
        width: 10,
        tiles: 2,
        missing: vec![],
        data: vec![0xFF, 0xC0],
        fit: None,
    };
    let json = serde_json::to_string(&s).unwrap();
    assert_eq!(json, r#"{"text":"AA","width":10,"tiles":2,"data":"ffc0"}"#);
    assert_eq!(serde_json::from_str::<FontSample>(&json).unwrap(), s);
}
