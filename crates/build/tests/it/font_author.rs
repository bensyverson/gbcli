//! Font authoring (leaf mDCZDQ): working copies written and saved back
//! unchanged, the build refusing while one is newer than its font, every
//! `.chars`/art disagreement a problem naming its cell, BDF import,
//! export, `new` with guides, `diff`, and composition of requested
//! letters.

use crate::common;

use std::fs;
use std::time::{Duration, SystemTime};

use gbcli_build::{
    AssetsInput, AssetsOptions, BuildError, assets, build, font_diff, font_edit, font_export,
    font_import, font_new, font_save,
};
use gbcli_protocol::{
    ErrorCode, FontDiffArgs, FontEditArgs, FontExportArgs, FontFileKind, FontForm, FontImportArgs,
    FontNewArgs, FontSaveArgs, GlyphChangeKind, GlyphOrigin, TileAt,
};

use common::{Temp, rgbds, starter};

/// A font with comments, properties, a named glyph, a mark and a property
/// value that a careless writer would turn into a label.
const FONT: &str = "\
# Talk, a test font.

name: Talk
notice:
    Free to use:
ascent: 6

# The space: 3 px of paper.
u+0020:
    ...
    ...
    ...
    ...
    ...
    ...
    ...
    ...

'e':
    .....
    .....
    .@@@.
    @...@
    @@@@@
    @....
    .@@@.
    .....

'E':
    @@@@@
    @....
    @@@@.
    @....
    @....
    @@@@@
    .....
    .....

# The acute, for composing.
u+0301:
    ..@.
    .@..
    ....
    ....
    ....
    ....
    ....
    ....

\"heart\":
    .@@.@@.
    @@@@@@@
    @@@@@@@
    .@@@@@.
    ..@@@..
    ...@...
    .......
    .......

# The end.
";

fn project(name: &str) -> (Temp, gbcli_build::BuildOptions) {
    let (temp, options) = starter(name);
    fs::create_dir_all(temp.path("demo/assets/fonts")).unwrap();
    fs::write(temp.path("demo/assets/fonts/talk.yaff"), FONT).unwrap();
    // An hour old, so a copy written now is newer.
    let old = SystemTime::now() - Duration::from_secs(3600);
    fs::File::options()
        .write(true)
        .open(temp.path("demo/assets/fonts/talk.yaff"))
        .unwrap()
        .set_modified(old)
        .unwrap();
    (temp, options)
}

const TALK: &str = "demo/assets/fonts/talk.yaff";

fn edit(temp: &Temp, form: FontForm) -> gbcli_protocol::FontWritten {
    let args = FontEditArgs {
        file: TALK.into(),
        form,
    };
    font_edit(&temp.0, &args).expect("edits")
}

fn save(
    temp: &Temp,
    file: &str,
) -> Result<gbcli_protocol::FontWritten, Box<gbcli_protocol::ErrorBody>> {
    font_save(&temp.0, &FontSaveArgs { file: file.into() })
}

#[test]
fn a_font_round_trips_through_a_png_and_a_bdf_working_copy_unchanged() {
    let (temp, _) = project("author-round-trip");
    let made = edit(&temp, FontForm::Png);
    let paths: Vec<(&str, FontFileKind)> = made
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.kind))
        .collect();
    assert_eq!(
        paths,
        [
            ("demo/build/font-edit/talk.png", FontFileKind::Png),
            ("demo/build/font-edit/talk.chars", FontFileKind::Chars),
            // The record of the font the copy was made from (leaf 2xiOiv).
            ("demo/build/font-edit/talk.base", FontFileKind::Base)
        ]
    );
    assert_eq!(made.glyphs, 5);
    // The copy's own path finds its font.
    let saved = save(&temp, "demo/build/font-edit/talk.png").expect("saves");
    assert_eq!(saved.font, TALK);
    assert_eq!(saved.removed.len(), 3);
    assert!(!temp.path("demo/build/font-edit/talk.png").exists());
    assert_eq!(fs::read_to_string(temp.path(TALK)).unwrap(), FONT);
    // And through BDF, saved by the font's path.
    let made = edit(&temp, FontForm::Bdf);
    assert_eq!(made.files[0].path, "demo/build/font-edit/talk.bdf");
    save(&temp, TALK).expect("saves");
    assert_eq!(fs::read_to_string(temp.path(TALK)).unwrap(), FONT);
}

