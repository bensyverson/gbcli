//! Checkpoints: the envelope, the seven verification steps, patch sets,
//! clones and the rebuilt-ROM check (design §6).

mod common;

use common::*;
use gbcli_core::{Budget, Model, Region, RunRequest};
use gbcli_engine::{
    Backend, Checkpoint, CheckpointError, LoadOptions, LoadWarning, Machine, MovedRole, SaveMeta,
    Session, sha256,
};
use gbcli_protocol::{BreakpointSpec, Location, Sha256};

fn meta() -> SaveMeta {
    SaveMeta {
        created_by: Some("test".into()),
        parent: None,
    }
}

fn run(m: &mut Machine, instructions: u64) {
    m.core_mut()
        .run(&mut RunRequest::new(Budget::instructions(instructions)))
        .expect("run");
}

/// A v1 machine stopped inside Work, called from Main.loop.
fn in_work() -> Machine {
    let mut m = machine(V1, Some(V1_SYM));
    step_to(&mut m, 0x0166); // Work+3: `add a, b`
    m
}

#[test]
fn save_mutate_restore_gives_a_byte_identical_blob_and_envelope() {
    let mut m = in_work();
    let saved = m.capture(meta());
    run(&mut m, 5_000);
    m.patch(0x0200, &[0xAA]).unwrap();
    m.core_mut().poke(Region::Wram, 2, &[0x55]).unwrap();
    assert_ne!(m.core().save(), saved.blob);

    m.restore(&saved, &LoadOptions::default()).unwrap();
    let again = m.capture(meta());
    assert_eq!(again.blob, saved.blob);
    assert_eq!(again.encode(), saved.encode());
}

#[test]
fn the_envelope_round_trips_and_starts_with_its_magic() {
    let saved = in_work().capture(meta());
    let bytes = saved.encode();
    assert_eq!(&bytes[..4], b"GBCK");
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 1);
    assert_eq!(Checkpoint::decode(&bytes).unwrap(), saved);
}

#[test]
fn a_bad_magic_unknown_version_or_truncation_is_unreadable() {
    let bytes = in_work().capture(meta()).encode();
    let mut bad = bytes.clone();
    bad[0] = b'X';
    let mut future = bytes.clone();
    future[4] = 9;
    for broken in [&bad[..], &future[..], &bytes[..10], &bytes[..20]] {
        let err = Checkpoint::decode(broken).unwrap_err();
        assert!(matches!(err, CheckpointError::Unreadable { .. }), "{err:?}");
        assert!(!err.remedy().is_empty());
    }
}

#[test]
fn another_backend_is_refused_with_a_remedy() {
    let mut saved = in_work().capture(meta());
    saved.header.core.backend = "bespoke".into();
    let err = in_work()
        .restore(&saved, &LoadOptions::default())
        .unwrap_err();
    assert!(
        matches!(err, CheckpointError::OtherBackend { .. }),
        "{err:?}"
    );
    assert!(err.remedy().contains("bespoke"));
}

#[test]
fn another_commit_of_the_same_backend_loads_with_a_warning() {
    let mut saved = in_work().capture(meta());
    saved.header.core.commit = "0000000".into();
    let warnings = in_work().restore(&saved, &LoadOptions::default()).unwrap();
    assert!(
        warnings
            .iter()
            .any(|w| matches!(w, LoadWarning::OtherCommit { saved, .. } if saved == "0000000"))
    );
}

#[test]
fn a_conflicting_model_is_refused_with_a_remedy() {
    let saved = in_work().capture(meta());
    let opts = LoadOptions {
        model: Some(Model::CgbE),
        ..LoadOptions::default()
    };
    let err = in_work().restore(&saved, &opts).unwrap_err();
    assert!(
        matches!(
            err,
            CheckpointError::ModelConflict {
                saved: Model::DmgB,
                requested: Model::CgbE
            }
        ),
        "{err:?}"
    );
    assert!(err.remedy().contains("dmg-b"), "{}", err.remedy());
}

