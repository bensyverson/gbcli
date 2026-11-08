//! Flash sessions through the handler (leaf 4hM0UN): the run's notes and
//! the mask-ROM hint, restore across carts, the power cycle (across a
//! rebuild too), and `sav` / `power-cycle --sav` with the chip image.

use std::fs;

use gbcli_bundle::RomSource;
use gbcli_protocol::{
    CheckpointArgs, ErrorCode, FlashFrom, FlashNoteKind, Op, PowerCycleArgs, ReadArgs, Request,
    Response, RestoreArgs, ResultBody, SavArgs,
};

use super::Handler;
use super::test_support::{handler, request, run_frames};
use crate::fixture::{FLASH_SAVE, Fixture};

/// Bank 12's first byte, where FLASH_SAVE programs $42.
const SAVED_AT: usize = 0x3_0000;
const CHIP: usize = 0x20_0000;
/// Enough frames after the boot handoff for the save to read back.
const SAVE_FRAMES: u64 = 20;

fn ok(response: Response) -> (ResultBody, Vec<String>) {
    let warnings = response.warnings.clone();
    let body = response.outcome.unwrap_or_else(|e| panic!("{e:?}"));
    (body, warnings)
}

fn saved(h: &mut Handler) -> i64 {
    let (body, _) = ok(h.handle(&request(Op::Read(ReadArgs {
        reads: vec!["[$C000]".into()],
        since: None,
        allow_uninit: true,
    }))));
    body.reads.unwrap()[0].value.unwrap()
}

fn sav(h: &mut Handler, path: &std::path::Path) -> Response {
    h.handle(&request(Op::Sav(SavArgs {
        path: path.display().to_string(),
    })))
}

fn power_cycle(h: &mut Handler, sav: Option<&std::path::Path>) -> Response {
    h.handle(&request(Op::PowerCycle(PowerCycleArgs {
        sav: sav.map(|p| p.display().to_string()),
        rtc_advance: None,
    })))
}

#[test]
fn a_flash_session_saves_and_a_power_cycle_keeps_the_chip() {
    let f = Fixture::with_flash("flash-cycle", FLASH_SAVE, "generic-wr-aaa-a9");
    let mut h = handler(&f);
    let (body, warnings) = ok(run_frames(&mut h, SAVE_FRAMES));
    assert_eq!(body.flash_notes, None, "{warnings:?}");
    assert_eq!(saved(&mut h), 1);
    let (body, _) = ok(power_cycle(&mut h, None));
    assert_eq!(body.power_cycle.unwrap().flash, Some(FlashFrom::Head));
    let image = f.dir.join("game.flash.gb");
    ok(sav(&mut h, &f.dir.join("game.sav")));
    assert_eq!(fs::read(&image).unwrap()[SAVED_AT], 0x42);
}

#[test]
fn a_board_that_misses_the_commands_warns_on_the_run_with_rows() {
    let f = Fixture::with_flash("flash-notes", FLASH_SAVE, "generic-wr-aaa-aa");
    let mut h = handler(&f);
    let (body, warnings) = ok(run_frames(&mut h, SAVE_FRAMES));
    let notes = body.flash_notes.expect("notes");
    assert_eq!(notes[0].kind, FlashNoteKind::DataLines);
    assert_eq!(notes[0].addr, 0x0AAA);
    assert!(
        warnings.iter().any(|w| w.contains("D0 and D1")),
        "{warnings:?}"
    );
    let (again, _) = ok(run_frames(&mut h, 2));
    assert_eq!(again.flash_notes, None, "notes surface once, on their run");
}

#[test]
fn a_mask_rom_session_that_sends_flash_commands_names_the_flag() {
    let f = Fixture::with_rom("flash-hint", FLASH_SAVE);
    let mut h = handler(&f);
    let (body, warnings) = ok(run_frames(&mut h, SAVE_FRAMES));
    let notes = body.flash_notes.expect("the hint");
    assert_eq!(notes[0].kind, FlashNoteKind::NoChip);
    assert_eq!(notes[0].board.as_deref(), Some("generic-wr-aaa-a9"));
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("--flash generic-wr-aaa-a9")),
        "{warnings:?}"
    );
}

