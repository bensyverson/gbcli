//! Rows of the generated manifest (`scripts/corpus/build_manifest.py`) whose
//! rules were found wrong by running them; see
//! `project/2026-11-06-sameboy-baseline.md`.

use std::path::PathBuf;

use gbcli_corpus::manifest::{self, Exit, Palette, PassRule, Row};

fn rows() -> Vec<Row> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let text = std::fs::read_to_string(root.join("tests/corpus/manifest.json")).unwrap();
    manifest::parse(&text).unwrap().rows
}

fn row<'a>(rows: &'a [Row], id: &str) -> &'a Row {
    rows.iter().find(|r| r.id == id).expect("row in manifest")
}

#[test]
fn mgb_oam_dma_halt_sprites_is_judged_after_a_fixed_time() {
    // The ROM halts with no interrupt enabled after its DMA and never reaches LD B,B.
    let rows = rows();
    let row = row(&rows, "mooneye/madness/mgb_oam_dma_halt_sprites.gb");
    assert!(
        matches!(row.exit, Exit::EmulatedSeconds { .. }),
        "{:?}",
        row.exit
    );
}

#[test]
fn oam_bug_on_cgb_is_judged_in_its_references_palette() {
    // c-sp's oam_bug-cgb.png has $F8 white: channels as x << 3.
    let rows = rows();
    let row = row(&rows, "blargg/oam_bug/oam_bug.gb@cgb");
    assert!(
        matches!(
            row.pass,
            PassRule::Framebuffer {
                palette: Palette::CgbRgb555Shift,
                ..
            }
        ),
        "{:?}",
        row.pass
    );
}