#[test]
fn the_machine_is_rebuilt_from_the_envelopes_model() {
    let saved = in_work().capture(meta());
    let mut cgb = Machine::new(
        Backend::sameboy(),
        {
            let mut c = config(V1);
            c.model = Model::CgbE;
            c
        },
        Some(V1_SYM),
    )
    .unwrap();
    cgb.restore(&saved, &LoadOptions::default()).unwrap();
    assert_eq!(cgb.core().info().model, Model::DmgB);
    assert_eq!(cgb.core().save(), saved.blob);
}

#[test]
fn a_blob_the_core_rejects_is_a_mismatch_naming_both_commits() {
    let mut saved = in_work().capture(meta());
    saved.header.model = Model::CgbE;
    saved.header.boot_rom_sha256 = sha256(gbcli_sameboy::builtin_boot_rom(Model::CgbE));
    let err = in_work()
        .restore(&saved, &LoadOptions::default())
        .unwrap_err();
    assert!(
        matches!(err, CheckpointError::CoreRejected { .. }),
        "{err:?}"
    );
    assert!(!err.remedy().is_empty());
}

/// A checkpoint as the build before the layer-identity patch wrote it: the
/// core identity is the bare SameBoy commit, and the SameBoy backend's blob is
/// format 2, which has no patch-set field (the 8-byte length and its bytes
/// after the magic and version).
fn from_before_the_layer_patch(mut cp: Checkpoint) -> Checkpoint {
    let (commit, _) = cp.header.core.commit.split_once('+').expect("a patch set");
    cp.header.core.commit = commit.to_string();
    let n = u64::from_le_bytes(cp.blob[12..20].try_into().unwrap()) as usize;
    let mut blob = cp.blob[..8].to_vec();
    blob.extend_from_slice(&2u32.to_le_bytes());
    blob.extend_from_slice(&cp.blob[20 + n..]);
    cp.blob = blob;
    cp
}

#[test]
fn a_checkpoint_from_before_the_layer_patch_is_refused_with_a_remedy() {
    let saved = from_before_the_layer_patch(in_work().capture(meta()));
    let err = in_work()
        .restore(&saved, &LoadOptions::default())
        .unwrap_err();
    let CheckpointError::CoreRejected { saved: s, ours, .. } = &err else {
        panic!("{err:?}");
    };
    assert_ne!(s.commit, ours.commit);
    let text = err.to_string();
    assert!(text.contains("layer-identity patch"), "{text}");
    assert!(
        err.remedy().contains("re-create the state"),
        "{}",
        err.remedy()
    );
}

#[test]
fn an_unknown_boot_rom_is_refused_naming_its_hash() {
    let mut saved = in_work().capture(meta());
    saved.header.boot_rom_sha256 = Sha256([7; 32]);
    let err = in_work()
        .restore(&saved, &LoadOptions::default())
        .unwrap_err();
    assert!(
        matches!(err, CheckpointError::BootRomMissing { sha256 } if sha256 == Sha256([7; 32])),
        "{err:?}"
    );
    assert!(err.remedy().contains(&"07".repeat(32)));
}

#[test]
fn a_boot_rom_passed_in_the_options_is_found_by_hash() {
    let mut m = Machine::new(
        Backend::sameboy(),
        {
            let mut c = config(V1);
            c.boot_rom = gbcli_core::BootRom::BuiltIn;
            c
        },
        Some(V1_SYM),
    )
    .unwrap();
    let saved = in_work().capture(meta());
    let err = m.restore(&saved, &LoadOptions::default()).unwrap_err();
    assert!(matches!(err, CheckpointError::BootRomMissing { .. }));
    let opts = LoadOptions {
        boot_roms: vec![QUICK_BOOT.to_vec()],
        ..LoadOptions::default()
    };
    m.restore(&saved, &opts).unwrap();
}

