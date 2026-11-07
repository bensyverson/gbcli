//! `run --profile` and `run --frame-budget` through the handler (design §11):
//! collected during the run itself, so breakpoints and `--to` still stop it.

use gbcli_protocol::{ErrorCode, Op, ProfileArgs, Response, RunArgs, StepKind, StopReason};

use super::Handler;
use super::test_support::{handler, request};
use crate::fixture::Fixture;

fn run(h: &mut Handler, args: RunArgs) -> Response {
    h.handle(&request(Op::Run(args)))
}

#[test]
fn run_profile_ranks_routines_by_exclusive_cycles_and_top_trims_them() {
    let f = Fixture::with_sym("profile-all");
    let all = run(
        &mut handler(&f),
        RunArgs {
            frames: Some(3),
            profile: Some(ProfileArgs::default()),
            ..RunArgs::default()
        },
    )
    .outcome
    .expect("ran")
    .profile
    .expect("a profile");
    assert!(all.routines.len() >= 2, "{:?}", all.routines);
    let excl: Vec<u64> = all.routines.iter().map(|r| r.exclusive_t).collect();
    assert!(excl.windows(2).all(|w| w[0] >= w[1]), "{excl:?}");
    assert_eq!(excl.iter().sum::<u64>(), all.total_t);
    // Work's body (76 T) and its ret (16) outweigh the loop's call and jr.
    assert_eq!(all.routines[0].name.as_deref(), Some("Work"));
    assert_eq!(all.omitted, 0);

    let g = Fixture::with_sym("profile-top");
    let top = run(
        &mut handler(&g),
        RunArgs {
            frames: Some(3),
            profile: Some(ProfileArgs { top: Some(1) }),
            ..RunArgs::default()
        },
    )
    .outcome
    .expect("ran")
    .profile
    .expect("a profile");
    assert_eq!(top.routines, all.routines[..1]);
    assert_eq!(top.omitted, all.routines.len() as u64 - 1);
    assert_eq!(top.total_t, all.total_t);
}

#[test]
fn run_profile_stops_where_the_run_stops() {
    let f = Fixture::with_sym("profile-to");
    let result = run(
        &mut handler(&f),
        RunArgs {
            to: Some("Work".into()),
            profile: Some(ProfileArgs::default()),
            ..RunArgs::default()
        },
    )
    .outcome
    .expect("ran");
    let stop = result.stop.expect("a stop");
    assert_eq!(
        stop.reason,
        StopReason::StepDone {
            unit: StepKind::ToAddress
        }
    );
    let profile = result.profile.expect("a profile");
    assert!(profile.total_t > 0);
    assert!(
        profile.total_t < 70_224,
        "stopped well inside the first frame"
    );
}

#[test]
fn run_frame_budget_gives_a_row_per_frame_and_counts_lag() {
    let f = Fixture::with_sym("frame-budget");
    let made = run(
        &mut handler(&f),
        RunArgs {
            frames: Some(3),
            frame_budget: true,
            ..RunArgs::default()
        },
    )
    .outcome
    .expect("ran")
    .frame_budget
    .expect("a frame budget");
    assert_eq!(made.frames.len(), 3);
    // V1 never halts: every frame is busy for all of it.
    assert!(made.frames.iter().all(|r| r.lag && r.idle_t == 0));
    assert_eq!(made.lag, 3);
    // A whole frame is 70,224 T; the first counts from the boot handoff.
    assert!(
        made.frames[1].active_t.abs_diff(70_224) < 32,
        "{:?}",
        made.frames
    );
}

#[test]
fn profile_top_zero_is_refused() {
    let f = Fixture::with_sym("profile-zero");
    let error = run(
        &mut handler(&f),
        RunArgs {
            frames: Some(1),
            profile: Some(ProfileArgs { top: Some(0) }),
            ..RunArgs::default()
        },
    )
    .outcome
    .expect_err("refused");
    assert_eq!(error.code, ErrorCode::BadRequest);
}
