use std::io;
use std::path::PathBuf;

use gbcli_core::{CoreError, Feature, Model};
use gbcli_engine::{
    CheckpointError, DebugError, LoadWarning, Moved, MovedRole, SetupError, SymbolAt, SymbolSide,
};
use gbcli_protocol::{CoreId, ErrorCode, Sha256};

use super::StoreError;

/// Asserts the code, its exit code (design §5) and the remedy together.
fn assert_maps(e: &StoreError, code: ErrorCode, exit: u8, remedy: &str) {
    assert_eq!(e.code(), code, "{e:?}");
    assert_eq!(e.code().exit_code(), exit, "{e:?}");
    assert_eq!(e.remedy(), remedy, "{e:?}");
}

fn core_id(commit: &str) -> CoreId {
    CoreId {
        backend: "sameboy".into(),
        commit: commit.into(),
    }
}

/// One of each, in the engine's step order. Each carries the code the engine
/// documents for it (crates/engine/src/checkpoint_error.rs) and design §6 states.
fn every_checkpoint_error() -> Vec<(CheckpointError, ErrorCode, u8)> {
    vec![
        (
            CheckpointError::Unreadable {
                reason: "bad magic".into(),
            },
            ErrorCode::CheckpointUnreadable,
            4,
        ),
        (
            CheckpointError::OtherBackend {
                saved: "mgba".into(),
                ours: "sameboy".into(),
            },
            ErrorCode::CheckpointMismatch,
            4,
        ),
        (
            CheckpointError::CoreRejected {
                saved: core_id("aaaa"),
                ours: core_id("bbbb"),
                reason: "another patch set".into(),
            },
            ErrorCode::CheckpointMismatch,
            4,
        ),
        (
            CheckpointError::ModelConflict {
                saved: Model::DmgB,
                requested: Model::Mgb,
            },
            ErrorCode::BadRequest,
            2,
        ),
        (
            CheckpointError::BootRomMissing {
                sha256: Sha256([0xab; 32]),
            },
            ErrorCode::BootRomMissing,
            5,
        ),
        (
            CheckpointError::SymbolsMissing {
                side: SymbolSide::Current,
            },
            ErrorCode::RomChanged,
            3,
        ),
        (
            CheckpointError::RomChanged {
                moved: vec![Moved {
                    name: "wSpeed".into(),
                    role: MovedRole::Ram,
                    old: SymbolAt {
                        bank: 0,
                        addr: 0xc000,
                    },
                    new: None,
                }],
            },
            ErrorCode::RomChanged,
            3,
        ),
        (
            CheckpointError::PatchConflict {
                offset: 0x150,
                expected: vec![0x00],
                found: vec![0x76],
            },
            ErrorCode::RomChanged,
            3,
        ),
        (
            CheckpointError::PatchRefused {
                offset: 0x150,
                reason: "out of range".into(),
            },
            ErrorCode::CheckpointMismatch,
            4,
        ),
        (
            CheckpointError::Unknown {
                name: "f30".into(),
                known: vec!["f10".into()],
            },
            ErrorCode::BadRequest,
            2,
        ),
    ]
}

#[test]
fn every_checkpoint_error_keeps_the_engines_code_exit_code_and_remedy() {
    for (inner, code, exit) in every_checkpoint_error() {
        let remedy = inner.remedy();
        assert_maps(&StoreError::Checkpoint(inner), code, exit, &remedy);
    }
}

#[test]
fn a_core_that_rejects_another_patch_sets_state_is_a_checkpoint_mismatch() {
    let inner = CheckpointError::CoreRejected {
        saved: core_id("1234+layer"),
        ours: core_id("1234+other"),
        reason: "the state is from another patch set".into(),
    };
    let remedy = inner.remedy();
    assert!(remedy.contains("1234+layer"), "{remedy}");
    assert_maps(
        &StoreError::Checkpoint(inner),
        ErrorCode::CheckpointMismatch,
        4,
        &remedy,
    );
}

#[test]
fn a_missing_checkpoint_name_is_a_bad_request_listing_the_names_that_exist() {
    let e = StoreError::CheckpointMissing {
        name: "f30".into(),
        known: vec!["f10".into(), "f20".into()],
    };
    assert_maps(&e, ErrorCode::BadRequest, 2, "known checkpoints: f10, f20");
}

#[test]
fn a_filesystem_failure_is_io_failed_not_refused() {
    let e = StoreError::Io {
        path: PathBuf::from("/x/.gbcli/sessions/demo/head.gbck"),
        source: io::Error::new(io::ErrorKind::StorageFull, "no space left on device"),
    };
    assert_maps(
        &e,
        ErrorCode::IoFailed,
        5,
        "check permissions on /x/.gbcli/sessions/demo/head.gbck and the free disk space",
    );
}

#[test]
fn a_bundle_root_that_cant_be_made_is_io_failed() {
    let e = StoreError::NoRoot {
        start: PathBuf::from("/ro"),
        source: io::Error::new(io::ErrorKind::PermissionDenied, "read-only"),
    };
    assert_maps(
        &e,
        ErrorCode::IoFailed,
        5,
        "create a writable project directory, or set GBCLI_DIR (tried /ro)",
    );
}

