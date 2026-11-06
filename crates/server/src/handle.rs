//! Applying one request to one session (design §5, §11): lock, restore, act,
//! save the head atomically, log. The handler keeps the machine warm between
//! requests and reloads it when another writer moved the session's head.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use gbcli_bundle::{Bundle, LOCK_WAIT, SessionStore, Start, next};
use gbcli_core::{Budget, Region};
use gbcli_engine::{
    DebugError, LoadOptions, Machine, RunOptions, SaveMeta, Since, addr, cond::Condition,
    cond::EvalCtx, cond::EvalWarning, mem,
};
use gbcli_protocol::{
    BreakArgs, BreakpointMade, BudgetLimits, CheckpointArgs, CheckpointMade, DeleteArgs, Deleted,
    ErrorBody, ErrorCode, EventLogLine, EventsArgs, Op, PpuView, PruneArgs, ReadArgs, ReadValue,
    RegistersView, Request, Response, RestoreArgs, Restored, ResultBody, Rev, RmArgs, RunArgs,
    SessionFile, StepArgs, StepKind, StopReason, StopRecord, StopSummary, TimeMark, WatchArgs,
    WatchMade, WriteArg, WriteArgs, WriteKind, WriteMade,
};

use crate::convert::{self, debug as debug_body};

/// The wall-clock backstop on every run (design §3).
const WALL_GUARD: Duration = Duration::from_secs(60);
/// The default budget for `run` without one (design Q4).
const DEFAULT_FRAMES: u64 = 600;

/// One server side: a project's bundle, a default session and actor, and the
/// machine kept warm between requests.
pub struct Handler {
    bundle: Bundle,
    default_session: Option<String>,
    default_actor: Option<String>,
    warm: Option<Warm>,
}

struct Warm {
    session: String,
    rev: Rev,
    machine: Machine,
}

/// A failed request: its body, and the gap-ledger entry it deserves.
struct Failure {
    /// Boxed: a stop record makes an `ErrorBody` large, and every `Result`
    /// here pays for its error type on the stack.
    body: Box<ErrorBody>,
    gap: Option<(&'static str, &'static str)>,
}

impl Failure {
    fn new(body: ErrorBody) -> Self {
        Failure {
            body: Box::new(body),
            gap: None,
        }
    }

    fn from_store(error: &gbcli_bundle::StoreError) -> Self {
        Failure::new(convert::store(error))
    }

    fn from_debug(error: &DebugError) -> Self {
        Failure {
            body: Box::new(debug_body(error)),
            gap: convert::gap_of(error),
        }
    }
}

impl From<ErrorBody> for Failure {
    fn from(body: ErrorBody) -> Self {
        Failure::new(body)
    }
}

/// How a request gets its machine.
#[derive(Clone, Copy)]
enum Need {
    /// Read-only: the head, or power-on when there is none; never the boot run.
    Read,
    /// Advance: from `at`, else the head, else the boot handoff.
    Run,
}

impl Handler {
    pub fn new(
        bundle: Bundle,
        default_session: Option<String>,
        default_actor: Option<String>,
    ) -> Handler {
        Handler {
            bundle,
            default_session,
            default_actor,
            warm: None,
        }
    }

    pub fn bundle(&self) -> &Bundle {
        &self.bundle
    }

    /// Answers one request. Every request takes the session's lock, so two
    /// processes alternate on one head (design §1).
    pub fn handle(&mut self, request: &Request) -> Response {
        let id = request.id.clone();
        match &request.op {
            Op::Sessions => self.list_sessions(id),
            Op::Gaps => self.gaps(id),
            Op::Prune(args) => self.prune(id, args),
            Op::Rm(args) => self.rm(id, args),
            _ => self.session_request(id, request),
        }
    }

    fn list_sessions(&self, id: String) -> Response {
        match self.bundle.list() {
            Ok(rows) => Response::ok(
                id,
                None,
                ResultBody {
                    sessions: Some(rows),
                    ..ResultBody::default()
                },
            ),
            Err(e) => Response::failed(id, None, convert::store(&e)),
        }
    }