#[test]
fn a_changed_rom_without_symbols_is_refused() {
    let mut saved_without = machine(V1, None);
    step_to(&mut saved_without, 0x0166);
    let saved = saved_without.capture(meta());
    let mut rebuilt = machine(V3, Some(V3_SYM));
    let err = rebuilt
        .restore(&saved, &LoadOptions::default())
        .unwrap_err();
    assert!(
        matches!(err, CheckpointError::SymbolsMissing { .. }),
        "{err:?}"
    );
    assert!(err.remedy().contains("-n") && err.remedy().contains("--allow-rom-change"));

    let saved = in_work().capture(meta());
    let err = machine(V3, None)
        .restore(&saved, &LoadOptions::default())
        .unwrap_err();
    assert!(matches!(err, CheckpointError::SymbolsMissing { .. }));
}

#[test]
fn a_rebuilt_rom_whose_symbols_moved_under_pc_is_refused_naming_them() {
    let saved = in_work().capture(meta());
    let mut rebuilt = machine(V2, Some(V2_SYM));
    let err = rebuilt
        .restore(&saved, &LoadOptions::default())
        .unwrap_err();
    let CheckpointError::RomChanged { moved } = &err else {
        panic!("{err:?}")
    };
    let pc = moved.iter().find(|m| m.role == MovedRole::Pc).unwrap();
    assert_eq!(pc.name, "Work");
    assert_eq!(
        (pc.old.addr, pc.new.map(|n| n.addr)),
        (0x0163, Some(0x0165))
    );
    assert!(
        moved
            .iter()
            .any(|m| m.role == MovedRole::Stack && m.name == "Main.loop"),
        "the return address into Main.loop: {moved:?}"
    );
    assert!(err.to_string().contains("Work"), "{err}");
    assert!(err.remedy().contains("--allow-rom-change"));
}

#[test]
fn a_rebuilt_rom_whose_ram_moved_is_refused() {
    let saved = in_work().capture(meta());
    let err = machine(V4, Some(V4_SYM))
        .restore(&saved, &LoadOptions::default())
        .unwrap_err();
    let CheckpointError::RomChanged { moved } = &err else {
        panic!("{err:?}")
    };
    assert!(
        moved
            .iter()
            .any(|m| m.role == MovedRole::Ram && m.name == "wFrame")
    );
}

#[test]
fn a_moved_breakpoint_symbol_is_refused() {
    let mut m = machine(V1, Some(V1_SYM));
    step_to(&mut m, 0x0150); // Main: nothing live has moved in v2
    let saved = m.capture(meta());
    let mut rebuilt = machine(V2, Some(V2_SYM));
    rebuilt
        .debugger_config_mut()
        .breakpoints
        .push(BreakpointSpec {
            id: 1,
            at: Location {
                addr: 0x0163,
                bank: Some(0),
                symbol: Some("Work".into()),
            },
            condition: None,
        });
    let err = rebuilt
        .restore(&saved, &LoadOptions::default())
        .unwrap_err();
    let CheckpointError::RomChanged { moved } = &err else {
        panic!("{err:?}")
    };
    assert_eq!(moved.len(), 1, "{moved:?}");
    assert_eq!(
        (moved[0].role, moved[0].name.as_str()),
        (MovedRole::Breakpoint, "Work")
    );
}

#[test]
fn a_breakpoint_at_a_symbol_plus_or_minus_an_offset_that_did_not_move_loads() {
    let mut m = machine(V1, Some(V1_SYM));
    step_to(&mut m, 0x0150);
    let saved = m.capture(meta());
    let mut rebuilt = machine(V3, Some(V3_SYM));
    for (id, text) in [(1, "Work-1"), (2, "Work+$10"), (3, "Main.loop+2")] {
        let at = gbcli_engine::addr::parse(text, rebuilt.symbols()).unwrap();
        rebuilt
            .debugger_config_mut()
            .breakpoints
            .push(BreakpointSpec {
                id,
                at,
                condition: None,
            });
    }
    let warnings = rebuilt.restore(&saved, &LoadOptions::default()).unwrap();
    assert!(
        warnings
            .iter()
            .any(|w| matches!(w, LoadWarning::RomChanged { moved, .. } if moved.is_empty())),
        "{warnings:?}"
    );
}

