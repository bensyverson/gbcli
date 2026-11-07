//! `run` and `step` (design §11). Everything a request can get wrong is
//! checked before the machine moves; once it has moved, the state it reached
//! is saved whatever fails after, and the failure carries its stop record.

mod budget;

use gbcli_bundle::{SessionStore, Start, validate_checkpoint_name};
use gbcli_core::Budget;
use gbcli_engine::{DebugError, Machine, Outcome, RunOptions, unapplied_keys};
use gbcli_protocol::{
    ErrorBody, ErrorCode, KeyEvent, Location, ResultBody, RunArgs, SessionFile, StepArgs, StepKind,
    StopReason, StopRecord,
};

use super::history::write_checkpoint;
use super::reads::{CompiledRead, compile_reads, eval_reads, warning_text};
use super::writes::{apply_writes, parse_location};
use super::{Failure, Handler, HeadChange, Need, save_head, shot};
use crate::convert;
use budget::{DEFAULT_FRAMES, budget_of, unanswered, wall_guard_error};

/// Instructions a scanline step may take: far more than one line holds.
const SCANLINE_STEP_INSTRUCTIONS: u64 = 100_000;

/// What a run stops at besides its budget, resolved before it starts.
enum Target {
    Budget,
    Until(String),
    To(Location),
}

/// A run's request, compiled against the machine before it moves.
struct Plan {
    reads: Vec<CompiledRead>,
    target: Target,
}

impl Handler {
    pub(super) fn run(
        &mut self,
        store: &SessionStore,
        meta: &mut SessionFile,
        args: &RunArgs,
        actor: &Option<String>,
        warnings: &mut Vec<String>,
    ) -> Result<ResultBody, Failure> {
        // What needs no machine.
        if args.until.is_some() && args.to.is_some() {
            return Err(Failure::new(ErrorBody::new(
                ErrorCode::BadRequest,
                "a run stops at --until or --to, not both",
                "drop one of them",
            )));
        }
        let from = args.from.unwrap_or(Start::Boot);
        let since = self.since(store, args.since.as_ref())?;
        if let Some(name) = &args.checkpoint {
            validate_checkpoint_name(name).map_err(|e| Failure::from_store(&e))?;
        }
        if let Some(shot) = &args.shot {
            shot::check(shot)?;
        }

        // What needs the machine's symbols and sizes. A refusal here hands
        // the machine back untouched.
        let (mut machine, loaded) =
            self.machine_for(store, meta, Need::Start(from), args.at.as_deref())?;
        warnings.extend(loaded);
        let plan = match plan_of(&machine, args) {
            Ok(plan) => plan,
            Err(failure) => {
                if args.at.is_none() && from == Start::Boot {
                    self.keep_warm(store, meta, machine);
                }
                return Err(failure);
            }
        };

        // The machine moves from here on.
        let writes = apply_writes(&mut machine, &args.writes)?;
        let (budget, limits, defaulted) = budget_of(args);
        let interrupt = self.interrupt.clone();
        let mut opts = RunOptions::new(budget);
        opts.since = since.clone();
        opts.allow_uninit = args.allow_uninit;
        opts.interrupt = Some(&interrupt);
        opts.keys = &args.keys;
        let outcome = match plan.target {
            Target::Until(text) => machine.run_until(&text, &opts),
            Target::To(at) => machine.run_to(at, &opts),
            Target::Budget => machine.run(&opts),
        };
        let mut outcome = match outcome {
            Ok(outcome) => outcome,
            Err(error) => {
                if let Some(stop) = stop_of(&error) {
                    unapplied(&args.keys, stop, warnings);
                }
                let parent = args.at.clone();
                return Err(self.after_run_error(store, meta, machine, &error, actor, parent));
            }
        };
        let guarded = self.guarded(&mut outcome);
        outcome.stop.budget.limits = limits;
        outcome.stop.budget.defaulted = defaulted;
        warnings.extend(outcome.warnings.iter().map(warning_text));

        let stop = outcome.stop;
        let keys_unapplied = unapplied(&args.keys, &stop, warnings);
        let change = HeadChange {
            actor: actor.clone(),
            parent: args.at.clone(),
            stop: Some(&stop),
            ..HeadChange::default()
        };
        save_head(store, meta, &machine, change)?;
        // After the save, so the checkpoint names the rev it is.
        let checkpoint = match &args.checkpoint {
            Some(name) => Some(write_checkpoint(store, &machine, name, actor, meta.rev)),
            None => None,
        }
        .transpose();
        let reads = eval_reads(&machine, &plan.reads, &since, args.allow_uninit, warnings);
        // After the reads (design §11 step 6); capturing only reads the core.
        let shot = args.shot.as_ref().map(|s| shot::take(&machine, s, &since));
        self.keep_warm(store, meta, machine);

        // The run is saved; anything that failed after it rides with its stop.
        let with_stop = |f: Failure| f.with_stop(stop.clone());
        let checkpoint = checkpoint.map_err(with_stop)?;
        let reads = reads.map_err(with_stop)?;
        let shot = shot.transpose().map_err(with_stop)?;
        if guarded {
            return Err(Failure::new(wall_guard_error(self.wall_guard, stop)));
        }
        if let Some(body) = unanswered(args, defaulted, &stop) {
            return Err(Failure::new(body));
        }
        Ok(ResultBody {
            stop: Some(stop),
            reads: Some(reads),
            shot,
            keys_unapplied,
            checkpoint,
            writes: (!writes.is_empty()).then_some(writes),
            ..ResultBody::default()
        })
    }