#[test]
fn a_checkpoint_from_a_flash_session_is_refused_by_a_mask_rom_one() {
    let f = Fixture::with_flash("flash-restore", FLASH_SAVE, "generic-wr-aaa-a9");
    let mut h = handler(&f);
    ok(run_frames(&mut h, SAVE_FRAMES));
    ok(h.handle(&request(Op::Checkpoint(CheckpointArgs {
        name: "a".into(),
    }))));
    f.bundle
        .session("mask")
        .unwrap()
        .create(&RomSource {
            rom: f.dir.join("demo.gb"),
            sym: None,
            model: None,
            seed: None,
            flash: None,
        })
        .unwrap();
    let sessions = f.dir.join(".gbcli").join("sessions");
    fs::copy(
        sessions.join("demo/checkpoints/a.gbck"),
        sessions.join("mask/checkpoints/a.gbck"),
    )
    .unwrap();
    let restore = Request {
        session: Some("mask".into()),
        ..request(Op::Restore(RestoreArgs { name: "a".into() }))
    };
    let err = h.handle(&restore).outcome.unwrap_err();
    assert_eq!(err.code, ErrorCode::BadRequest, "{err:?}");
    assert!(err.remedy.contains("--flash generic-wr-aaa-a9"), "{err:?}");
}

#[test]
fn a_power_cycle_across_a_rebuild_carries_the_flash_save() {
    let f = Fixture::with_flash("flash-rebuild", FLASH_SAVE, "generic-wr-aaa-a9");
    let mut h = handler(&f);
    ok(run_frames(&mut h, SAVE_FRAMES));
    let mut rebuilt = FLASH_SAVE.to_vec();
    rebuilt[0x1_C000 + 1] = 0x77; // in the last bank, past the code
    fs::write(f.dir.join("demo.gb"), &rebuilt).unwrap();
    let (body, _) = ok(power_cycle(&mut h, None));
    let made = body.power_cycle.unwrap();
    assert!(made.rom_rebuilt);
    assert_eq!(made.flash, Some(FlashFrom::Head));
    ok(sav(&mut h, &f.dir.join("game.sav")));
    let image = fs::read(f.dir.join("game.flash.gb")).unwrap();
    assert_eq!(image[SAVED_AT], 0x42, "the save crossed the rebuild");
    assert_eq!(image[0x1_C001], 0x77, "and the code is the new build's");
}

#[test]
fn sav_writes_the_chip_image_beside_the_sav_and_power_cycle_imports_it() {
    let f = Fixture::with_flash("flash-sav", FLASH_SAVE, "generic-wr-aaa-a9");
    let mut h = handler(&f);
    ok(run_frames(&mut h, SAVE_FRAMES));
    let path = f.dir.join("game.sav");
    let (body, _) = ok(sav(&mut h, &path));
    let made = body.sav.unwrap();
    assert_eq!(made.path.as_deref(), Some(path.to_str().unwrap()));
    assert_eq!(made.bytes, 8 * 1024, "the cart RAM, as before");
    let image_path = f.dir.join("game.flash.gb");
    let flash = made.flash.expect("the chip image");
    assert_eq!(flash.path, image_path.display().to_string());
    assert_eq!(flash.bytes, CHIP as u64);
    let image = fs::read(&image_path).unwrap();
    assert_eq!(image.len(), CHIP);
    assert_eq!(&image[..FLASH_SAVE.len()], FLASH_SAVE, "the game, as built");

    // Another session imports it: its chip starts with the save.
    let g = Fixture::with_flash("flash-sav-import", FLASH_SAVE, "generic-wr-aaa-a9");
    let mut other = handler(&g);
    let (body, warnings) = ok(power_cycle(&mut other, Some(&path)));
    let made = body.power_cycle.unwrap();
    assert_eq!(made.flash, Some(FlashFrom::Image));
    assert_eq!(made.flash_bytes, Some(CHIP as u64));
    assert!(warnings.is_empty(), "{warnings:?}");
    ok(sav(&mut other, &g.dir.join("out.sav")));
    assert_eq!(
        fs::read(g.dir.join("out.flash.gb")).unwrap()[SAVED_AT],
        0x42
    );
}

#[test]
fn an_image_of_the_wrong_size_is_refused_and_changes_nothing() {
    let f = Fixture::with_flash("flash-bad-image", FLASH_SAVE, "generic-wr-aaa-a9");
    let mut h = handler(&f);
    let path = f.dir.join("bad.sav");
    fs::write(&path, vec![0xFF; 8 * 1024]).unwrap();
    fs::write(f.dir.join("bad.flash.gb"), vec![0xFF; 0x1000]).unwrap();
    let err = power_cycle(&mut h, Some(&path)).outcome.unwrap_err();
    assert!(err.message.contains("0x200000"), "{err:?}");
}
