use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use gbcli_bundle::{SessionStore, Start};
use gbcli_core::Budget;
use gbcli_engine::{DebugError, Machine, RunOptions, addr};
use gbcli_protocol::{
    BudgetLimits, ErrorBody, ErrorCode, ResultBody, RunArgs, SessionFile, StepArgs, StepKind,
    StopReason, StopRecord,
};

use super::history::write_checkpoint;
use super::reads::{read_values, warning_text};
use super::writes::apply_writes;
use super::{Failure, Handler, Need, save_head};
use crate::convert::{self};
/// The wall-clock backstop on every run (design §3).
const WALL_GUARD: Duration = Duration::from_secs(60);
/// The default budget for `run` without one (design Q4).
const DEFAULT_FRAMES: u64 = 600;

impl Handler {
    pub(super) fn run(
        &mut self,
        store: &SessionStore,
        meta: &mut SessionFile,
        args: &RunArgs,
        actor: &Option<String>,
        warnings: &mut Vec<String>,
    ) -> Result<ResultBody, Failure> {
        let since = self.since(store, args.since.as_deref())?;
        let from = match args.from.as_deref() {
            None | Some("boot") => Start::Boot,
            Some("power-on") => Start::PowerOn,
            Some(other) => {
                return Err(Failure::new(ErrorBody::new(
                    ErrorCode::BadRequest,
                    format!("--from {other:?} isn't a start state"),
                    "use `--from power-on`, or omit it",
                )));
            }
        };
        let (mut machine, loaded) =
            self.machine_for(store, meta, Need::Run, args.at.as_deref(), from)?;
        warnings.extend(loaded);

        let writes = apply_writes(&mut machine, &args.writes)?;
        let (budget, limits, defaulted) = budget_of(args);
        let explicit = args.frames.is_some() || args.instructions.is_some() || args.ticks.is_some();

        if args.until.is_some() && args.to.is_some() {
            return Err(Failure::new(ErrorBody::new(
                ErrorCode::BadRequest,
                "a run stops at --until or --to, not both",
                "drop one of them",
            )));
        }

        let flag = Arc::new(AtomicBool::new(false));
        watch_wall(flag.clone());
        let mut opts = RunOptions::new(budget);
        opts.since = since.clone();
        opts.allow_uninit = args.allow_uninit;
        opts.interrupt = Some(&flag);
        let outcome = match (&args.until, &args.to) {
            (Some(predicate), _) => machine.run_until(predicate, &opts),
            (None, Some(text)) => {
                let at = addr::parse(text, machine.symbols())
                    .map_err(|e| Failure::new(convert::addr(&e)))?;
                machine.run_to(at, &opts)
            }
            (None, None) => machine.run(&opts),
        };
        let guarded = flag.load(Ordering::Relaxed);
        let mut outcome = match outcome {
            Ok(outcome) => outcome,
            Err(error) => {
                return Err(self.after_run_error(store, meta, machine, &error, actor, args));
            }
        };
        if guarded {
            outcome.stop.reason = StopReason::WallGuard;
        }
        outcome.stop.budget.limits = limits;
        outcome.stop.budget.defaulted = defaulted;
        warnings.extend(outcome.warnings.iter().map(warning_text));

        let reads = read_values(&machine, &args.read, &since, warnings)?;
        let checkpoint = match &args.checkpoint {
            Some(name) => Some(write_checkpoint(store, &machine, name, actor, meta.rev)?),
            None => None,
        };
        let stop = outcome.stop.clone();
        save_head(
            store,
            meta,
            &machine,
            actor.clone(),
            args.at.clone(),
            Some(&stop),
        )?;
        self.keep_warm(store, meta, machine);

        if guarded {
            return Err(Failure::new(
                ErrorBody::new(
                    ErrorCode::BudgetExhausted,
                    "the 60 s wall guard ended the run; this stop isn't reproducible",
                    "narrow the experiment, or give it a structure that ends",
                )
                .with_stop(stop),
            ));
        }
        let missed = match &stop.reason {
            StopReason::BudgetExhausted { .. } => !explicit,
            StopReason::WallGuard => true,
            _ => false,
        };
        if missed {
            let what = match (&args.until, &args.to) {
                (Some(until), _) => format!("`{until}` never held"),
                (None, Some(to)) => format!("PC never reached {to}"),
                _ => "nothing stopped it".to_string(),
            };
            let body = ErrorBody::new(
                ErrorCode::BudgetExhausted,
                format!(
                    "the budget ended the run at frame {}; {what}",
                    stop.time.frame
                ),
                match (&args.until, &args.to) {
                    (Some(until), _) => format!("raise --frames, or check the predicate: {until}"),
                    (None, Some(to)) => format!("raise --frames, or check the address: {to}"),
                    _ => "pass --frames, --ticks or --instructions to bound it".to_string(),
                },
            )
            .with_stop(stop);
            return Err(Failure::new(body));
        }
        Ok(ResultBody {
            stop: Some(stop),
            reads: Some(reads),
            checkpoint,
            writes: (!writes.is_empty()).then_some(writes),
            ..ResultBody::default()
        })
    }