    /// Rewrites an interrupted stop as the wall guard's, which is the only
    /// thing that interrupts a request; true when it did.
    fn guarded(&self, outcome: &mut Outcome) -> bool {
        let guarded = outcome.stop.reason == StopReason::UserBreak;
        if guarded {
            outcome.stop.reason = StopReason::WallGuard;
        }
        guarded
    }

    /// A run that failed: save the state it reached, and keep the stop record.
    /// If that save fails, the save's failure is the answer, since the stop
    /// record would describe a head that doesn't exist.
    fn after_run_error(
        &mut self,
        store: &SessionStore,
        meta: &mut SessionFile,
        machine: Machine,
        error: &DebugError,
        actor: &Option<String>,
        parent: Option<String>,
    ) -> Failure {
        let failure = Failure::from_debug(error);
        let Some(stop) = stop_of(error).cloned() else {
            return failure;
        };
        let change = HeadChange {
            actor: actor.clone(),
            parent,
            stop: Some(&stop),
            ..HeadChange::default()
        };
        if let Err(mut saved) = save_head(store, meta, &machine, change) {
            saved.body.message = format!("{} (after: {error})", saved.body.message);
            return saved;
        }
        self.keep_warm(store, meta, machine);
        failure.with_stop(stop)
    }

    pub(super) fn step(
        &mut self,
        store: &SessionStore,
        meta: &mut SessionFile,
        args: &StepArgs,
        actor: &Option<String>,
        warnings: &mut Vec<String>,
    ) -> Result<ResultBody, Failure> {
        if args.unit == StepKind::ToAddress {
            return Err(Failure::new(ErrorBody::new(
                ErrorCode::BadRequest,
                "step doesn't take an address",
                "use `run --to <address>`",
            )));
        }
        let need = Need::Start(Start::Boot);
        let (mut machine, loaded) = self.machine_for(store, meta, need, None)?;
        warnings.extend(loaded);
        let budget = match args.unit {
            StepKind::Frame => Budget::frames(1),
            StepKind::Instruction => Budget::instructions(1),
            StepKind::Over | StepKind::Out => Budget::frames(DEFAULT_FRAMES),
            StepKind::Scanline | StepKind::ToAddress => {
                Budget::instructions(SCANLINE_STEP_INSTRUCTIONS)
            }
        };
        let interrupt = self.interrupt.clone();
        let mut opts = RunOptions::new(budget);
        opts.interrupt = Some(&interrupt);
        let mut outcome = match machine.step(args.unit, &opts) {
            Ok(outcome) => outcome,
            Err(error) => {
                return Err(self.after_run_error(store, meta, machine, &error, actor, None));
            }
        };
        let guarded = self.guarded(&mut outcome);
        warnings.extend(outcome.warnings.iter().map(warning_text));
        let stop = outcome.stop;
        let change = HeadChange {
            actor: actor.clone(),
            stop: Some(&stop),
            ..HeadChange::default()
        };
        save_head(store, meta, &machine, change)?;
        self.keep_warm(store, meta, machine);
        if guarded {
            return Err(Failure::new(wall_guard_error(self.wall_guard, stop)));
        }
        Ok(ResultBody {
            stop: Some(stop),
            ..ResultBody::default()
        })
    }
}

/// Compiles the reads and resolves the target on `machine`, before it moves.
fn plan_of(machine: &Machine, args: &RunArgs) -> Result<Plan, Failure> {
    let reads = compile_reads(machine, &args.read)?;
    let target = match (&args.until, &args.to) {
        (Some(text), _) => {
            // Compiled here only to refuse it before any write; the run
            // compiles it again with its cadence.
            gbcli_engine::cond::Condition::compile(text, machine.symbols()).map_err(|error| {
                Failure::new(convert::debug(&DebugError::UntilSyntax { error }))
            })?;
            Target::Until(text.clone())
        }
        (None, Some(text)) => Target::To(parse_location(machine, text)?),
        (None, None) => Target::Budget,
    };
    Ok(Plan { reads, target })
}

/// The key events past the stop, as a warning and for the result: never
/// silently dropped (design §11 step 3).
fn unapplied(
    keys: &[KeyEvent],
    stop: &StopRecord,
    warnings: &mut Vec<String>,
) -> Option<Vec<KeyEvent>> {
    let late = unapplied_keys(keys, stop.budget.frames);
    if late.is_empty() {
        return None;
    }
    let tokens: Vec<String> = late.iter().map(KeyEvent::to_string).collect();
    warnings.push(format!(
        "the run stopped after {} frames, before {} key event(s): {}",
        stop.budget.frames,
        late.len(),
        tokens.join(" ")
    ));
    Some(late)
}

fn stop_of(error: &DebugError) -> Option<&StopRecord> {
    match error {
        DebugError::Eval { stop, .. } | DebugError::Until { stop, .. } => Some(stop),
        DebugError::Condition { .. }
        | DebugError::UntilSyntax { .. }
        | DebugError::Core(_)
        | DebugError::NeedsAddress => None,
    }
}

#[cfg(test)]
mod tests;
