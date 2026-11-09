//! Fonts (`assets/fonts/NAME.yaff`, leaf IQRDj5): each font's glyphs in
//! file order, never deduplicated, as `<Name>FontGlyphs` and
//! `<Name>FontWidths`, with a charmap macro, so `db "Hi"` assembles to
//! glyph numbers; at most 254 glyphs a build, or a character list; and
//! `FontAsset` in the results, which `assets` returns for the characters
//! a request names, with no cap.

use crate::common;

use std::fs;

use gbcli_build::{AssetsInput, AssetsOptions, BuildOptions, assets, build};
use gbcli_protocol::{
    AssetManifest, ErrorCode, FontAsset, GlyphOrigin, ImageLimit, ImagesMade, ManifestFont,
};

use common::{Temp, failed, rgbds, starter};

/// A `.yaff` of `glyphs`, each a label and its 8 rows.
fn yaff(glyphs: &[(&str, [&str; 8])]) -> String {
    let mut out = String::from("name: Test\n");
    for (label, rows) in glyphs {
        out += &format!("\n{label}:\n");
        for r in rows {
            out += &format!("    {r}\n");
        }
    }
    out
}

const SPACE: [&str; 8] = ["..."; 8];
const H: [&str; 8] = [
    "@...@", "@...@", "@@@@@", "@...@", "@...@", "@...@", ".....", ".....",
];
const I: [&str; 8] = ["@.", "..", "@.", "@.", "@.", "@.", "..", ".."];
const QUOTE: [&str; 8] = [
    "@.@.", "@.@.", "....", "....", "....", "....", "....", "....",
];
const HEART: [&str; 8] = [
    ".@@.@@.", "@@@@@@@", "@@@@@@@", ".@@@@@.", "..@@@..", "...@...", ".......", ".......",
];

fn main_font(temp: &Temp) {
    let text = yaff(&[
        ("u+0020", SPACE),
        ("'H'", H),
        ("'i'", I),
        ("\"heart\"", HEART),
        ("'\"'", QUOTE),
    ]);
    common::without_text(&temp.path("demo"));
    fs::create_dir_all(temp.path("demo/assets/fonts")).unwrap();
    fs::write(temp.path("demo/assets/fonts/main.yaff"), text).unwrap();
}

/// `key` 1 px wide, one per character from `first`, `n` of them.
fn big_font(temp: &Temp, name: &str, first: u32, n: u32) {
    let rows = ["@"; 8];
    let labels: Vec<String> = (first..first + n).map(|c| format!("u+{c:04X}")).collect();
    let glyphs: Vec<(&str, [&str; 8])> = labels.iter().map(|l| (l.as_str(), rows)).collect();
    fs::create_dir_all(temp.path("demo/assets/fonts")).unwrap();
    fs::write(
        temp.path(&format!("demo/assets/fonts/{name}.yaff")),
        yaff(&glyphs),
    )
    .unwrap();
}

/// The ROM bytes at `symbol`, by the build's `.sym`.
fn rom_at(temp: &Temp, symbol: &str, len: usize) -> Vec<u8> {
    let sym = fs::read_to_string(temp.path("demo/build/demo.sym")).unwrap();
    let line = sym
        .lines()
        .find(|l| l.split_whitespace().nth(1) == Some(symbol))
        .unwrap_or_else(|| panic!("{symbol} not in the .sym:\n{sym}"));
    let place = line.split_whitespace().next().unwrap();
    let (bank, addr) = place.split_once(':').unwrap();
    let bank = usize::from_str_radix(bank, 16).unwrap();
    let addr = usize::from_str_radix(addr, 16).unwrap();
    let offset = if bank == 0 {
        addr
    } else {
        bank * 0x4000 + addr - 0x4000
    };
    let rom = fs::read(temp.path("demo/build/demo.gb")).unwrap();
    rom[offset..offset + len].to_vec()
}

