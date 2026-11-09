//! What font authoring must never overwrite (rev-fonts' review of
//! 6feaf3e, leaf 2xiOiv): a save never writes a working copy's glyphs over
//! a font changed since the copy was made, and export, `new
//! --guide-sheet` and `diff --png` never write over a font or into the
//! working copies' folder.

use std::fs;
use std::time::{Duration, SystemTime};

use gbcli_build::{font_diff, font_edit, font_export, font_new, font_save};
use gbcli_protocol::{
    FontDiffArgs, FontEditArgs, FontExportArgs, FontFileKind, FontForm, FontNewArgs, FontSaveArgs,
};

use crate::common::Temp;

const FONT: &str = "'A':\n    @@.\n    @@.\n    @@.\n    @@.\n    @@.\n    @@.\n    @@.\n    @@.\n";
const TALK: &str = "assets/fonts/talk.yaff";
const BDF_COPY: &str = "build/font-edit/talk.bdf";
const BASE: &str = "build/font-edit/talk.base";

fn aged(temp: &Temp, rel: &str, secs: u64) {
    fs::File::options()
        .write(true)
        .open(temp.path(rel))
        .unwrap()
        .set_modified(SystemTime::now() - Duration::from_secs(secs))
        .unwrap();
}

fn project(name: &str) -> Temp {
    let temp = Temp::new(name);
    fs::create_dir_all(temp.path("assets/fonts")).unwrap();
    fs::write(temp.path(TALK), FONT).unwrap();
    aged(&temp, TALK, 3600);
    temp
}

fn edit(temp: &Temp, form: FontForm) -> gbcli_protocol::FontWritten {
    font_edit(
        &temp.0,
        &FontEditArgs {
            file: TALK.into(),
            form,
        },
    )
    .expect("edits")
}

fn save(temp: &Temp) -> Result<gbcli_protocol::FontWritten, Box<gbcli_protocol::ErrorBody>> {
    font_save(&temp.0, &FontSaveArgs { file: TALK.into() })
}

/// The font edited by hand after the copy was made: a new glyph.
fn edited() -> String {
    format!("{FONT}\n'B':\n{}", "    @.\n".repeat(8))
}

#[test]
fn a_save_refuses_when_the_font_changed_after_its_copy_was_written() {
    let temp = project("probe-newer-source");
    edit(&temp, FontForm::Bdf);
    aged(&temp, BDF_COPY, 1800);
    let edited = edited();
    fs::write(temp.path(TALK), &edited).unwrap();
    let r = save(&temp);
    let now = fs::read_to_string(temp.path(TALK)).unwrap();
    assert!(
        r.is_err() && now == edited,
        "the save replaced a newer font's glyphs with its older copy's; the font is now:\n{now}"
    );
    // Naming both files, and the way out; the copy's edits are kept.
    let e = r.unwrap_err();
    assert!(
        e.message.contains(TALK) && e.message.contains(BDF_COPY),
        "{}",
        e.message
    );
    assert!(
        e.remedy.contains("gbcli font edit") && e.remedy.contains(BASE),
        "{}",
        e.remedy
    );
    assert!(temp.path(BDF_COPY).exists());
}

#[test]
fn a_copy_records_its_base_and_a_normal_save_removes_it() {
    let temp = project("base-recorded");
    let made = edit(&temp, FontForm::Png);
    assert_eq!(made.files.last().unwrap().path, BASE);
    assert_eq!(made.files.last().unwrap().kind, FontFileKind::Base);
    // Saving the copy unchanged, though written later than the font, is
    // no conflict.
    let saved = save(&temp).expect("saves");
    assert!(saved.removed.contains(&BASE.to_string()), "{saved:?}");
    assert!(!temp.path(BASE).exists());
    assert_eq!(fs::read_to_string(temp.path(TALK)).unwrap(), FONT);
}

#[test]
fn a_copy_with_no_base_saves_as_before_which_is_also_the_way_to_force_one() {
    let temp = project("base-missing");
    edit(&temp, FontForm::Bdf);
    fs::remove_file(temp.path(BASE)).unwrap();
    fs::write(temp.path(TALK), edited()).unwrap();
    save(&temp).expect("a copy made before bases were recorded still saves");
    assert_eq!(fs::read_to_string(temp.path(TALK)).unwrap(), FONT);
}

#[test]
fn a_new_fonts_save_refuses_when_the_font_was_made_meanwhile() {
    let temp = Temp::new("base-new");
    let args = FontNewArgs {
        name: "talk".into(),
        chars: "ascii".into(),
        guide_sheet: None,
    };
    font_new(&temp.0, &args).expect("new");
    fs::create_dir_all(temp.path("assets/fonts")).unwrap();
    fs::write(temp.path(TALK), FONT).unwrap();
    let e = save(&temp).expect_err("the font exists now");
    assert!(e.message.contains(TALK), "{}", e.message);
    assert_eq!(fs::read_to_string(temp.path(TALK)).unwrap(), FONT);
}