    fn gaps(&self, id: String) -> Response {
        match self.bundle.gap_counts() {
            Ok(rows) => Response::ok(
                id,
                None,
                ResultBody {
                    gaps: Some(rows),
                    ..ResultBody::default()
                },
            ),
            Err(e) => Response::failed(id, None, convert::store(&e)),
        }
    }

    fn prune(&self, id: String, args: &PruneArgs) -> Response {
        let unused_for = match gbcli_bundle::parse_duration(&args.unused_for) {
            Ok(d) => d,
            Err(e) => return Response::failed(id, None, convert::store(&e)),
        };
        match self.bundle.prune(unused_for) {
            Ok(pruned) => Response::ok(
                id,
                None,
                ResultBody {
                    pruned: Some(pruned),
                    ..ResultBody::default()
                },
            ),
            Err(e) => Response::failed(id, None, convert::store(&e)),
        }
    }

    fn rm(&self, id: String, args: &RmArgs) -> Response {
        let store = match self.bundle.session(&args.name) {
            Ok(store) => store,
            Err(e) => return Response::failed(id, None, convert::store(&e)),
        };
        match store.remove(args.with_checkpoints) {
            Ok(()) => Response::ok(
                id,
                None,
                ResultBody {
                    removed: Some(args.name.clone()),
                    ..ResultBody::default()
                },
            ),
            Err(e) => Response::failed(id, None, convert::store(&e)),
        }
    }

    fn session_request(&mut self, id: String, request: &Request) -> Response {
        let actor = request.actor.clone().or_else(|| self.default_actor.clone());
        let Some(name) = request
            .session
            .clone()
            .or_else(|| self.default_session.clone())
        else {
            let body = ErrorBody::new(
                ErrorCode::NoSession,
                "the request names no session",
                "pass --session <name>, or name it in the request",
            );
            return Response::failed(id, None, body);
        };
        let store = match self.bundle.session(&name) {
            Ok(store) => store,
            Err(e) => return Response::failed(id, None, convert::store(&e)),
        };
        let _lock = match store.lock(LOCK_WAIT) {
            Ok(lock) => lock,
            Err(e) => return Response::failed(id, None, convert::store(&e)),
        };
        let mut meta = match store.read_meta() {
            Ok(meta) => meta,
            Err(e) => return Response::failed(id, None, convert::store(&e)),
        };
        let rev_before = meta.rev;

        // A mutation may name the revision it read; a mismatch names the writer.
        if request.op.mutates_head()
            && let Some(asked) = request.rev
            && asked != meta.rev
        {
            let by = store.events(Some(20)).ok().and_then(|rows| {
                rows.iter()
                    .rev()
                    .find(|row| row.rev_after == meta.rev)
                    .and_then(|row| row.actor.clone())
            });
            let error = gbcli_bundle::StoreError::RevMismatch {
                expected: asked.0,
                found: meta.rev.0,
                by,
            };
            return self.log(
                id,
                store,
                &meta,
                actor,
                request,
                rev_before,
                Err(Failure::from_store(&error)),
                Vec::new(),
            );
        }

        let mut warnings: Vec<String> = Vec::new();
        let outcome = self.dispatch(&store, &mut meta, request, &actor, &mut warnings);
        self.log(
            id, store, &meta, actor, request, rev_before, outcome, warnings,
        )
    }

    fn dispatch(
        &mut self,
        store: &SessionStore,
        meta: &mut SessionFile,
        request: &Request,
        actor: &Option<String>,
        warnings: &mut Vec<String>,
    ) -> Result<ResultBody, Failure> {
        match &request.op {
            Op::Run(args) => self.run(store, meta, args, actor, warnings),
            Op::Step(args) => self.step(store, meta, args, warnings),
            Op::Show => self.show(store, meta),
            Op::Read(args) => self.read(store, meta, args),
            Op::Break(args) => self.breakpoint(store, meta, args),
            Op::Watch(args) => self.watch(store, meta, args),
            Op::Delete(args) => self.delete(store, meta, args),
            Op::Checkpoint(args) => self.checkpoint(store, meta, args, actor),
            Op::Restore(args) => self.restore(store, meta, args, actor, warnings),
            Op::Undo => self.undo(store, meta, actor),
            Op::Write(args) => self.write(store, meta, args, actor),
            Op::Events(args) => self.events(store, args),
            Op::Sessions | Op::Gaps | Op::Prune(_) | Op::Rm(_) => {
                Err(Failure::new(ErrorBody::new(
                    ErrorCode::BadRequest,
                    "that op doesn't name a session",
                    ErrorCode::BadRequest.default_remedy(),
                )))
            }
        }
    }