fn built(options: &BuildOptions) -> ImagesMade {
    let images = build(&rgbds(), options).unwrap().images.expect("images");
    assert!(images.first_error().is_none(), "{:#?}", images.problems);
    let json = serde_json::to_string(&images).unwrap();
    let read: ImagesMade = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{e}: {json}"));
    assert_eq!(read, images);
    images
}

fn generated(temp: &Temp, file: &str) -> String {
    fs::read_to_string(temp.path(&format!("demo/build/images/{file}"))).unwrap()
}

const USES: &str = "INCLUDE \"images.inc\"\n\
    \tMAIN_FONT_CHARMAP hud, 0\n\
    \tMAIN_FONT_CHARMAP tiles, $80\n\
    SECTION \"text\", ROMX\n\
    \tPUSHC hud\n\
    Text:: db \"Hi <heart>\\\"\", MAIN_FONT_END\n\
    \tPOPC\n\
    \tPUSHC tiles\n\
    TileText:: db \"iH\\n\"\n\
    \tPOPC\n\
    Plain:: db \"H\"\n\
    Uses:: dw MainFontGlyphs, MainFontGlyphsEnd, MainFontWidths, MainFontWidthsEnd\n\
    \tdb MAIN_FONT_GLYPHS, MAIN_FONT_FIXED, MAIN_FONT_NEWLINE\n";

#[test]
fn a_font_builds_to_glyph_and_width_tables_and_a_charmap_that_assembles_strings_to_glyph_numbers() {
    let (temp, options) = starter("fonts-build");
    main_font(&temp);
    fs::write(temp.path("demo/src/text.asm"), USES).unwrap();
    let images = built(&options);
    // The charmap maps each glyph to base + its number, in file order.
    assert_eq!(rom_at(&temp, "Text", 6), [1, 2, 0, 3, 4, 0xFF]);
    assert_eq!(rom_at(&temp, "TileText", 3), [0x82, 0x81, 0xFE]);
    // Outside the charmap, text is ASCII again.
    assert_eq!(rom_at(&temp, "Plain", 1), [b'H']);
    // 8 bytes a glyph, row 0 first, bit 7 the leftmost pixel.
    let glyphs = rom_at(&temp, "MainFontGlyphs", 40);
    assert_eq!(&glyphs[..8], &[0; 8]);
    assert_eq!(&glyphs[8..16], &[0x88, 0x88, 0xF8, 0x88, 0x88, 0x88, 0, 0]);
    assert_eq!(&glyphs[16..24], &[0x80, 0, 0x80, 0x80, 0x80, 0x80, 0, 0]);
    assert_eq!(rom_at(&temp, "MainFontWidths", 5), [3, 5, 2, 7, 4]);
    let inc = generated(&temp, "images.inc");
    assert!(inc.contains("DEF MAIN_FONT_GLYPHS EQU 5"), "{inc}");
    assert!(inc.contains("DEF MAIN_FONT_FIXED EQU 0"), "{inc}");
    assert!(inc.contains("MACRO MAIN_FONT_CHARMAP"), "{inc}");
    assert!(inc.contains("CHARMAP \"<heart>\", (\\2) + 3"), "{inc}");
    assert!(inc.contains("CHARMAP \"\\\"\", (\\2) + 4"), "{inc}");
    // The result is the seam LoadHigh reads.
    assert_eq!(images.fonts.len(), 1);
    let f: &FontAsset = &images.fonts[0];
    assert_eq!(
        (f.source.as_str(), f.label.as_str(), f.fixed),
        ("demo/assets/fonts/main.yaff", "Main", false)
    );
    let keys: Vec<&str> = f.glyphs.iter().map(|g| g.key.as_str()).collect();
    assert_eq!(keys, [" ", "H", "i", "heart", "\""]);
    assert_eq!(f.glyphs[1].rows, [0x88, 0x88, 0xF8, 0x88, 0x88, 0x88, 0, 0]);
    assert_eq!(f.glyphs[3].width, 7);
    assert!(f.glyphs.iter().all(|g| g.origin == GlyphOrigin::Drawn));
    // The constants are exported, so the .sym names them.
    let sym = fs::read_to_string(temp.path("demo/build/demo.sym")).unwrap();
    assert!(sym.contains(" MAIN_FONT_GLYPHS"), "{sym}");
}

