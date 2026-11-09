//! `run` and `step` (design §11). Everything a request can get wrong is
//! checked before the machine moves; once it has moved, the state it reached
//! is saved whatever fails after, and the failure carries its stop record.

mod audio;
mod budget;
mod equiv;
mod expect;
mod filmstrip;
mod measure;
mod oam_log;
mod probes;
mod raster;

use gbcli_bundle::{SessionStore, validate_checkpoint_name};
use gbcli_core::Budget;
use gbcli_engine::cond::EvalWarning;
use gbcli_engine::{DebugError, EventTap, FrameSink, KeyTimeline, Machine, Outcome, RunOptions};
use gbcli_protocol::{
    ErrorBody, ErrorCode, FlashNoteRow, IgnoredWritesRow, KeyEvent, Location, ResultBody, RunArgs,
    SessionFile, StepArgs, StepKind, StopReason, StopRecord, WavChannels,
};

use super::history::write_checkpoint;
use super::reads::{CompiledRead, compile_reads, eval_reads_each, warning_text};
use super::writes::{apply_writes, parse_location};
use super::{Failure, Handler, HeadChange, Need, save_head, shot};
use crate::convert;
#[cfg(test)]
pub(super) use budget::WALL_GUARD_REMEDY;
use budget::{DEFAULT_FRAMES, budget_of, short_of_target, unanswered, wall_guard_error};
#[cfg(test)]
pub(super) use equiv::equiv_failure;
use probes::Probes;

/// Instructions a scanline step may take: far more than one line holds.
const SCANLINE_STEP_INSTRUCTIONS: u64 = 100_000;

/// What a run stops at besides its budget, resolved before it starts.
#[derive(Clone)]
enum Target {
    Budget,
    Until(String),
    To(Location),
}

/// A run's request, compiled against the machine before it moves.
struct Plan {
    reads: Vec<CompiledRead>,
    expect: Vec<CompiledRead>,
    target: Target,
    probes: Probes,
}

impl Handler {
    /// A failed run names the checkpoint it didn't save and the key events
    /// it didn't reach (design §11).
    pub(super) fn run(
        &mut self,
        store: &SessionStore,
        meta: &mut SessionFile,
        args: &RunArgs,
        actor: &Option<String>,
        warnings: &mut Vec<String>,
    ) -> Result<ResultBody, Failure> {
        let mut keys = None;
        let mut failure = match self.run_request(store, meta, args, actor, warnings, &mut keys) {
            Ok(result) => return Ok(result),
            Err(failure) => failure,
        };
        if let Some(name) = &args.checkpoint {
            failure.body.message += &format!("; checkpoint `{name}` was not saved");
        }
        if let (Some(stop), Some(keys)) = (&failure.body.stop, &keys) {
            failure.body.keys_unapplied = unapplied(keys, stop);
        }
        Err(failure)
    }