    /// The machine a request starts from: warm when another writer hasn't
    /// moved the head, else rebuilt (and, for a run, booted to the handoff).
    fn machine_for(
        &mut self,
        store: &SessionStore,
        meta: &SessionFile,
        need: Need,
        at: Option<&str>,
        from: Start,
    ) -> Result<(Machine, Vec<String>), Failure> {
        let cold = matches!(need, Need::Run) && from == Start::PowerOn;
        let warm = at.is_none()
            && !cold
            && meta.rev.0 > 0
            && self
                .warm
                .as_ref()
                .is_some_and(|w| w.session == store.name() && w.rev == meta.rev);
        let (mut machine, warnings) = if warm {
            (self.warm.take().expect("checked above").machine, Vec::new())
        } else {
            let from = match need {
                Need::Read => Start::PowerOn,
                Need::Run => from,
            };
            store
                .start_machine(meta, at, from, &LoadOptions::default())
                .map_err(|e| Failure::from_store(&e))?
        };
        *machine.debugger_config_mut() = meta.debugger.clone();
        Ok((machine, warnings))
    }

    fn keep_warm(&mut self, store: &SessionStore, meta: &SessionFile, machine: Machine) {
        self.warm = Some(Warm {
            session: store.name().to_string(),
            rev: meta.rev,
            machine,
        });
    }