#[test]
fn identical_glyphs_keep_their_own_numbers_and_a_fixed_font_says_so() {
    let (temp, options) = starter("fonts-fixed");
    let full = ["@@@@@@@@"; 8];
    let text = yaff(&[("'O'", full), ("'0'", full), ("'1'", ["@......."; 8])]);
    fs::create_dir_all(temp.path("demo/assets/fonts")).unwrap();
    fs::write(temp.path("demo/assets/fonts/digits.yaff"), text).unwrap();
    let images = built(&options);
    let f = &images.fonts[0];
    assert!(f.fixed);
    assert_eq!(f.glyphs.len(), 3, "never deduplicated");
    let inc = generated(&temp, "images.inc");
    assert!(inc.contains("DEF DIGITS_FONT_FIXED EQU 1"), "{inc}");
    assert!(inc.contains("CHARMAP \"0\", (\\2) + 1"), "{inc}");
}

#[test]
fn a_font_beside_images_is_in_the_same_generated_files() {
    let (temp, options) = starter("fonts-beside");
    main_font(&temp);
    crate::pngs::write_png(&temp.path("demo/assets/tiles/blocks.png"), 8, 8, |x, _| {
        if x < 4 {
            crate::pngs::WHITE
        } else {
            crate::pngs::BLACK
        }
    });
    let images = built(&options);
    assert_eq!((images.images.len(), images.fonts.len()), (1, 1));
    let asm = generated(&temp, "images.asm");
    assert!(
        asm.contains("BlocksTiles::") && asm.contains("MainFontGlyphs::"),
        "{asm}"
    );
}

#[test]
fn a_font_past_254_glyphs_is_refused_with_a_character_list_remedy() {
    let (temp, options) = starter("fonts-cap");
    big_font(&temp, "kanji", 0x4E00, 255);
    let made = failed(build(&rgbds(), &options));
    let images = made.images.expect("images");
    let p = images.first_error().expect("refused");
    assert_eq!(p.limit, ImageLimit::FontGlyphs);
    assert_eq!((p.count, p.max), (255, 254));
    assert_eq!(p.image.as_deref(), Some("demo/assets/fonts/kanji.yaff"));
    assert!(
        p.remedy.contains("demo/assets/fonts/kanji.charlist"),
        "{}",
        p.remedy
    );
    // A character list limits what the build emits, in the font's order.
    fs::write(temp.path("demo/assets/fonts/kanji.charlist"), "丁一\n").unwrap();
    let images = built(&options);
    let keys: Vec<&str> = images.fonts[0]
        .glyphs
        .iter()
        .map(|g| g.key.as_str())
        .collect();
    assert_eq!(keys, ["一", "丁"]);
    let inc = generated(&temp, "images.inc");
    assert!(inc.contains("DEF KANJI_FONT_GLYPHS EQU 2"), "{inc}");
    // A character the list names that the font lacks is a problem naming it.
    fs::write(temp.path("demo/assets/fonts/kanji.charlist"), "一x\n").unwrap();
    let made = failed(build(&rgbds(), &options));
    let p = made
        .images
        .unwrap()
        .first_error()
        .cloned()
        .expect("missing");
    assert_eq!(p.limit, ImageLimit::FontMissing);
    assert!(p.message.contains("'x' (u+0078)"), "{}", p.message);
}