    /// `keys` is set to the run's key timeline once the machine is there to
    /// place it, before the machine moves.
    fn run_request(
        &mut self,
        store: &SessionStore,
        meta: &mut SessionFile,
        args: &RunArgs,
        actor: &Option<String>,
        warnings: &mut Vec<String>,
        keys: &mut Option<KeyTimeline>,
    ) -> Result<ResultBody, Failure> {
        // What needs no machine.
        if args.until.is_some() && args.to.is_some() {
            return Err(Failure::new(ErrorBody::new(
                ErrorCode::BadRequest,
                "a run stops at --until or --to, not both",
                "drop one of them",
            )));
        }
        if args.at.is_some() && args.from.is_some() {
            return Err(Failure::new(ErrorBody::new(
                ErrorCode::BadRequest,
                "a run starts from --at or --from, not both",
                "drop one of them: --at names the state to start from",
            )));
        }
        let since = self.since(store, args.since.as_ref())?;
        let keys_since = match &args.keys_since {
            Some(base) => Some(self.since(store, Some(base))?),
            None => None,
        };
        if let Some(name) = &args.checkpoint {
            validate_checkpoint_name(name).map_err(|e| Failure::from_store(&e))?;
        }
        if let Some(shot) = &args.shot {
            shot::check(shot, &self.host)?;
        }
        if let Some(strip) = &args.filmstrip {
            filmstrip::check(strip, &self.host)?;
        }
        if let Some(sound) = &args.audio {
            audio::check(sound, &self.host)?;
        }

        // What needs the machine's symbols and sizes. A refusal here hands
        // the machine back untouched.
        let (mut machine, loaded) =
            self.machine_for(store, meta, Need::Start(args.from), args.at.as_deref())?;
        warnings.extend(loaded);
        let planned = plan_of(&machine, args).and_then(|plan| {
            let timeline = match &keys_since {
                Some(base) => KeyTimeline::from_base(&args.keys, &machine, base)
                    .ok_or_else(keys_before_the_handoff)?,
                None => KeyTimeline::from_run_start(&args.keys),
            };
            Ok((plan, timeline))
        });
        let (plan, timeline) = match planned {
            Ok(planned) => planned,
            Err(failure) => {
                if args.at.is_none() && args.from.is_none() {
                    self.keep_warm(store, meta, machine);
                }
                return Err(failure);
            }
        };
        let timeline = &*keys.insert(timeline);

        // The machine moves from here on.
        let writes = apply_writes(&mut machine, &args.writes)?;
        let sound = args.audio.as_deref();
        // The per-channel replays start where the run starts: after the writes.
        let mut replay_from = match sound.and_then(|a| a.wav.as_ref()) {
            Some(w) if w.channels == WavChannels::Each => {
                Some(machine.try_clone().map_err(|e| Failure::from_core(&e))?)
            }
            _ => None,
        };
        let collecting = match sound {
            Some(a) => Some((audio::start(&mut machine, a)?, machine.core().now())),
            None => None,
        };
        let (budget, limits, defaulted) = budget_of(args);
        let interrupt = self.interrupt.clone();
        let host = self.host.clone();
        let collector = args
            .filmstrip
            .as_deref()
            .map(|strip| filmstrip::Collector::new(strip, &host));
        let oam_log = args.oam_log.then(oam_log::Collector::default);
        let sinks = plan.probes.sinks.as_ref();
        let sink = |m: &Machine, done: u64| {
            if let Some(c) = &collector {
                c.on_frame(m, done);
            }
            if let Some(o) = &oam_log {
                o.on_frame(m);
            }
            if let Some(s) = sinks {
                s.borrow_mut().cut(m);
            }
        };
        let mut opts = RunOptions::new(budget);
        opts.since = since.clone();
        opts.allow_uninit = args.allow_uninit;
        opts.interrupt = Some(&interrupt);
        opts.keys = timeline.events();
        // The audio replays run the same request without collecting frames.
        let replay_opts = opts.clone();
        if collector.is_some() || oam_log.is_some() || plan.probes.wants_frames() {
            opts.on_frame = Some(FrameSink(&sink));
        }
        if let Some(s) = sinks {
            opts.on_event = Some(EventTap(s));
        }
        let outcome = run_target(&mut machine, &plan.target, &opts);
        // Whatever the outcome, so the machine kept warm stops collecting.
        let collected = match (sound, collecting) {
            (Some(a), Some((snapshot, from))) => {
                Some(audio::finish(&mut machine, a, snapshot, from))
            }
            _ => None,
        };
        let mut outcome = match outcome {
            Ok(outcome) => outcome,
            Err(error) => {
                let flash_notes = report_flash_failed(&mut machine, warnings);
                let parent = args.at.clone();
                let failure = self.after_run_error(store, meta, machine, &error, actor, parent);
                return Err(failure.with_flash_notes(flash_notes));
            }
        };
        let guarded = self.guarded(&mut outcome);
        outcome.stop.budget.limits = limits;
        outcome.stop.budget.defaulted = defaulted;
        warnings.extend(outcome.warnings.iter().map(warning_text));
        let ignored_writes = ignored_rows(&outcome.warnings);
        let flash_notes = report_flash(&mut machine, warnings)?;

        let stop = outcome.stop;
        let change = HeadChange {
            actor: actor.clone(),
            parent: args.at.clone(),
            stop: Some(&stop),
            ..HeadChange::default()
        };
        save_head(store, meta, &mut machine, change)?;
        let reads = eval_reads_each(
            &machine,
            &plan.reads,
            &since,
            args.allow_uninit,
            warnings,
            &mut self.gaps_found,
        );
        let expected = expect::evaluate(
            &machine,
            &plan.expect,
            &since,
            args.allow_uninit,
            warnings,
            &mut self.gaps_found,
        );
        // After the reads (design §11 step 6); capturing only reads the core.
        let shot = args
            .shot
            .as_ref()
            .map(|s| shot::take(&machine, s, &since, &host));
        let answered = self.answered(args, defaulted, guarded, &stop);
        warnings.extend(short_of_target(args, &stop));
        // Step 5, on a copy of the stop; and what the run's sinks collected.
        let measured = plan
            .probes
            .measure(&mut machine, &interrupt, self.wall_guard);
        let collected_probes = plan.probes.finish(&machine, &since);
        let oam_made = oam_log.as_ref().map(|o| o.finish(&machine, &since));
        // The core's gaps reach the ledger whether or not the request fails.
        self.gaps_found
            .extend(collected_probes.gaps.iter().cloned());
        // The files after the shot are written only when everything before
        // them succeeded: the filmstrip, then the audio, whose per-channel
        // replays run the request four more times.
        let so_far = shot.as_ref().is_none_or(Result::is_ok)
            && answered.is_ok()
            && measured.as_ref().is_none_or(Result::is_ok);
        let strip = match &collector {
            Some(c) if so_far => Some(c.write(&machine, &since, warnings)),
            _ => None,
        };
        let so_far = so_far && strip.as_ref().is_none_or(Result::is_ok);
        let sound = match (sound, collected) {
            (Some(a), Some(collected)) if so_far => Some(collected.and_then(|c| {
                let guard = self.wall_guard;
                let drive = |m: &mut Machine| replay(m, &plan.target, &replay_opts, guard);
                audio::deliver(a, c, &machine, &since, &host, || {
                    audio::takes(replay_from.as_mut(), drive)
                })
            })),
            _ => None,
        };
        // Last, and only when everything before it succeeded (step 7); after
        // the head's save, so the checkpoint records the rev it is. A false
        // expectation keeps the files, its evidence, but names no state.
        let so_far = so_far && sound.as_ref().is_none_or(Result::is_ok) && expected.held();
        let checkpoint = match &args.checkpoint {
            Some(name) if so_far => {
                Some(write_checkpoint(store, &mut machine, name, actor, meta.rev))
            }
            _ => None,
        }
        .transpose();
        self.keep_warm(store, meta, machine);

        // The run is saved; anything that failed after it rides with its stop.
        let with_stop = |f: Failure| {
            f.with_stop(stop.clone())
                .with_flash_notes(flash_notes.clone())
        };
        let shot = shot.transpose().map_err(with_stop)?;
        answered.map_err(|f| f.with_flash_notes(flash_notes.clone()))?;
        let measured = measured.transpose().map_err(with_stop)?;
        let strip = strip.transpose().map_err(with_stop)?;
        let sound = sound.transpose().map_err(with_stop)?;
        let oam_made = oam_made.transpose().map_err(with_stop)?;
        let checkpoint = checkpoint.map_err(with_stop)?;
        let keys_unapplied = unapplied(timeline, &stop);
        let result = ResultBody {
            stop: Some(stop.clone()),
            reads: Some(reads),
            expect: expected.rows(),
            shot,
            filmstrip: strip,
            measure: measured,
            profile: collected_probes.profile,
            frame_budget: collected_probes.frame_budget,
            raster: collected_probes.raster,
            audio: sound,
            oam_log: oam_made,
            keys_unapplied,
            ignored_writes,
            checkpoint,
            writes: (!writes.is_empty()).then_some(writes),
            flash_notes: flash_notes.clone(),
            ..ResultBody::default()
        };
        expected.verdict(result).map_err(with_stop)
    }