#[test]
fn a_rebuilt_rom_with_nothing_live_moved_loads_with_a_warning() {
    let saved = in_work().capture(meta());
    let mut rebuilt = machine(V3, Some(V3_SYM));
    let warnings = rebuilt.restore(&saved, &LoadOptions::default()).unwrap();
    assert!(
        warnings
            .iter()
            .any(|w| matches!(w, LoadWarning::RomChanged { moved, .. } if moved.is_empty())),
        "{warnings:?}"
    );
    assert_eq!(rebuilt.core().registers().pc, 0x0166);
}

#[test]
fn allow_rom_change_loads_anyway_and_lists_what_moved() {
    let saved = in_work().capture(meta());
    let opts = LoadOptions {
        allow_rom_change: true,
        ..LoadOptions::default()
    };
    let warnings = machine(V2, Some(V2_SYM)).restore(&saved, &opts).unwrap();
    assert!(
        warnings
            .iter()
            .any(|w| matches!(w, LoadWarning::RomChanged { moved, .. } if !moved.is_empty()))
    );
    let warnings = machine(V2, None).restore(&saved, &opts).unwrap();
    assert!(!warnings.is_empty());
}

#[test]
fn restoring_restores_exactly_the_checkpoints_patch_set() {
    let mut m = in_work();
    let original = m.core().region(Region::Rom).unwrap().bytes[..0x8000].to_vec();
    let bare = m.capture(meta());
    m.patch(0x0200, &[0x11, 0x22]).unwrap();
    let one = m.capture(meta());
    m.patch(0x0300, &[0x33]).unwrap();
    m.patch(0x0200, &[0x44]).unwrap();
    assert_eq!(m.patches().len(), 3);

    m.restore(&one, &LoadOptions::default()).unwrap();
    let rom = m.core().region(Region::Rom).unwrap().bytes;
    assert_eq!(&rom[0x0200..0x0202], &[0x11, 0x22]);
    assert_eq!(rom[0x0300], original[0x0300], "a later patch is removed");
    assert_eq!(m.patches(), one.header.patches.as_slice());
    assert_eq!(one.header.patches[0].before, original[0x0200..0x0202]);

    m.restore(&bare, &LoadOptions::default()).unwrap();
    assert_eq!(
        &m.core().region(Region::Rom).unwrap().bytes[..0x8000],
        &original[..]
    );
    assert!(m.patches().is_empty());
}

#[test]
fn a_patch_to_the_header_is_refused_and_not_recorded() {
    let mut m = in_work();
    assert!(m.patch(0x0134, &[0]).is_err());
    assert!(m.patches().is_empty());
}

#[test]
fn a_clone_runs_identically_and_independently() {
    let mut a = in_work();
    a.patch(0x0300, &[0x99]).unwrap();
    let mut b = a.try_clone().unwrap();
    assert_eq!(a.core().save(), b.core().save());
    assert_eq!(a.patches(), b.patches());
    run(&mut a, 3_000);
    run(&mut b, 3_000);
    assert_eq!(a.core().save(), b.core().save());
    b.core_mut().poke(Region::Wram, 2, &[7]).unwrap();
    run(&mut b, 10);
    run(&mut a, 10);
    assert_ne!(a.core().save(), b.core().save());
}

#[test]
fn a_session_keeps_named_checkpoints_and_clones() {
    let mut session = Session::new();
    let id = session.add(in_work());
    session.save(id, "f30", meta());
    let exported = session.checkpoint("f30").unwrap().encode();
    run(session.machine_mut(id), 2_000);
    session.restore(id, "f30", &LoadOptions::default()).unwrap();
    assert_eq!(
        session.machine(id).core().save(),
        session.checkpoint("f30").unwrap().blob
    );
    let b = session.clone_machine(id).unwrap();
    assert_ne!(id, b);
    session.import("copy", &exported).unwrap();
    assert_eq!(session.checkpoint_names(), vec!["copy", "f30"]);
    let err = session
        .restore(id, "nope", &LoadOptions::default())
        .unwrap_err();
    assert!(err.remedy().contains("f30"), "{}", err.remedy());
}