#[test]
fn a_font_outside_the_subset_is_a_problem_at_its_line_and_column() {
    let (temp, options) = starter("fonts-bad");
    fs::create_dir_all(temp.path("demo/assets/fonts")).unwrap();
    fs::write(
        temp.path("demo/assets/fonts/main.yaff"),
        "'A':\n    @.\n    @x\n",
    )
    .unwrap();
    let made = failed(build(&rgbds(), &options));
    let p = made
        .images
        .unwrap()
        .first_error()
        .cloned()
        .expect("refused");
    assert_eq!(p.limit, ImageLimit::FontText);
    assert_eq!((p.line, p.column.map(|c| c.get())), (Some(3), Some(6)));
    assert!(p.remedy.contains("gbcli help fonts"), "{}", p.remedy);
}

#[test]
fn the_assets_op_returns_a_font_asset_for_requested_characters_with_no_cap() {
    let (temp, _) = starter("fonts-assets");
    big_font(&temp, "kanji", 0x4E00, 300);
    let project = || AssetsOptions::new(AssetsInput::Project("demo".into()), temp.0.clone());
    // Asking for nothing returns every glyph: the op has no cap.
    let made = assets(rgbds().images(), &project()).expect("converts");
    assert_eq!(made.images.fonts[0].glyphs.len(), 300);
    // Asking for 260 returns exactly those, in the font's order.
    let wanted: Vec<String> = (0..260)
        .rev()
        .map(|i| char::from_u32(0x4E00 + i).unwrap().to_string())
        .collect();
    let mut options = project();
    options
        .glyphs
        .insert("fonts/kanji.yaff".into(), wanted.clone());
    let made = assets(rgbds().images(), &options).expect("converts");
    let keys: Vec<String> = made.images.fonts[0]
        .glyphs
        .iter()
        .map(|g| g.key.clone())
        .collect();
    let mut sorted = wanted.clone();
    sorted.reverse();
    assert_eq!(keys, sorted);
    // A character the font lacks fails the request, naming it.
    options
        .glyphs
        .insert("fonts/kanji.yaff".into(), vec!["一".into(), "é".into()]);
    let e = assets(rgbds().images(), &options).expect_err("missing");
    let body = e.body();
    assert_eq!(body.code, ErrorCode::BuildFailed);
    assert!(body.message.contains("'é' (u+00E9)"), "{}", body.message);
    // A request for a font the project hasn't got is refused.
    let mut options = project();
    options.glyphs.insert("fonts/none.yaff".into(), vec![]);
    let e = assets(rgbds().images(), &options).expect_err("no font");
    assert!(
        e.body().message.contains("fonts/none.yaff"),
        "{}",
        e.body().message
    );
}

#[test]
fn a_manifest_converts_a_font_without_a_project() {
    let temp = Temp::new("fonts-manifest");
    fs::write(temp.path("talk.yaff"), yaff(&[("'i'", I)])).unwrap();
    let manifest = AssetManifest {
        fonts: vec![ManifestFont {
            font: "fonts/talk.yaff".into(),
            file: "talk.yaff".into(),
        }],
        ..AssetManifest::default()
    };
    let options = AssetsOptions::new(AssetsInput::Manifest(manifest), temp.0.clone());
    let made = assets(rgbds().images(), &options).expect("converts");
    let f = &made.images.fonts[0];
    assert_eq!((f.label.as_str(), f.glyphs.len()), ("Talk", 1));
    assert_eq!(f.glyphs[0].rows[0], 0x80);
}

mod verb {
    //! `gbcli font FILE` (the `font` op): a font read, a sample set by the
    //! layout rules, a preview, and `--fit`.

    use super::*;
    use gbcli_build::font;
    use gbcli_protocol::FontArgs;

    fn args(file: &str) -> FontArgs {
        FontArgs {
            file: file.into(),
            ..FontArgs::default()
        }
    }

    fn temp_font(name: &str) -> Temp {
        let temp = Temp::new(name);
        let text = yaff(&[
            ("u+0020", SPACE),
            ("'H'", H),
            ("'i'", I),
            ("\"heart\"", HEART),
        ]);
        fs::write(temp.path("main.yaff"), text).unwrap();
        temp
    }

