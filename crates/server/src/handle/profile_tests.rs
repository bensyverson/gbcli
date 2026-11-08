//! `run --profile` and `run --frame-budget` through the handler (design §11):
//! collected during the run itself, so breakpoints and `--to` still stop it.

use gbcli_protocol::{
    ErrorCode, FrameBudgetMade, Op, ProfileArgs, Response, RunArgs, StepKind, StopReason,
};

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
    assert!(
        made.frames
            .iter()
            .all(|r| r.lag == Some(true) && r.idle_t == 0)
    );
    assert_eq!(made.lag, 3);
    // A whole frame is 70,224 T; the first counts from the boot handoff.
    assert!(
        made.frames[1].active_t.abs_diff(70_224) < 32,
        "{:?}",
        made.frames
    );
}

/// A run of `instructions` with `--frame-budget`: its rows, and the T-cycles
/// the run took by its stop record.
fn budget_of_instructions(name: &str, instructions: u64) -> (FrameBudgetMade, u64) {
    let f = Fixture::with_sym(name);
    let result = run(
        &mut handler(&f),
        RunArgs {
            instructions: Some(instructions),
            frame_budget: true,
            ..RunArgs::default()
        },
    )
    .outcome
    .expect("ran");
    let ticks = result.stop.expect("a stop").budget.ticks;
    (result.frame_budget.expect("a frame budget"), ticks / 2)
}

#[test]
fn a_run_inside_one_frame_reports_it_as_partial() {
    let (made, run_t) = budget_of_instructions("frame-budget-inside", 5_000);
    assert_eq!(made.frames.len(), 1, "{:?}", made.frames);
    let row = made.frames[0];
    assert!(row.partial);
    assert_eq!(
        row.lag, None,
        "lag isn't defined for a frame that didn't end"
    );
    assert_eq!(row.active_t + row.idle_t, run_t);
    assert_eq!(made.lag, 0);
}

#[test]
fn a_run_stopped_mid_frame_ends_with_a_partial_row_so_rows_cover_the_run() {
    let (made, run_t) = budget_of_instructions("frame-budget-mid", 30_000);
    let (last, whole) = made.frames.split_last().expect("rows");
    assert!(last.partial && last.lag.is_none(), "{last:?}");
    assert!(whole.iter().all(|r| !r.partial && r.lag == Some(true)));
    let sum: u64 = made.frames.iter().map(|r| r.active_t + r.idle_t).sum();
    assert_eq!(sum, run_t);
    assert_eq!(made.lag, whole.len() as u64);
}

#[test]
fn a_run_ending_at_a_vblank_has_no_partial_row() {
    let f = Fixture::with_sym("frame-budget-whole");
    let made = run(
        &mut handler(&f),
        RunArgs {
            frames: Some(2),
            frame_budget: true,
            ..RunArgs::default()
        },
    )
    .outcome
    .expect("ran")
    .frame_budget
    .expect("a frame budget");
    assert_eq!(made.frames.len(), 2);
    assert!(made.frames.iter().all(|r| !r.partial && r.lag.is_some()));
}

/// `--frame-budget` on a run that starts after `first` (none: a fresh
/// session, at the boot handoff), for `frames` frames.
fn budget_after(name: &str, first: Option<RunArgs>, frames: u64) -> FrameBudgetMade {
    let f = Fixture::with_sym(name);
    let mut h = handler(&f);
    if let Some(first) = first {
        run(&mut h, first).outcome.expect("the first run");
    }
    run(
        &mut h,
        RunArgs {
            frames: Some(frames),
            frame_budget: true,
            ..RunArgs::default()
        },
    )
    .outcome
    .expect("ran")
    .frame_budget
    .expect("a frame budget")
}

/// A run that starts mid-frame (after --instructions, a breakpoint or
/// --until) only sees the end of its first frame: that row is partial and
/// stays out of the summary and the lag count, as the last one does (leaf
/// Fjy1RU, from review probe re13: its 7,840 T was the summary's min).
#[test]
fn a_run_started_mid_frame_begins_with_a_partial_row_left_out_of_the_summary() {
    let mid = RunArgs {
        instructions: Some(5_000),
        ..RunArgs::default()
    };
    let made = budget_after("frame-budget-starts-mid", Some(mid), 3);
    assert_eq!(made.frames.len(), 3, "{:?}", made.frames);
    let (first, whole) = made.frames.split_first().expect("rows");
    assert!(first.partial && first.lag.is_none(), "{first:?}");
    assert!(whole.iter().all(|r| !r.partial && r.lag == Some(true)));
    assert_eq!(made.lag, 2);
    let stats = made.active_t.expect("a summary");
    // V1 never halts, so every whole frame is about 70,224 T active.
    assert!(
        stats.min.abs_diff(70_224) < 32,
        "{stats:?} {:?}",
        made.frames
    );
    assert_ne!(made.busiest, Some(0), "the busiest is a whole frame");
}

/// A run that starts at a vblank stop, or at the boot handoff, where the
/// game's first frame begins, counts its first frame whole.
#[test]
fn a_run_started_at_a_vblank_or_the_handoff_counts_its_first_frame_whole() {
    let at_vblank = RunArgs {
        frames: Some(2),
        ..RunArgs::default()
    };
    for (name, first) in [
        ("frame-budget-starts-vblank", Some(at_vblank)),
        ("frame-budget-starts-handoff", None),
    ] {
        let made = budget_after(name, first, 3);
        assert_eq!(made.frames.len(), 3, "{name}: {:?}", made.frames);
        assert!(
            made.frames
                .iter()
                .all(|r| !r.partial && r.lag == Some(true)),
            "{name}: {:?}",
            made.frames
        );
        assert_eq!(made.lag, 3, "{name}");
    }
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