    /// A run that failed: save the state it reached, and keep the stop record.
    fn after_run_error(
        &mut self,
        store: &SessionStore,
        meta: &mut SessionFile,
        machine: Machine,
        error: &DebugError,
        actor: &Option<String>,
        args: &RunArgs,
    ) -> Failure {
        let stop = stop_of(error).cloned();
        if stop.is_some() {
            let _ = save_head(
                store,
                meta,
                &machine,
                actor.clone(),
                args.at.clone(),
                stop.as_ref(),
            );
        }
        let mut failure = Failure::from_debug(error);
        if let Some(stop) = stop {
            failure.body = Box::new((*failure.body).with_stop(stop));
        }
        failure
    }

    pub(super) fn step(
        &mut self,
        store: &SessionStore,
        meta: &mut SessionFile,
        args: &StepArgs,
        warnings: &mut Vec<String>,
    ) -> Result<ResultBody, Failure> {
        if args.unit == StepKind::ToAddress {
            return Err(Failure::new(ErrorBody::new(
                ErrorCode::BadRequest,
                "step doesn't take an address",
                "use `run --to <address>`",
            )));
        }
        let (mut machine, _) = self.machine_for(store, meta, Need::Run, None, Start::Boot)?;
        let budget = match args.unit {
            StepKind::Frame => Budget::frames(1),
            StepKind::Instruction => Budget::instructions(1),
            StepKind::Over | StepKind::Out => Budget::frames(DEFAULT_FRAMES),
            StepKind::Scanline | StepKind::ToAddress => Budget::instructions(100_000),
        };
        let flag = Arc::new(AtomicBool::new(false));
        watch_wall(flag.clone());
        let mut opts = RunOptions::new(budget);
        opts.interrupt = Some(&flag);
        let outcome = machine
            .step(args.unit, &opts)
            .map_err(|e| Failure::from_debug(&e))?;
        let _ = flag;
        warnings.extend(outcome.warnings.iter().map(warning_text));
        let stop = outcome.stop;
        save_head(store, meta, &machine, None, None, Some(&stop))?;
        self.keep_warm(store, meta, machine);
        Ok(ResultBody {
            stop: Some(stop),
            ..ResultBody::default()
        })
    }
}

fn stop_of(error: &DebugError) -> Option<&StopRecord> {
    match error {
        DebugError::Eval { stop, .. } => Some(stop),
        _ => None,
    }
}

fn watch_wall(flag: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        std::thread::sleep(WALL_GUARD);
        flag.store(true, Ordering::Relaxed);
    });
}