    #[test]
    fn a_font_is_summarized_and_a_sample_set_by_the_layout_rules() {
        let temp = temp_font("font-verb");
        let made = font(&temp.0, &args("main.yaff")).expect("reads");
        assert_eq!(
            (made.glyphs, made.fixed, made.min_width, made.max_width),
            (4, false, Some(2), Some(7))
        );
        assert!(made.sample.is_none() && made.preview.is_none());
        let made = font(
            &temp.0,
            &FontArgs {
                sample: Some("Hi <heart>?".into()),
                ..args("main.yaff")
            },
        )
        .expect("sets");
        let s = made.sample.expect("a sample");
        // 5 + 2 + 3 + 7 pixels: the pen moves by each width; ? is missing.
        assert_eq!((s.width, s.tiles), (17, 3));
        assert_eq!(s.missing, ["?"]);
        // Tile 0: H at 0-4, i at 5-6, the space from 7; row 0 is H's
        // @...@ then i's @.
        assert_eq!(s.data[0], 0b1000_1100);
        assert_eq!(s.data.len(), 24);
    }

    #[test]
    fn fit_fails_past_its_width_and_passes_within_it() {
        let temp = temp_font("font-fit");
        let fit = |px| FontArgs {
            sample: Some("HiHi".into()),
            fit: Some(px),
            ..args("main.yaff")
        };
        let made = font(&temp.0, &fit(14)).expect("14 px fits in 14");
        assert_eq!(made.sample.unwrap().fit, Some(14));
        let e = font(&temp.0, &fit(13)).expect_err("too wide");
        assert_eq!(e.code, ErrorCode::PredicateFalse);
        assert!(
            e.message.contains("14") && e.message.contains("13"),
            "{}",
            e.message
        );
        let result = e.result.expect("the result rides along");
        assert_eq!(result.font.unwrap().sample.unwrap().width, 14);
        // --fit measures a sample, so it needs one.
        let e = font(
            &temp.0,
            &FontArgs {
                fit: Some(10),
                ..args("main.yaff")
            },
        )
        .expect_err("no sample");
        assert_eq!(e.code, ErrorCode::BadRequest);
    }

    #[test]
    fn a_preview_is_written_for_the_sample_or_the_whole_font() {
        let temp = temp_font("font-preview");
        let made = font(
            &temp.0,
            &FontArgs {
                preview: Some("all.png".into()),
                ..args("main.yaff")
            },
        )
        .expect("writes");
        let p = made.preview.expect("a preview");
        assert_eq!(p.path, "all.png");
        let bytes = fs::read(temp.path("all.png")).unwrap();
        assert_eq!(bytes.len() as u64, p.bytes);
        assert_eq!(&bytes[1..4], b"PNG");
        let made = font(
            &temp.0,
            &FontArgs {
                sample: Some("Hi".into()),
                preview: Some("line.png".into()),
                ..args("main.yaff")
            },
        )
        .expect("writes");
        assert!(made.preview.is_some() && temp.path("line.png").is_file());
    }

    #[test]
    fn a_font_that_doesnt_read_is_font_unreadable_at_its_line() {
        let temp = Temp::new("font-bad");
        fs::write(temp.path("bad.yaff"), "'A':\n    @\n    @@\n").unwrap();
        let e = font(&temp.0, &args("bad.yaff")).expect_err("bad");
        assert_eq!(e.code, ErrorCode::FontUnreadable);
        let p = e.result.unwrap().font_problem.expect("located");
        assert_eq!(
            (p.path.as_str(), p.line, p.column),
            ("bad.yaff", Some(3), Some(5))
        );
        let e = font(&temp.0, &args("none.yaff")).expect_err("missing");
        assert_eq!(e.code, ErrorCode::FontUnreadable);
    }
}