#[test]
fn an_unparseable_session_file_is_session_unreadable() {
    let e = StoreError::SessionUnreadable {
        name: "demo".into(),
        path: PathBuf::from("/x/.gbcli/sessions/demo/session.json"),
        reason: "expected value at line 1 column 1".into(),
    };
    assert_maps(
        &e,
        ErrorCode::SessionUnreadable,
        4,
        "fix /x/.gbcli/sessions/demo/session.json by hand, or remove the session with \
         `gbcli sessions rm demo` and create it again",
    );
    assert!(e.to_string().contains("session.json"), "{e}");
}

#[test]
fn creating_an_existing_session_is_a_bad_request_naming_another_way() {
    let e = StoreError::SessionExists {
        name: "demo".into(),
    };
    assert_maps(
        &e,
        ErrorCode::BadRequest,
        2,
        "use another --session name, or remove this one with `gbcli sessions rm demo`",
    );
    assert_eq!(e.to_string(), "session \"demo\" already exists");
}

#[test]
fn a_rom_other_than_the_sessions_is_a_bad_request_naming_three_ways_out() {
    let e = StoreError::RomMismatch {
        name: "demo".into(),
        stored: "build/demo.gb".into(),
        given: PathBuf::from("/work/other.gb"),
    };
    assert_maps(
        &e,
        ErrorCode::BadRequest,
        2,
        "omit --rom to open demo over build/demo.gb, use another --session name, or remove \
         it with `gbcli sessions rm demo` and create it again",
    );
    assert_eq!(
        e.to_string(),
        "session \"demo\" runs build/demo.gb, not /work/other.gb"
    );
}

#[test]
fn a_bad_session_name_says_a_name_cant_start_with_a_dash() {
    let e = StoreError::BadSessionName { name: "-x".into() };
    assert_maps(
        &e,
        ErrorCode::BadRequest,
        2,
        "names are [a-z0-9-], 1 to 40 characters, not starting with -",
    );
}

#[test]
fn a_boot_run_the_core_cant_do_keeps_the_unsupported_code() {
    let e = StoreError::BootFailed(DebugError::Core(CoreError::Unsupported {
        feature: Feature::DotStep,
        reason: "no",
    }));
    assert_maps(
        &e,
        ErrorCode::Unsupported,
        5,
        "pass --from power-on to run the boot ROM inside the request and see where it stops",
    );
}

#[test]
fn a_boot_run_that_fails_otherwise_keeps_the_runs_own_code() {
    // One mapping, the engine's (DebugError::code): a backend fault is
    // gbcli's, not a missing flag.
    let e = StoreError::BootFailed(DebugError::Core(CoreError::Backend {
        message: "boom".into(),
    }));
    assert_maps(
        &e,
        ErrorCode::Internal,
        5,
        "pass --from power-on to run the boot ROM inside the request and see where it stops",
    );
    assert!(e.to_string().contains("boom"), "{e}");
}

#[test]
fn a_core_that_refuses_a_rom_is_rom_unreadable() {
    let e = StoreError::Setup(SetupError::Core(CoreError::InvalidState {
        reason: "a ROM needs at least 0x150 bytes for its header; got 0x10".into(),
    }));
    assert_maps(
        &e,
        ErrorCode::RomUnreadable,
        4,
        "the core refused this ROM; check that the file is a Game Boy ROM with a full header",
    );
}

#[test]
fn a_rom_the_core_cant_run_is_unsupported() {
    let e = StoreError::Setup(SetupError::Core(CoreError::Unsupported {
        feature: Feature::DotStep,
        reason: "no mapper",
    }));
    assert_maps(
        &e,
        ErrorCode::Unsupported,
        5,
        "the core can't run this ROM (no mapper); `gbcli gaps` lists what it can't do that this \
         project has hit",
    );
}

/// A rebuild over the session's ROM file changes what the session runs; the
/// warning says so, by path, and how to keep a baseline instead (an agent
/// measured its "baseline" session on the optimized build without noticing).
#[test]
fn a_rebuilt_rom_warning_says_the_session_now_runs_the_file_and_how_to_keep_a_baseline() {
    let warning = LoadWarning::RomChanged {
        moved: Vec::new(),
        checked: 12,
        forced: false,
    };
    let text = StoreError::warning_text(&warning, "build/game.gb");
    for needle in [
        "build/game.gb",
        "now runs the rebuilt file",
        "no live symbol moved (12 checked)",
        "gbcli build --out",
    ] {
        assert!(text.contains(needle), "{needle} missing: {text}");
    }
}

#[test]
fn bad_arguments_are_bad_requests_with_their_own_remedies() {
    assert_maps(
        &StoreError::BadHistoryRef { text: "~x".into() },
        ErrorCode::BadRequest,
        2,
        "use ~N for the head N requests back, e.g. ~1",
    );
    assert_maps(
        &StoreError::BadCheckpointName {
            name: "../x".into(),
        },
        ErrorCode::BadRequest,
        2,
        "use letters, digits, -, _ and . (1 to 64 characters)",
    );
    assert_maps(
        &StoreError::BadDuration { text: "7".into() },
        ErrorCode::BadRequest,
        2,
        "use a number and a unit (s, m, h or d): 30m, 12h or 7d",
    );
}