    fn run(
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

    fn step(
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

    fn show(&mut self, store: &SessionStore, meta: &SessionFile) -> Result<ResultBody, Failure> {
        let (machine, _) = self.machine_for(store, meta, Need::Read, None, Start::PowerOn)?;
        let result = ResultBody {
            registers: Some(RegistersView::of(&machine.core().registers())),
            ppu: Some(PpuView::of(&machine.core().ppu())),
            stop: meta.last_stop.clone(),
            ..ResultBody::default()
        };
        self.keep_warm(store, meta, machine);
        Ok(result)
    }

    fn events(&self, store: &SessionStore, args: &EventsArgs) -> Result<ResultBody, Failure> {
        let rows = store
            .events(args.limit)
            .map_err(|e| Failure::from_store(&e))?;
        Ok(ResultBody {
            events: Some(rows),
            ..ResultBody::default()
        })
    }

    fn read(
        &mut self,
        store: &SessionStore,
        meta: &SessionFile,
        args: &ReadArgs,
    ) -> Result<ResultBody, Failure> {
        let since = self.since(store, args.since.as_deref())?;
        let (machine, _) = self.machine_for(store, meta, Need::Read, None, Start::PowerOn)?;
        let reads = read_values(&machine, &args.reads, &since, &mut Vec::new())?;
        self.keep_warm(store, meta, machine);
        Ok(ResultBody {
            reads: Some(reads),
            ..ResultBody::default()
        })
    }

    fn breakpoint(
        &mut self,
        store: &SessionStore,
        meta: &mut SessionFile,
        args: &BreakArgs,
    ) -> Result<ResultBody, Failure> {
        let (mut machine, _) = self.machine_for(store, meta, Need::Read, None, Start::PowerOn)?;
        let at = addr::parse(&args.at, machine.symbols())
            .map_err(|e| Failure::new(convert::addr(&e)))?;
        let id = machine
            .add_breakpoint(at.clone(), args.condition.as_deref())
            .map_err(|e| Failure::new(convert::expr(&e)))?;
        meta.debugger = machine.debugger_config().clone();
        store
            .write_meta(meta)
            .map_err(|e| Failure::from_store(&e))?;
        let made = machine
            .debugger_config()
            .breakpoints
            .iter()
            .find(|b| b.id == id)
            .map(|b| b.at.clone())
            .unwrap_or(at);
        self.keep_warm(store, meta, machine);
        Ok(ResultBody {
            breakpoint: Some(BreakpointMade { id, at: made }),
            ..ResultBody::default()
        })
    }

    fn watch(
        &mut self,
        store: &SessionStore,
        meta: &mut SessionFile,
        args: &WatchArgs,
    ) -> Result<ResultBody, Failure> {
        let (mut machine, _) = self.machine_for(store, meta, Need::Read, None, Start::PowerOn)?;
        let at = addr::parse(&args.at, machine.symbols())
            .map_err(|e| Failure::new(convert::addr(&e)))?;
        let id = machine
            .add_watch(
                args.access,
                at.clone(),
                args.condition.as_deref(),
                args.once,
            )
            .map_err(|e| Failure::new(convert::expr(&e)))?;
        meta.debugger = machine.debugger_config().clone();
        store
            .write_meta(meta)
            .map_err(|e| Failure::from_store(&e))?;
        let made = machine
            .debugger_config()
            .watches
            .iter()
            .find(|w| w.id == id)
            .map(|w| w.at.clone())
            .unwrap_or(at);
        self.keep_warm(store, meta, machine);
        Ok(ResultBody {
            watch: Some(WatchMade {
                id,
                access: args.access,
                at: made,
            }),
            ..ResultBody::default()
        })
    }

    fn delete(
        &mut self,
        store: &SessionStore,
        meta: &mut SessionFile,
        args: &DeleteArgs,
    ) -> Result<ResultBody, Failure> {
        let (mut machine, _) = self.machine_for(store, meta, Need::Read, None, Start::PowerOn)?;
        let found = machine.delete(args.id);
        meta.debugger = machine.debugger_config().clone();
        store
            .write_meta(meta)
            .map_err(|e| Failure::from_store(&e))?;
        self.keep_warm(store, meta, machine);
        Ok(ResultBody {
            deleted: Some(Deleted { id: args.id, found }),
            ..ResultBody::default()
        })
    }

    fn checkpoint(
        &mut self,
        store: &SessionStore,
        meta: &SessionFile,
        args: &CheckpointArgs,
        actor: &Option<String>,
    ) -> Result<ResultBody, Failure> {
        let (machine, _) = self.machine_for(store, meta, Need::Read, None, Start::PowerOn)?;
        let made = write_checkpoint(store, &machine, &args.name, actor, meta.rev)?;
        self.keep_warm(store, meta, machine);
        Ok(ResultBody {
            checkpoint: Some(made),
            ..ResultBody::default()
        })
    }

    fn restore(
        &mut self,
        store: &SessionStore,
        meta: &mut SessionFile,
        args: &RestoreArgs,
        actor: &Option<String>,
        warnings: &mut Vec<String>,
    ) -> Result<ResultBody, Failure> {
        let (mut machine, _) = self.machine_for(store, meta, Need::Read, None, Start::PowerOn)?;
        let cp = store
            .checkpoint(&args.name)
            .map_err(|e| Failure::from_store(&e))?;
        let load = LoadOptions {
            allow_rom_change: args.allow_rom_change,
            ..LoadOptions::default()
        };
        warnings.extend(
            store
                .restore(&mut machine, &cp, &load)
                .map_err(|e| Failure::from_store(&e))?,
        );
        let frame = checkpoint_frame(&cp);
        save_head(
            store,
            meta,
            &machine,
            actor.clone(),
            Some(args.name.clone()),
            None,
        )?;
        self.keep_warm(store, meta, machine);
        Ok(ResultBody {
            restored: Some(Restored {
                name: Some(args.name.clone()),
                rev: meta.rev,
                frame: Some(frame),
            }),
            ..ResultBody::default()
        })
    }

    fn undo(
        &mut self,
        store: &SessionStore,
        meta: &mut SessionFile,
        actor: &Option<String>,
    ) -> Result<ResultBody, Failure> {
        let (mut machine, _) = self.machine_for(store, meta, Need::Read, None, Start::PowerOn)?;
        let cp = store
            .checkpoint_or_history(meta.rev, "~1")
            .map_err(|e| Failure::from_store(&e))?;
        let frame = checkpoint_frame(&cp);
        let load = LoadOptions::default();
        store
            .restore(&mut machine, &cp, &load)
            .map_err(|e| Failure::from_store(&e))?;
        save_head(
            store,
            meta,
            &machine,
            actor.clone(),
            Some("undo".into()),
            None,
        )?;
        self.keep_warm(store, meta, machine);
        Ok(ResultBody {
            restored: Some(Restored {
                name: None,
                rev: meta.rev,
                frame: Some(frame),
            }),
            ..ResultBody::default()
        })
    }

    fn write(
        &mut self,
        store: &SessionStore,
        meta: &mut SessionFile,
        args: &WriteArgs,
        actor: &Option<String>,
    ) -> Result<ResultBody, Failure> {
        let (mut machine, _) = self.machine_for(store, meta, Need::Read, None, Start::PowerOn)?;
        let writes = apply_writes(&mut machine, &args.writes)?;
        save_head(store, meta, &machine, actor.clone(), None, None)?;
        self.keep_warm(store, meta, machine);
        Ok(ResultBody {
            writes: Some(writes),
            ..ResultBody::default()
        })
    }

    /// The time base a request's `--since` names (design §9).
    fn since(&self, store: &SessionStore, text: Option<&str>) -> Result<Since, Failure> {
        match text {
            None | Some("boot") => Ok(Since::Boot),
            Some("power-on") => Ok(Since::PowerOn),
            Some(name) => {
                let cp = store
                    .checkpoint(name)
                    .map_err(|e| Failure::from_store(&e))?;
                Ok(Since::Checkpoint {
                    name: name.to_string(),
                    mark: TimeMark {
                        ticks: cp.header.ticks,
                        frame: cp.header.frame,
                    },
                })
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn log(
        &mut self,
        id: String,
        store: SessionStore,
        meta: &SessionFile,
        actor: Option<String>,
        request: &Request,
        rev_before: Rev,
        outcome: Result<ResultBody, Failure>,
        warnings: Vec<String>,
    ) -> Response {
        let rev_after = meta.rev;
        let mut gap_map = std::collections::BTreeMap::new();
        let mut code = None;
        let response = match outcome {
            Ok(result) => {
                let stop = result.stop.as_ref().map(summary);
                (Response::ok(id, Some(rev_after), result), stop)
            }
            Err(Failure { body, gap }) => {
                code = Some(body.code);
                let stop = body.stop.as_ref().map(summary);
                if let Some((feature, reason)) = gap {
                    let _ = self.bundle.append_gap(
                        Some(store.name()),
                        request.op.name(),
                        feature,
                        reason,
                    );
                    if let Ok(counts) = self.bundle.gap_counts() {
                        for row in counts {
                            gap_map.insert(row.feature, row.count);
                        }
                    }
                }
                (Response::failed(id, Some(rev_after), *body), stop)
            }
        };
        let (response, stop) = response;
        let _ = store.append_events(&[EventLogLine {
            at: gbcli_bundle::now_secs(),
            actor,
            op: request.op.name().to_string(),
            ok: response.ok,
            code,
            rev_before,
            rev_after,
            gaps: gap_map,
            stop,
        }]);
        response.with_warnings(warnings)
    }
}

/// A checkpoint's frame since the boot handoff when it has one, else since
/// power-on: the number `checkpoint` and `restore` report (design §12).
fn checkpoint_frame(cp: &gbcli_engine::Checkpoint) -> u64 {
    let base = cp.header.boot.map_or(0, |boot| boot.frame);
    cp.header.frame.saturating_sub(base)
}

fn summary(stop: &StopRecord) -> StopSummary {
    StopSummary {
        reason: stop.reason.clone(),
        pc: stop.pc,
        frame: stop.time.frame,
        ticks: stop.time.ticks,
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

fn save_head(
    store: &SessionStore,
    meta: &mut SessionFile,
    machine: &Machine,
    actor: Option<String>,
    parent: Option<String>,
    stop: Option<&StopRecord>,
) -> Result<(), Failure> {
    let cp = machine.capture(SaveMeta {
        created_by: actor,
        parent,
    });
    let rev = next(meta.rev);
    store
        .write_head(&cp, rev)
        .map_err(|e| Failure::from_store(&e))?;
    meta.rev = rev;
    meta.last_stop = stop.cloned();
    store
        .write_meta(meta)
        .map_err(|e| Failure::from_store(&e))?;
    Ok(())
}

fn write_checkpoint(
    store: &SessionStore,
    machine: &Machine,
    name: &str,
    actor: &Option<String>,
    rev: Rev,
) -> Result<CheckpointMade, Failure> {
    let cp = machine.capture(SaveMeta {
        created_by: actor.clone(),
        parent: None,
    });
    let frame = checkpoint_frame(&cp);
    store
        .write_checkpoint(name, &cp)
        .map_err(|e| Failure::from_store(&e))?;
    Ok(CheckpointMade {
        name: name.to_string(),
        path: store.display_path(&store.dir().join("checkpoints").join(format!("{name}.gbck"))),
        frame: Some(frame),
        rev,
    })
}

fn warning_text(warning: &EvalWarning) -> String {
    match warning {
        EvalWarning::UninitializedRead(read) => format!("uninitialized read of {read}"),
    }
}

fn read_values(
    machine: &Machine,
    reads: &[String],
    since: &Since,
    warnings: &mut Vec<String>,
) -> Result<Vec<ReadValue>, Failure> {
    let base = machine.time_base(since);
    let mut values = Vec::new();
    for text in reads {
        let condition = Condition::compile(text, machine.symbols())
            .map_err(|e| Failure::new(convert::expr(&e)))?;
        let ctx = EvalCtx {
            core: machine.core(),
            symbols: machine.symbols(),
            access: None,
            base,
            allow_uninit: false,
        };
        let mut eval_warnings: Vec<EvalWarning> = Vec::new();
        let value = condition
            .eval(&ctx, &mut eval_warnings)
            .map_err(|e| Failure {
                body: Box::new(convert::eval(&e)),
                gap: convert::gap_of_eval(&e),
            })?;
        warnings.extend(eval_warnings.iter().map(warning_text));
        values.push(ReadValue {
            expr: text.clone(),
            value,
        });
    }
    Ok(values)
}

/// Applies the three kinds of write in order (design §4).
fn apply_writes(machine: &mut Machine, writes: &[WriteArg]) -> Result<Vec<WriteMade>, Failure> {
    let mut made = Vec::new();
    for write in writes {
        match write.kind {
            WriteKind::Bus => {
                let at = addr::parse(&write.target, machine.symbols())
                    .map_err(|e| Failure::new(convert::addr(&e)))?;
                let base = at.addr;
                for (i, &byte) in write.bytes.iter().enumerate() {
                    machine
                        .core_mut()
                        .bus_write(base.wrapping_add(i as u16), byte)
                        .map_err(|e| Failure::new(convert::core_error(&e)))?;
                }
                made.push(WriteMade {
                    kind: write.kind,
                    target: write.target.clone(),
                    bytes: write.bytes.clone(),
                    addr: Some(base),
                });
            }
            WriteKind::Poke => {
                let target = addr::parse_write(&write.target, machine.symbols())
                    .map_err(|e| Failure::new(convert::addr(&e)))?;
                let gbcli_engine::addr::WriteTarget::Cpu(at) = target else {
                    return Err(Failure::new(ErrorBody::new(
                        ErrorCode::Refused,
                        "a poke can't target a ROM file offset",
                        "use the patch kind; ROM changes are recorded",
                    )));
                };
                let base = at.addr;
                for (i, &byte) in write.bytes.iter().enumerate() {
                    let addr = base.wrapping_add(i as u16);
                    let target = mem::target(machine.core(), addr)
                        .map_err(|e| Failure::new(convert::core_error(&e)))?;
                    let (region, offset) = match target {
                        mem::Target::Backing { region, offset } => (region, offset),
                        mem::Target::Io(reg) => (Region::Io, usize::from(reg.0)),
                    };
                    machine
                        .core_mut()
                        .poke(region, offset as u32, &[byte])
                        .map_err(|e| Failure::new(convert::core_error(&e)))?;
                }
                made.push(WriteMade {
                    kind: write.kind,
                    target: write.target.clone(),
                    bytes: write.bytes.clone(),
                    addr: Some(base),
                });
            }
            WriteKind::Patch => {
                let offset = rom_offset(machine, &write.target)?;
                machine
                    .patch(offset, &write.bytes)
                    .map_err(|e| Failure::new(convert::core_error(&e)))?;
                made.push(WriteMade {
                    kind: write.kind,
                    target: write.target.clone(),
                    bytes: write.bytes.clone(),
                    addr: None,
                });
            }
        }
    }
    Ok(made)
}

/// A patch target as a ROM file offset (design §7).
fn rom_offset(machine: &Machine, text: &str) -> Result<u32, Failure> {
    match addr::parse_write(text, machine.symbols()).map_err(|e| Failure::new(convert::addr(&e)))? {
        addr::WriteTarget::RomOffset(offset) => Ok(offset),
        addr::WriteTarget::Cpu(at) => {
            let bank = at
                .bank
                .unwrap_or_else(|| mem::bank_at(machine.core(), at.addr));
            match mem::banked_target(bank, at.addr)
                .map_err(|e| Failure::new(convert::core_error(&e)))?
            {
                mem::Target::Backing {
                    region: Region::Rom,
                    offset,
                } => Ok(offset as u32),
                _ => Err(Failure::new(ErrorBody::new(
                    ErrorCode::BadAddress,
                    format!("{text} isn't in ROM"),
                    "a patch targets ROM; use a poke for RAM or IO",
                ))),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::Fixture;
    use gbcli_core::BudgetKind;
    use gbcli_protocol::{EventsArgs, RunArgs, StepArgs};

    fn handler(f: &Fixture) -> Handler {
        Handler::new(f.bundle.clone(), Some("demo".into()), Some("tester".into()))
    }

    fn request(op: Op) -> Request {
        Request {
            id: "1".into(),
            session: None,
            actor: None,
            rev: None,
            op,
        }
    }

    fn run_frames(handler: &mut Handler, frames: u64) -> Response {
        handler.handle(&request(Op::Run(RunArgs {
            frames: Some(frames),
            ..RunArgs::default()
        })))
    }

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
    fn a_stale_revision_is_refused_and_names_the_writer() {
        let f = Fixture::new("rev");
        let mut h = handler(&f);
        let mut first = request(Op::Run(RunArgs {
            frames: Some(1),
            ..RunArgs::default()
        }));
        first.actor = Some("alice".into());
        assert!(h.handle(&first).ok);

        let mut stale = request(Op::Run(RunArgs {
            frames: Some(1),
            ..RunArgs::default()
        }));
        stale.rev = Some(Rev(0));
        let response = h.handle(&stale);
        let error = response.error.expect("stale rev");
        assert_eq!(error.code, ErrorCode::RevMismatch);
        assert!(error.remedy.contains("alice"), "{}", error.remedy);
    }

    #[test]
    fn an_unsupported_read_writes_the_gap_ledger_and_shows_in_the_event_log() {
        let f = Fixture::new("gaps");
        let mut h = handler(&f);
        let response = h.handle(&request(Op::Read(ReadArgs {
            reads: vec!["[$FEA0]".into()],
            since: None,
        })));
        let error = response.error.expect("FEA0 has no backing store");
        assert_eq!(error.code, ErrorCode::Unsupported);

        let gaps = f.bundle.gap_counts().unwrap();
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0].feature, "extra_oam");

        let store = f.bundle.session("demo").unwrap();
        let events = store.events(None).unwrap();
        let last = events.last().expect("one event");
        assert_eq!(last.gaps.get("extra_oam"), Some(&1));
    }

    #[test]
    fn the_event_log_records_every_request_with_both_revisions() {
        let f = Fixture::new("events");
        let mut h = handler(&f);
        assert!(run_frames(&mut h, 1).ok);
        let response = h.handle(&request(Op::Events(EventsArgs { limit: None })));
        let rows = response.result.unwrap().events.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].op, "run");
        assert_eq!(rows[0].rev_before, Rev(0));
        assert_eq!(rows[0].rev_after, Rev(1));
        assert_eq!(rows[0].actor.as_deref(), Some("tester"));
    }

    #[test]
    fn checkpoint_restore_and_undo_move_the_head_back_and_forth() {
        let f = Fixture::new("undo");
        let mut h = handler(&f);
        assert!(run_frames(&mut h, 2).ok);
        let saved = h.handle(&request(Op::Checkpoint(CheckpointArgs {
            name: "f2".into(),
        })));
        assert!(saved.ok);
        assert_eq!(
            saved.rev,
            Some(Rev(1)),
            "a checkpoint doesn't move the head"
        );
        assert!(run_frames(&mut h, 2).ok);

        let restored = h.handle(&request(Op::Restore(RestoreArgs {
            name: "f2".into(),
            allow_rom_change: false,
        })));
        let restored = restored.result.unwrap().restored.unwrap();
        assert_eq!(restored.name.as_deref(), Some("f2"));
        assert_eq!(restored.frame, Some(2));

        let undone = h.handle(&request(Op::Undo));
        let undone = undone.result.unwrap().restored.unwrap();
        assert_eq!(undone.name, None);
        assert_eq!(undone.frame, Some(4));
    }

    #[test]
    fn a_breakpoint_stops_the_run_where_it_is() {
        let f = Fixture::new("break");
        let mut h = handler(&f);
        assert!(
            h.handle(&request(Op::Break(BreakArgs {
                at: "$0170".into(),
                condition: None
            })))
            .ok
        );
        let response = h.handle(&request(Op::Run(RunArgs {
            frames: Some(10),
            ..RunArgs::default()
        })));
        let stop = response.result.unwrap().stop.unwrap();
        assert!(matches!(stop.reason, StopReason::Breakpoint { .. }));
        assert_eq!(stop.pc, 0x0170);
    }

    #[test]
    fn a_poke_is_backing_memory_and_the_next_read_sees_it() {
        let f = Fixture::new("poke");
        let mut h = handler(&f);
        let written = h.handle(&request(Op::Write(WriteArgs {
            writes: vec![WriteArg {
                kind: WriteKind::Poke,
                target: "$C002".into(),
                bytes: vec![2],
            }],
        })));
        assert!(written.ok, "{:?}", written.error);
        let read = h.handle(&request(Op::Read(ReadArgs {
            reads: vec!["[$C002]".into()],
            since: None,
        })));
        let reads = read.result.unwrap().reads.unwrap();
        assert_eq!(reads[0].value, 2);
    }

    #[test]
    fn show_reports_registers_and_the_last_stop() {
        let f = Fixture::new("show");
        let mut h = handler(&f);
        assert!(run_frames(&mut h, 1).ok);
        let shown = h.handle(&request(Op::Show));
        let result = shown.result.unwrap();
        assert!(result.registers.is_some());
        assert!(result.ppu.is_some());
        assert!(result.stop.is_some(), "the last stop is still there");
    }

    #[test]
    fn sessions_lists_the_row_and_removal_waits_for_named_checkpoints() {
        let f = Fixture::new("sessions");
        let mut h = handler(&f);
        let listed = h.handle(&request(Op::Sessions));
        let rows = listed.result.unwrap().sessions.unwrap();
        assert_eq!(rows[0].name, "demo");
        assert_eq!(rows[0].rev, Rev(0));

        assert!(
            h.handle(&request(Op::Checkpoint(CheckpointArgs {
                name: "keep".into()
            })))
            .ok
        );
        let refused = h.handle(&request(Op::Rm(RmArgs {
            name: "demo".into(),
            with_checkpoints: false,
        })));
        assert_eq!(refused.error.unwrap().code, ErrorCode::Refused);
        let removed = h.handle(&request(Op::Rm(RmArgs {
            name: "demo".into(),
            with_checkpoints: true,
        })));
        assert!(removed.ok);
        assert_eq!(removed.result.unwrap().removed.as_deref(), Some("demo"));
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