    /// Whether the run reached what it was asked for: not stopped by the
    /// wall guard, and not out of budget short of an explicit target.
    fn answered(
        &self,
        args: &RunArgs,
        defaulted: bool,
        guarded: bool,
        stop: &StopRecord,
    ) -> Result<(), Failure> {
        if guarded {
            return Err(Failure::new(wall_guard_error(
                self.wall_guard,
                stop.clone(),
            )));
        }
        match unanswered(args, defaulted, stop) {
            Some(body) => Err(Failure::new(body)),
            None => Ok(()),
        }
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
        mut machine: Machine,
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
        if let Err(mut saved) = save_head(store, meta, &mut machine, change) {
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
        let need = Need::Start(None);
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
                let flash_notes = report_flash_failed(&mut machine, warnings);
                let failure = self.after_run_error(store, meta, machine, &error, actor, None);
                return Err(failure.with_flash_notes(flash_notes));
            }
        };
        let guarded = self.guarded(&mut outcome);
        warnings.extend(outcome.warnings.iter().map(warning_text));
        let ignored_writes = ignored_rows(&outcome.warnings);
        let flash_notes = report_flash(&mut machine, warnings)?;
        let stop = outcome.stop;
        let change = HeadChange {
            actor: actor.clone(),
            stop: Some(&stop),
            ..HeadChange::default()
        };
        save_head(store, meta, &mut machine, change)?;
        self.keep_warm(store, meta, machine);
        if guarded {
            let failure = Failure::new(wall_guard_error(self.wall_guard, stop));
            return Err(failure.with_flash_notes(flash_notes));
        }
        Ok(ResultBody {
            stop: Some(stop),
            ignored_writes,
            flash_notes,
            ..ResultBody::default()
        })
    }
}

