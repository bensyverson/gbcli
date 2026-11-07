//! The `sym` and `checkpoints` read verbs.

use gbcli_protocol::{
    CheckpointArgs, CheckpointRow, Op, Rev, RunArgs, SymArgs, SymbolArea, SymbolRow,
};

use super::Handler;
use super::test_support::{handler, request, run_frames};
use crate::fixture::{Fixture, V2, V2_SYM};

fn sym(h: &mut Handler, pattern: &str) -> (Vec<SymbolRow>, Vec<String>) {
    let response = h.handle(&request(Op::Sym(SymArgs {
        pattern: pattern.into(),
    })));
    let warnings = response.warnings.clone();
    let rows = response.outcome.expect("sym").symbols.expect("rows");
    (rows, warnings)
}

fn names(rows: &[SymbolRow]) -> Vec<&str> {
    rows.iter().map(|r| r.name.as_str()).collect()
}

#[test]
fn sym_lists_the_symbols_a_glob_matches_in_address_order() {
    let f = Fixture::with_sym("sym-glob");
    let mut h = handler(&f);
    let (rows, _) = sym(&mut h, "Main*");
    assert_eq!(names(&rows), ["Main", "Main.loop"]);
    assert_eq!(
        rows[1],
        SymbolRow {
            name: "Main.loop".into(),
            bank: 0,
            addr: 0x015E,
            area: SymbolArea::Rom0,
        }
    );
    let (rows, _) = sym(&mut h, "w?????");
    assert_eq!(names(&rows), ["wFrame", "wSpeed", "wNever", "wStack"]);
    assert!(rows.iter().all(|r| r.area == SymbolArea::Wram0));
    assert_eq!(sym(&mut h, "*").0.len(), 9);
    assert!(sym(&mut h, "main").0.is_empty(), "names match as spelled");
}

#[test]
fn sym_without_a_sym_file_is_empty_and_says_why() {
    let f = Fixture::new("sym-none");
    let mut h = handler(&f);
    let (rows, warnings) = sym(&mut h, "*");
    assert!(rows.is_empty());
    assert!(warnings.iter().any(|w| w.contains(".sym")), "{warnings:?}");
    let meta = f.bundle.session("demo").unwrap().read_meta().unwrap();
    assert_eq!(meta.rev, Rev(0), "a read verb");
}

fn checkpoints(h: &mut Handler) -> Vec<CheckpointRow> {
    let response = h.handle(&request(Op::Checkpoints));
    response.outcome.expect("listed").checkpoints.expect("rows")
}

#[test]
fn checkpoints_lists_named_ones_then_history_with_rev_frame_and_rom_match() {
    let f = Fixture::new("listing");
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 2).is_ok());
    let saved = h.handle(&request(Op::Checkpoint(CheckpointArgs {
        name: "f2".into(),
    })));
    assert!(saved.is_ok());
    let ran = h.handle(&request(Op::Run(RunArgs {
        frames: Some(3),
        checkpoint: Some("f5".into()),
        ..RunArgs::default()
    })));
    assert!(ran.is_ok());
    assert!(run_frames(&mut h, 1).is_ok());

    let rows = checkpoints(&mut h);
    let row = |at: &str, rev: u64, frame: i64| CheckpointRow {
        at: at.into(),
        rev: Some(Rev(rev)),
        frame: Some(frame),
        rom_matches: true,
    };
    assert_eq!(
        rows,
        vec![
            row("f2", 1, 2),
            row("f5", 2, 5),
            row("~1", 2, 5),
            row("~2", 1, 2)
        ]
    );
    let meta = f.bundle.session("demo").unwrap().read_meta().unwrap();
    assert_eq!(meta.rev, Rev(3), "listing doesn't move the head");

    // After a rebuild, none of them was saved from the ROM on disk.
    f.rebuild(V2, V2_SYM);
    let rows = checkpoints(&mut handler(&f));
    assert!(rows.iter().all(|r| !r.rom_matches), "{rows:?}");
}
