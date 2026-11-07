//! Regression tests for the re-review's server findings (leaf ZbyotP).

use std::fs;
use std::os::unix::fs::PermissionsExt;

use gbcli_bundle::StoreError;
use gbcli_core::{CoreError, Feature};
use gbcli_engine::{DebugError, SetupError};
use gbcli_protocol::{ErrorCode, Op, ReadArgs, Rev};

use super::Failure;
use super::test_support::{handler, request, run_frames};
use crate::fixture::Fixture;

#[test]
fn read_allow_uninit_reads_a_never_written_byte_with_a_warning() {
    let f = Fixture::new("read-allow-uninit-op");
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 1).is_ok());
    let read = |h: &mut super::Handler, allow_uninit| {
        h.handle(&request(Op::Read(ReadArgs {
            reads: vec!["[$C003]".into()],
            since: None,
            allow_uninit,
        })))
    };
    let refused = read(&mut h, false);
    let error = refused.error().expect("an uninitialized read fails");
    assert_eq!(error.code, ErrorCode::UninitializedRead);
    let allowed = read(&mut h, true);
    assert!(allowed.is_ok(), "{:?}", allowed.error());
    assert!(
        allowed.warnings.iter().any(|w| w.contains("$C003")),
        "{:?}",
        allowed.warnings
    );
}

#[test]
fn undo_past_the_oldest_names_the_head_and_the_oldest_entry() {
    let f = Fixture::new("undo-remedy");
    let mut h = handler(&f);
    for _ in 0..4 {
        assert!(run_frames(&mut h, 1).is_ok());
    }
    for _ in 0..3 {
        assert!(h.handle(&request(Op::Undo)).is_ok());
    }
    // The head is s7, a copy of s1; s1 is the oldest entry kept.
    let response = h.handle(&request(Op::Undo));
    let error = response.error().expect("nothing before s1");
    assert!(error.remedy.contains("s7"), "{}", error.remedy);
    assert!(error.remedy.contains("s1"), "{}", error.remedy);
    assert!(!error.remedy.contains("heads back"), "{}", error.remedy);
}

#[test]
fn a_head_whose_session_file_cant_be_written_does_not_move_the_rev() {
    let f = Fixture::new("save-meta-fails");
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 1).is_ok());
    let dir = f.bundle.session("demo").unwrap().dir();
    let mode = |m| fs::set_permissions(&dir, fs::Permissions::from_mode(m)).unwrap();
    // history/ stays writable, so only the session.json write fails.
    mode(0o555);
    let response = run_frames(&mut h, 1);
    mode(0o755);
    assert_eq!(
        response.error().map(|e| e.code),
        Some(ErrorCode::IoFailed),
        "{response:?}"
    );
    assert_eq!(response.rev, Some(Rev(1)), "the response claims a save");
    let meta = f.bundle.session("demo").unwrap().read_meta().unwrap();
    assert_eq!(meta.rev, Rev(1));
}

#[test]
fn an_unsupported_feature_behind_a_store_error_reaches_the_gap_ledger() {
    let unsupported = || CoreError::Unsupported {
        feature: Feature::DotStep,
        reason: "test",
    };
    let boot = StoreError::BootFailed(DebugError::Core(unsupported()));
    let setup = StoreError::Setup(SetupError::Core(unsupported()));
    for error in [boot, setup] {
        let failure = Failure::from_store(&error);
        assert_eq!(failure.body.code, ErrorCode::Unsupported, "{error:?}");
        assert!(failure.gap.is_some(), "{error:?} has no gap entry");
    }
}