#[test]
fn an_edit_in_the_copy_reaches_the_font_and_a_second_copy_is_refused() {
    let (temp, _) = project("author-change");
    edit(&temp, FontForm::Bdf);
    let again = font_edit(
        &temp.0,
        &FontEditArgs {
            file: TALK.into(),
            form: FontForm::Png,
        },
    )
    .expect_err("one copy at a time");
    assert!(again.remedy.contains("gbcli font save"), "{}", again.remedy);
    let bdf = temp.path("demo/build/font-edit/talk.bdf");
    let text = fs::read_to_string(&bdf).unwrap();
    // e is the second glyph: widen it by a column (DWIDTH and BBX).
    let text = text.replacen("DWIDTH 5 0\nBBX 5 8 0 -2", "DWIDTH 6 0\nBBX 6 8 0 -2", 2);
    fs::write(&bdf, text).unwrap();
    save(&temp, TALK).expect("saves");
    let font = fs::read_to_string(temp.path(TALK)).unwrap();
    assert!(font.contains("'e':\n    ......\n"), "{font}");
    assert!(
        font.contains("# The space: 3 px of paper."),
        "comments kept: {font}"
    );
}

#[test]
fn a_newer_working_copy_blocks_the_build_naming_font_save() {
    let (temp, options) = project("author-blocks");
    edit(&temp, FontForm::Png);
    let body = match build(&rgbds(), &options) {
        Err(e @ BuildError::Layout { .. }) => e.body(),
        other => panic!("expected a refusal, got {other:?}"),
    };
    assert!(
        body.message.contains("demo/build/font-edit/talk.png"),
        "{}",
        body.message
    );
    assert!(
        body.remedy
            .contains("gbcli font save demo/assets/fonts/talk.yaff"),
        "{}",
        body.remedy
    );
    // Saved, it builds.
    save(&temp, TALK).expect("saves");
    build(&rgbds(), &options).expect("builds");
}

#[test]
fn every_chars_disagreement_is_a_problem_naming_the_cell() {
    let (temp, _) = project("author-cells");
    edit(&temp, FontForm::Png);
    let chars = temp.path("demo/build/font-edit/talk.chars");
    // Two more characters than cells drawn, and the heart listed twice.
    fs::write(&chars, "<u+0020>eE<u+0301><heart>xy<heart>\n").unwrap();
    let e = save(&temp, TALK).expect_err("disagrees");
    assert_eq!(e.code, ErrorCode::FontUnreadable);
    let problems = e.result.expect("result").font_problems;
    let cells: Vec<Option<TileAt>> = problems.iter().map(|p| p.cell).collect();
    for x in [5, 6, 7] {
        assert!(
            cells.contains(&Some(TileAt { x, y: 0 })),
            "{x}: {problems:#?}"
        );
    }
    assert!(
        problems
            .iter()
            .all(|p| p.path.starts_with("demo/build/font-edit/talk."))
    );
    // Nothing was written or removed.
    assert_eq!(fs::read_to_string(temp.path(TALK)).unwrap(), FONT);
    assert!(chars.exists());
}

