//! Session state across requests: what a request starts from, what it saves,
//! and which machine the handler keeps warm.

use std::time::Duration;

use gbcli_bundle::RomSource;
use gbcli_core::{Model, Region};
use gbcli_protocol::{
    CheckpointArgs, ErrorCode, Op, ReadArgs, RestoreArgs, Rev, RunArgs, StepArgs, StepKind,
    StopReason, WatchAccess, WatchArgs, WriteArg, WriteArgs, WriteKind,
};

use super::Handler;
use super::test_support::{handler, request, run_frames};
use crate::fixture::{Fixture, V2, V2_SYM, V3, V3_SYM};

fn poke(target: &str, byte: u8) -> Op {
    Op::Write(WriteArgs {
        writes: vec![WriteArg {
            kind: WriteKind::Poke,
            target: target.into(),
            bytes: vec![byte],
        }],
    })
}

fn read_one(h: &mut Handler, expr: &str) -> i64 {
    let response = h.handle(&request(Op::Read(ReadArgs {
        reads: vec![expr.into()],
        since: None,
        allow_uninit: false,
    })));
    assert!(response.is_ok(), "{:?}", response.error());
    response.outcome.unwrap().reads.unwrap()[0].value
}

#[test]
fn a_once_watch_fires_once_across_two_runs_and_leaves_the_session_file() {
    let f = Fixture::new("once");
    let mut h = handler(&f);
    let made = h.handle(&request(Op::Watch(WatchArgs {
        access: WatchAccess::Write,
        at: "$C000".into(),
        condition: None,
        once: true,
    })));
    assert!(made.is_ok(), "{:?}", made.error());
    let first = run_frames(&mut h, 2);
    let stop = first.outcome.unwrap().stop.unwrap();
    assert!(
        matches!(stop.reason, StopReason::Watchpoint { .. }),
        "{stop:?}"
    );

    let second = run_frames(&mut h, 2);
    let stop = second.outcome.unwrap().stop.unwrap();
    assert!(
        matches!(stop.reason, StopReason::BudgetExhausted { .. }),
        "the watch fired again: {stop:?}"
    );
    let meta = f.bundle.session("demo").unwrap().read_meta().unwrap();
    assert!(meta.debugger.watches.is_empty(), "{:?}", meta.debugger);
}

#[test]
fn a_poke_before_the_first_run_starts_from_the_boot_handoff() {
    let f = Fixture::new("poke-first");
    let mut h = handler(&f);
    let written = h.handle(&request(poke("$C002", 2)));
    assert!(written.is_ok(), "{:?}", written.error());
    let head = f.bundle.session("demo").unwrap().read_head(Rev(1)).unwrap();
    assert!(head.unwrap().header.boot.is_some(), "a power-on head");

    let stop = run_frames(&mut h, 1).outcome.unwrap().stop.unwrap();
    assert_eq!(stop.time.since, gbcli_protocol::TimeBase::Boot);
    assert_eq!(stop.time.frame, 1);
}

#[test]
fn a_checkpoint_before_the_first_run_is_the_boot_handoff() {
    let f = Fixture::new("checkpoint-first");
    let mut h = handler(&f);
    let made = h.handle(&request(Op::Checkpoint(CheckpointArgs {
        name: "start".into(),
    })));
    assert!(made.is_ok(), "{:?}", made.error());
    let cp = f
        .bundle
        .session("demo")
        .unwrap()
        .checkpoint("start")
        .unwrap();
    assert!(cp.header.boot.is_some(), "a power-on checkpoint");
}

#[test]
fn run_allow_rom_change_loads_the_head_over_a_rebuild() {
    let f = Fixture::with_sym("run-rebuild");
    assert!(run_frames(&mut handler(&f), 1).is_ok());
    f.rebuild(V2, V2_SYM);
    let refused = run_frames(&mut handler(&f), 1);
    assert_eq!(
        refused.outcome.err().map(|e| e.code),
        Some(ErrorCode::RomChanged)
    );

    let mut run = request(Op::Run(RunArgs {
        frames: Some(1),
        ..RunArgs::default()
    }));
    run.allow_rom_change = true;
    let forced = handler(&f).handle(&run);
    assert!(forced.is_ok(), "{:?}", forced.error());
    assert!(!forced.warnings.is_empty(), "a forced load warns");
}

#[test]
fn restore_over_a_rebuild_does_not_load_the_head_first() {
    let f = Fixture::with_sym("restore-rebuild");
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 1).is_ok());
    let made = h.handle(&request(Op::Checkpoint(CheckpointArgs {
        name: "a".into(),
    })));
    assert!(made.is_ok());
    f.rebuild(V2, V2_SYM);
    let mut restore = request(Op::Restore(RestoreArgs { name: "a".into() }));
    restore.allow_rom_change = true;
    let restored = handler(&f).handle(&restore);
    assert!(restored.is_ok(), "{:?}", restored.error());
    assert!(!restored.warnings.is_empty(), "a forced load warns");
}

#[test]
fn undo_over_a_compatible_rebuild_reports_the_load_warning() {
    let f = Fixture::with_sym("undo-rebuild");
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 1).is_ok());
    assert!(run_frames(&mut h, 1).is_ok());
    f.rebuild(V3, V3_SYM);
    let undone = handler(&f).handle(&request(Op::Undo));
    assert!(undone.is_ok(), "{:?}", undone.error());
    assert!(
        undone.warnings.iter().any(|w| w.contains("ROM")),
        "{:?}",
        undone.warnings
    );
}