/// The budget a run gets, and whether the server supplied the default.
fn budget_of(args: &RunArgs) -> (Budget, BudgetLimits, bool) {
    if args.frames.is_none() && args.instructions.is_none() && args.ticks.is_none() {
        let limits = BudgetLimits {
            frames: Some(DEFAULT_FRAMES),
            ..BudgetLimits::default()
        };
        return (Budget::frames(DEFAULT_FRAMES), limits, true);
    }
    let limits = BudgetLimits {
        ticks: args.ticks,
        instructions: args.instructions,
        frames: args.frames,
    };
    let budget = Budget::frames(args.frames.unwrap_or(u64::MAX))
        .with_instructions(args.instructions.unwrap_or(u64::MAX))
        .with_ticks(args.ticks.unwrap_or(u64::MAX));
    (budget, limits, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::Fixture;
    use crate::handle::test_support::{handler, request, run_frames};
    use gbcli_core::BudgetKind;
    use gbcli_protocol::{Op, Rev};

    #[test]
    fn an_explicit_budget_runs_and_says_it_was_not_defaulted() {
        let f = Fixture::new("explicit-budget");
        let response = run_frames(&mut handler(&f), 3);
        assert!(response.ok, "{:?}", response.error);
        assert_eq!(response.rev, Some(Rev(1)));
        let stop = response.result.unwrap().stop.unwrap();
        assert_eq!(
            stop.reason,
            StopReason::BudgetExhausted {
                kind: BudgetKind::Frames
            }
        );
        assert_eq!(stop.budget.frames, 3);
        assert_eq!(stop.budget.limits.frames, Some(3));
        assert!(!stop.budget.defaulted);
    }

    #[test]
    fn a_run_without_a_budget_gets_the_default_and_the_record_says_so() {
        let f = Fixture::new("default-budget");
        let response = handler(&f).handle(&request(Op::Run(RunArgs::default())));
        let error = response.error.expect("a bare run ran out of budget");
        assert_eq!(error.code, ErrorCode::BudgetExhausted);
        let stop = error.stop.expect("the stop record rides on the error");
        assert!(stop.budget.defaulted);
        assert_eq!(stop.budget.limits.frames, Some(DEFAULT_FRAMES));
        assert_eq!(stop.budget.frames, DEFAULT_FRAMES);
    }

    #[test]
    fn from_power_on_skips_the_head_and_the_boot_cache() {
        let f = Fixture::new("power-on");
        let mut h = handler(&f);
        assert!(run_frames(&mut h, 2).ok);
        let response = h.handle(&request(Op::Run(RunArgs {
            from: Some("power-on".into()),
            frames: Some(1),
            ..RunArgs::default()
        })));
        let stop = response.result.unwrap().stop.unwrap();
        assert_eq!(stop.time.since, gbcli_protocol::TimeBase::PowerOn);
        assert_eq!(
            stop.time.frame, 1,
            "one vblank since power-on, not from boot"
        );
    }

    #[test]
    fn until_stops_where_the_predicate_holds_and_names_its_cadence() {
        let f = Fixture::new("until");
        let mut h = handler(&f);
        assert!(run_frames(&mut h, 2).ok);
        let response = h.handle(&request(Op::Run(RunArgs {
            until: Some("u16[$C000] >= 2000".into()),
            frames: Some(600),
            ..RunArgs::default()
        })));
        let stop = response.result.unwrap().stop.unwrap();
        assert!(
            matches!(stop.reason, StopReason::Predicate { ref cadence, .. } if *cadence == gbcli_protocol::Cadence::Writes),
            "{:?}",
            stop.reason
        );
        assert!(!stop.budget.defaulted);
    }

    #[test]
    fn a_predicate_already_true_at_the_start_costs_no_instructions() {
        let f = Fixture::new("until-start");
        let mut h = handler(&f);
        assert!(run_frames(&mut h, 1).ok);
        let response = h.handle(&request(Op::Run(RunArgs {
            until: Some("1".into()),
            frames: Some(600),
            ..RunArgs::default()
        })));
        let stop = response.result.unwrap().stop.unwrap();
        assert!(matches!(stop.reason, StopReason::Predicate { .. }));
        assert_eq!(stop.budget.instructions, 0);
    }

    #[test]
    fn a_predicate_reading_never_written_memory_points_at_allow_uninit() {
        let f = Fixture::new("until-uninit");
        let mut h = handler(&f);
        let response = h.handle(&request(Op::Run(RunArgs {
            until: Some("[$C003] == 0".into()),
            frames: Some(600),
            ..RunArgs::default()
        })));
        let error = response.error.expect("an uninitialized read fails the run");
        assert_eq!(error.code, ErrorCode::UninitializedRead);
        assert!(error.remedy.contains("--allow-uninit"), "{}", error.remedy);
        assert!(error.stop.is_some(), "the stop record rides on the error");
    }

    #[test]
    fn step_one_instruction_advances_and_saves_a_head() {
        let f = Fixture::new("step");
        let mut h = handler(&f);
        let response = h.handle(&request(Op::Step(StepArgs {
            unit: StepKind::Instruction,
        })));
        let stop = response.result.unwrap().stop.unwrap();
        assert_eq!(
            stop.reason,
            StopReason::StepDone {
                unit: StepKind::Instruction
            }
        );
        assert_eq!(response.rev, Some(Rev(1)));
    }
}