#[test]
fn a_requested_letter_is_composed_from_its_base_and_mark_and_reported_as_composed() {
    let (temp, _) = project("author-compose");
    let mut options = AssetsOptions::new(AssetsInput::Project("demo".into()), temp.0.clone());
    options.glyphs.insert(
        "fonts/talk.yaff".into(),
        vec!["é".into(), "e".into(), "É".into()],
    );
    let made = assets(&options).expect("converts");
    // The starter's own font (main.yaff) sorts first.
    let font = talk(&made.images.fonts);
    let got: Vec<(&str, GlyphOrigin, u8)> = font
        .glyphs
        .iter()
        .map(|g| (g.key.as_str(), g.origin, g.width))
        .collect();
    assert_eq!(
        got,
        [
            ("e", GlyphOrigin::Drawn, 5),
            ("é", GlyphOrigin::Composed, 5),
            ("É", GlyphOrigin::Composed, 5)
        ]
    );
    assert_eq!(
        font.glyphs[1].rows,
        [0x20, 0x40, 0x70, 0x88, 0xF8, 0x80, 0x70, 0]
    );
    // É's acute lands on E's top bar: a warning naming the glyph.
    let overlaps = &made.images.font_overlaps;
    assert_eq!(overlaps.len(), 1, "{overlaps:#?}");
    assert_eq!(
        (overlaps[0].key.as_str(), overlaps[0].rows.clone()),
        ("É", vec![0])
    );
    // A letter neither drawn nor composable is still missing.
    options
        .glyphs
        .insert("fonts/talk.yaff".into(), vec!["ë".into()]);
    let e = assets(&options).expect_err("missing");
    assert!(
        e.body().message.contains("'ë' (u+00EB)"),
        "{}",
        e.body().message
    );
}

#[test]
fn a_character_list_composes_for_the_build_and_its_charmap() {
    let (temp, options) = project("author-charlist");
    fs::write(temp.path("demo/assets/fonts/talk.charlist"), "eé <heart>").unwrap();
    fs::write(
        temp.path("demo/src/talk_probe.asm"),
        "INCLUDE \"images.inc\"\n\tTALK_FONT_CHARMAP t, 0\nSECTION \"t\", ROMX\n\tPUSHC t\nT:: db \"é\"\n\tPOPC\n",
    )
    .unwrap();
    let made = build(&rgbds(), &options).expect("builds");
    let images = made.images.unwrap();
    let keys: Vec<(String, GlyphOrigin)> = talk(&images.fonts)
        .glyphs
        .iter()
        .map(|g| (g.key.clone(), g.origin))
        .collect();
    assert_eq!(
        keys,
        [
            (" ".into(), GlyphOrigin::Drawn),
            ("e".into(), GlyphOrigin::Drawn),
            ("heart".into(), GlyphOrigin::Drawn),
            ("é".into(), GlyphOrigin::Composed)
        ]
    );
}

/// A small trimmed BDF: A and a period.
const BDF: &str = "STARTFONT 2.1\nFONT test\nSIZE 8 75 75\nFONTBOUNDINGBOX 8 8 0 -2\n\
STARTPROPERTIES 3\nFONT_ASCENT 6\nFONT_DESCENT 2\nCOPYRIGHT \"Public domain\"\nENDPROPERTIES\n\
CHARS 2\nSTARTCHAR A\nENCODING 65\nDWIDTH 4 0\nBBX 3 2 0 0\nBITMAP\n40\nA0\nENDCHAR\n\
STARTCHAR period\nENCODING 46\nDWIDTH 2 0\nBBX 1 1 0 0\nBITMAP\n80\nENDCHAR\nENDFONT\n";

#[test]
fn a_bdf_imports_with_its_properties_and_never_over_a_font() {
    let temp = Temp::new("author-import");
    fs::write(temp.path("m.bdf"), BDF).unwrap();
    fs::write(temp.path("list.txt"), "A¿\n").unwrap();
    let args = FontImportArgs {
        file: "m.bdf".into(),
        chars: Some("list.txt".into()),
        out: None,
    };
    let made = font_import(&temp.0, &args).expect("imports");
    assert_eq!((made.font.as_str(), made.glyphs), ("m.yaff", 1));
    assert_eq!(made.missing, ["¿"]);
    let text = fs::read_to_string(temp.path("m.yaff")).unwrap();
    assert!(text.contains("copyright: Public domain"), "{text}");
    assert!(
        text.contains("    ....\n    .@..\n    @.@.\n    ....\n    ....\n"),
        "{text}"
    );
    gbcli_font::read(&text).expect("reads back");
    let e = font_import(&temp.0, &args).expect_err("exists");
    assert!(e.remedy.contains("--out"), "{}", e.remedy);
}