#[test]
fn run_checkpoint_records_the_revision_it_saved() {
    let f = Fixture::new("run-checkpoint");
    let response = handler(&f).handle(&request(Op::Run(RunArgs {
        frames: Some(1),
        checkpoint: Some("after".into()),
        ..RunArgs::default()
    })));
    assert!(response.is_ok(), "{:?}", response.error());
    assert_eq!(response.rev, Some(Rev(1)));
    assert_eq!(response.outcome.unwrap().checkpoint.unwrap().rev, Rev(1));
}

#[test]
fn an_until_failure_saves_the_head_its_stop_record_describes() {
    let f = Fixture::new("until-fails");
    let mut h = handler(&f);
    // Short-circuited until wFrame reaches 60; then it divides by zero.
    let response = h.handle(&request(Op::Run(RunArgs {
        until: Some("[$C000] == 60 && 1 / ([$C000] - 60) == 0".into()),
        frames: Some(10),
        ..RunArgs::default()
    })));
    let error = response.outcome.expect_err("the predicate divides by zero");
    let stop = error.stop.expect("the failure carries its stop");
    assert_eq!(response.rev, Some(Rev(1)), "the head wasn't saved");
    let meta = f.bundle.session("demo").unwrap().read_meta().unwrap();
    assert_eq!(meta.last_stop.as_ref().map(|s| s.pc), Some(stop.pc));
    // A cold handler reads the saved head, not the warm machine.
    assert_eq!(read_one(&mut handler(&f), "pc"), i64::from(stop.pc));
}

#[test]
fn a_failed_run_whose_save_fails_reports_the_save_not_a_stop_it_lost() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new("until-save-fails");
    let history = f.bundle.session("demo").unwrap().dir().join("history");
    std::fs::create_dir_all(&history).unwrap();
    std::fs::set_permissions(&history, std::fs::Permissions::from_mode(0o555)).unwrap();
    let response = handler(&f).handle(&request(Op::Run(RunArgs {
        until: Some("[$C000] == 60 && 1 / ([$C000] - 60) == 0".into()),
        frames: Some(10),
        ..RunArgs::default()
    })));
    std::fs::set_permissions(&history, std::fs::Permissions::from_mode(0o755)).unwrap();
    let error = response.outcome.expect_err("the save failed");
    assert_eq!(error.code, ErrorCode::IoFailed, "{error:?}");
    assert!(error.stop.is_none(), "no head holds that stop");
    assert!(error.message.contains("until"), "{}", error.message);
    assert_eq!(response.rev, Some(Rev(0)));
}

#[test]
fn a_cold_handler_reads_the_head_not_power_on() {
    let f = Fixture::new("cold-read");
    assert!(run_frames(&mut handler(&f), 3).is_ok());
    assert_eq!(read_one(&mut handler(&f), "frame"), 3);
}

#[test]
fn step_records_its_actor_in_the_head() {
    let f = Fixture::new("step-actor");
    let response = handler(&f).handle(&request(Op::Step(StepArgs {
        unit: StepKind::Instruction,
    })));
    assert!(response.is_ok(), "{:?}", response.error());
    let head = f.bundle.session("demo").unwrap().read_head(Rev(1)).unwrap();
    assert_eq!(head.unwrap().header.created_by.as_deref(), Some("tester"));
}

#[test]
fn a_session_removed_and_recreated_at_the_same_rev_is_not_served_warm() {
    let f = Fixture::new("recreate");
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 1).is_ok());
    // Another process removes the session, creates it again, runs it to rev 1.
    let store = f.bundle.session("demo").unwrap();
    store.remove(true, Duration::from_secs(1)).unwrap();
    f.create_session(false);
    assert_eq!(run_frames(&mut handler(&f), 3).rev, Some(Rev(1)));
    assert_eq!(read_one(&mut h, "frame"), 3, "the warm machine was reused");
}

#[test]
fn rm_forgets_the_warm_machine() {
    let f = Fixture::new("rm-warm");
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 1).is_ok());
    let removed = h.handle(&request(Op::Rm(gbcli_protocol::RmArgs {
        name: "demo".into(),
        with_checkpoints: true,
    })));
    assert!(removed.is_ok(), "{:?}", removed.error());
    assert!(h.warm.is_none());
}

#[test]
fn a_bank_qualified_poke_writes_that_bank() {
    let f = Fixture::new("poke-bank");
    f.bundle
        .session("cgb")
        .unwrap()
        .create(&RomSource {
            rom: f.dir.join("demo.gb"),
            sym: None,
            model: Some(Model::CgbE),
            seed: Some(0),
        })
        .unwrap();
    let mut h = Handler::new(f.bundle.clone(), Some("cgb".into()), None);
    let written = h.handle(&request(poke("03:D000", 0x5A)));
    assert!(written.is_ok(), "{:?}", written.error());
    let machine = &h.warm.as_ref().expect("kept warm").machine;
    let wram = machine.core().region(Region::Wram).unwrap();
    assert_eq!(wram.bytes[3 * 0x1000], 0x5A, "bank 3");
    assert_ne!(wram.bytes[0x1000], 0x5A, "not the mapped bank 1");
}

#[test]
fn a_region_offset_past_this_models_size_is_refused() {
    let f = Fixture::new("poke-dmg-bounds");
    let response = handler(&f).handle(&request(poke("wram:0x2000", 1)));
    let error = response.outcome.expect_err("a DMG has 8 KB of WRAM");
    assert_eq!(error.code, ErrorCode::BadAddress);
}

#[test]
fn a_poke_to_rom_names_the_patch_kind() {
    let f = Fixture::new("poke-rom");
    let response = handler(&f).handle(&request(poke("$0150", 0)));
    let error = response.outcome.expect_err("ROM is patched, not poked");
    assert!(error.remedy.contains("patch"), "{}", error.remedy);
}