/// The writes the PPU ignored among a run's warnings, as rows: every field
/// of those warnings, which the text carries too.
fn ignored_rows(warnings: &[EvalWarning]) -> Option<Vec<IgnoredWritesRow>> {
    let rows: Vec<IgnoredWritesRow> = warnings
        .iter()
        .filter_map(|w| match w {
            EvalWarning::WritesIgnored(found) => Some(IgnoredWritesRow {
                kind: found.kind,
                count: found.count,
                pc: found.pc,
                symbol: found.symbol.clone(),
                addr: found.addr,
                line: found.line,
                frame: found.frame,
            }),
            _ => None,
        })
        .collect();
    (!rows.is_empty()).then_some(rows)
}

/// Takes the flash notes the machine took, a mask-ROM cart's hint too
/// ([`Machine::take_flash_report`]): each a warning, and the rows.
fn report_flash(
    machine: &mut Machine,
    warnings: &mut Vec<String>,
) -> Result<Option<Vec<FlashNoteRow>>, Failure> {
    let report = machine
        .take_flash_report()
        .map_err(|e| Failure::from_core(&e))?;
    warnings.extend(report.warnings);
    Ok(report.rows)
}

/// [`report_flash`] for a run or step that already failed: its own failure
/// stays the answer if the notes can't be read, and a warning says so.
fn report_flash_failed(
    machine: &mut Machine,
    warnings: &mut Vec<String>,
) -> Option<Vec<FlashNoteRow>> {
    report_flash(machine, warnings).unwrap_or_else(|f| {
        warnings.push(format!("the flash notes were lost: {}", f.body.message));
        None
    })
}

/// Runs `machine` to `target`.
fn run_target(
    machine: &mut Machine,
    target: &Target,
    opts: &RunOptions<'_>,
) -> Result<Outcome, DebugError> {
    match target {
        Target::Until(text) => machine.run_until(text, opts),
        Target::To(at) => machine.run_to(at.clone(), opts),
        Target::Budget => machine.run(opts),
    }
}

/// One of the per-channel WAVs' replays of the run. They share the request's
/// interrupt flag, so a replay the wall guard ended is the wall guard's error,
/// not a replay that diverged from the run.
fn replay(
    machine: &mut Machine,
    target: &Target,
    opts: &RunOptions<'_>,
    guard: std::time::Duration,
) -> Result<(), Failure> {
    let mut stop = run_target(machine, target, opts)
        .map_err(|e| Failure::from_debug(&e))?
        .stop;
    if stop.reason == StopReason::UserBreak {
        stop.reason = StopReason::WallGuard;
        return Err(Failure::new(wall_guard_error(guard, stop)));
    }
    Ok(())
}

/// Compiles the reads and resolves the target on `machine`, before it moves.
fn plan_of(machine: &Machine, args: &RunArgs) -> Result<Plan, Failure> {
    if let Some(sound) = &args.audio {
        audio::check_machine(machine, sound)?;
    }
    let reads = compile_reads(machine, &args.read)?;
    let expect = expect::compile(machine, &args.expect)?;
    let probes = Probes::plan(machine, args)?;
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
    Ok(Plan {
        reads,
        expect,
        target,
        probes,
    })
}

/// `--keys-since boot` on a run that starts inside the boot ROM.
fn keys_before_the_handoff() -> Failure {
    Failure::new(ErrorBody::new(
        ErrorCode::BadRequest,
        "--keys-since boot counts from the boot handoff, which hasn't happened where this run \
         starts (inside the boot ROM)",
        "count the keys from power-on with --keys-since power-on, or start the run from boot \
         (--from boot, or --at a checkpoint after the handoff)",
    ))
}

/// The key events past the stop, for the result: never silently dropped
/// (design §11 step 3). No warning repeats them: a long keys file would
/// bury the stop.
fn unapplied(keys: &KeyTimeline, stop: &StopRecord) -> Option<Vec<KeyEvent>> {
    let late = keys.unapplied(stop.budget.frames);
    (!late.is_empty()).then_some(late)
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
mod checkpoint_tests;
#[cfg(test)]
mod expect_tests;
#[cfg(test)]
mod target_tests;
#[cfg(test)]
mod tests;