#[test]
fn export_writes_where_asked_and_new_writes_a_guided_copy_to_save() {
    let (temp, _) = project("author-new");
    let made = font_export(
        &temp.0,
        &FontExportArgs {
            file: TALK.into(),
            png: Some("out/talk.png".into()),
            bdf: Some("out/talk.bdf".into()),
            guides: true,
        },
    )
    .expect("exports");
    let kinds: Vec<FontFileKind> = made.files.iter().map(|f| f.kind).collect();
    assert_eq!(
        kinds,
        [FontFileKind::Png, FontFileKind::Chars, FontFileKind::Bdf]
    );
    assert!(temp.path("out/talk.chars").exists());
    // `new` makes a guided copy and a guide sheet; the font comes at save.
    let args = FontNewArgs {
        name: "hud".into(),
        chars: "ascii".into(),
        guide_sheet: Some("sheet.png".into()),
    };
    let made = font_new(&temp.0, &args).expect("new");
    assert_eq!(made.font, "assets/fonts/hud.yaff");
    assert_eq!(made.files.last().unwrap().kind, FontFileKind::GuideSheet);
    let chars = fs::read_to_string(temp.path("build/font-edit/hud.chars")).unwrap();
    assert_eq!(chars.lines().count(), 6);
    // Undrawn, it doesn't save: each listed cell asks to be drawn.
    let e = save(&temp, "assets/fonts/hud.yaff").expect_err("empty");
    assert_eq!(e.result.unwrap().font_problems.len(), 95);
    // Drawn as paper in its first cell, with the rest left empty, it saves.
    let png = temp.path("build/font-edit/hud.png");
    let mut image = gbcli_pixels::decode_png(&fs::read(&png).unwrap()).unwrap();
    for y in 0..8 {
        for x in 0..3 {
            image.rgba[(y * 128 + x) as usize] = [0xFF; 4];
        }
    }
    fs::write(&png, gbcli_pixels::encode_png(&image)).unwrap();
    fs::write(temp.path("build/font-edit/hud.chars"), "<u+0020>\n").unwrap();
    let saved = save(&temp, "assets/fonts/hud.yaff").expect("saves");
    assert_eq!(saved.glyphs, 1);
    let text = fs::read_to_string(temp.path("assets/fonts/hud.yaff")).unwrap();
    assert_eq!(text, format!("u+0020:\n{}", "    ...\n".repeat(8)));
    assert!(font_new(&temp.0, &args).is_err(), "exists now");
}

#[test]
fn diff_names_each_changed_glyph_and_draws_them() {
    let (temp, _) = project("author-diff");
    let changed = FONT
        .replace("'e':\n    .....\n", "'e':\n    ..@..\n")
        .replace(
            "\"heart\":",
            "'x':\n    @\n    @\n    @\n    @\n    @\n    @\n    @\n    @\n\n\"heart\":",
        );
    fs::write(temp.path("b.yaff"), changed).unwrap();
    let made = font_diff(
        &temp.0,
        &FontDiffArgs {
            a: TALK.into(),
            b: "b.yaff".into(),
            png: Some("diff.png".into()),
        },
    )
    .expect("diffs");
    let got: Vec<(&str, GlyphChangeKind)> = made
        .glyphs
        .iter()
        .map(|g| (g.key.as_str(), g.change))
        .collect();
    assert_eq!(
        got,
        [
            ("e", GlyphChangeKind::Changed),
            ("x", GlyphChangeKind::Added)
        ]
    );
    assert!(!made.reordered);
    assert!(temp.path("diff.png").exists());
}

/// The test project's own font, beside the starter's main.yaff.
fn talk(fonts: &[gbcli_protocol::FontAsset]) -> &gbcli_protocol::FontAsset {
    fonts
        .iter()
        .find(|f| f.source.ends_with("talk.yaff"))
        .expect("talk.yaff")
}