#[test]
fn a_garbled_base_is_refused_naming_it() {
    let temp = project("base-garbled");
    edit(&temp, FontForm::Bdf);
    fs::write(temp.path(BASE), "not a base\n").unwrap();
    let e = save(&temp).expect_err("garbled");
    assert!(e.message.contains(BASE), "{}", e.message);
}

#[test]
fn an_export_never_writes_over_a_font_or_an_unsaved_working_copy() {
    let temp = project("probe-export-over");
    edit(&temp, FontForm::Png);
    let copy = fs::read(temp.path("build/font-edit/talk.png")).unwrap();
    // Over the font itself.
    let over_font = font_export(
        &temp.0,
        &FontExportArgs {
            file: TALK.into(),
            png: None,
            bdf: Some(TALK.into()),
            guides: false,
        },
    );
    assert!(
        over_font.is_err() && fs::read_to_string(temp.path(TALK)).unwrap() == FONT,
        "export wrote BDF over the .yaff"
    );
    // Over the working copy, whose edits aren't saved.
    let over_copy = font_export(
        &temp.0,
        &FontExportArgs {
            file: TALK.into(),
            png: Some("build/font-edit/talk.png".into()),
            bdf: None,
            guides: false,
        },
    );
    assert!(
        over_copy.is_err() && fs::read(temp.path("build/font-edit/talk.png")).unwrap() == copy,
        "export wrote over the unsaved working copy"
    );
}

fn export(temp: &Temp, png: Option<&str>, bdf: Option<&str>) -> Result<(), String> {
    font_export(
        &temp.0,
        &FontExportArgs {
            file: TALK.into(),
            png: png.map(Into::into),
            bdf: bdf.map(Into::into),
            guides: false,
        },
    )
    .map(|_| ())
    .map_err(|e| format!("{} / {}", e.message, e.remedy))
}

#[test]
fn an_export_refuses_any_font_and_the_copies_folder_but_rewrites_its_own_output() {
    let temp = project("export-rule");
    fs::write(temp.path("other.yaff"), FONT).unwrap();
    let e = export(&temp, None, Some("other.yaff")).expect_err("another font");
    assert!(e.contains("other.yaff"), "{e}");
    // Nothing in the copies' folder, even with no copy there: a file there
    // would be taken for one.
    let e = export(&temp, None, Some("build/font-edit/talk.bdf")).expect_err("copies folder");
    assert!(e.contains("build/font-edit"), "{e}");
    assert!(!temp.path("build/font-edit/talk.bdf").exists());
    // Refused as a whole: nothing written when one output is refused.
    assert!(export(&temp, Some("art.png"), Some("x.yaff")).is_err());
    assert!(!temp.path("art.png").exists());
    // Exporting again over the last export, the everyday loop, works.
    export(&temp, Some("out/talk.png"), Some("out/talk.bdf")).expect("first");
    export(&temp, Some("out/talk.png"), Some("out/talk.bdf")).expect("again");
}

#[test]
fn a_guide_sheet_and_a_diff_png_never_land_on_a_font_or_in_the_copies_folder() {
    let temp = project("sheet-diff-rule");
    let new = |sheet: &str| {
        font_new(
            &temp.0,
            &FontNewArgs {
                name: "hud".into(),
                chars: "ascii".into(),
                guide_sheet: Some(sheet.into()),
            },
        )
    };
    assert!(new(TALK).is_err(), "a guide sheet over a font");
    assert_eq!(fs::read_to_string(temp.path(TALK)).unwrap(), FONT);
    assert!(
        new("build/font-edit/hud-sheet.png").is_err(),
        "a guide sheet in the copies' folder"
    );
    assert!(
        !temp.path("build/font-edit/hud.png").exists(),
        "a refused new writes no copy"
    );
    new("hud-sheet.png").expect("elsewhere");
    let diff = |png: &str| {
        font_diff(
            &temp.0,
            &FontDiffArgs {
                a: TALK.into(),
                b: TALK.into(),
                png: Some(png.into()),
            },
        )
    };
    assert!(diff(TALK).is_err(), "a diff PNG over a font");
    assert_eq!(fs::read_to_string(temp.path(TALK)).unwrap(), FONT);
    let copy = fs::read(temp.path("build/font-edit/hud.png")).unwrap();
    assert!(diff("build/font-edit/hud.png").is_err(), "over a copy");
    assert_eq!(
        fs::read(temp.path("build/font-edit/hud.png")).unwrap(),
        copy
    );
    diff("diff.png").expect("elsewhere");
    diff("diff.png").expect("and again");
}
