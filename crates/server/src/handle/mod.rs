//! Applying one request to one session (design §5, §11): lock, restore, act,
//! save the head atomically, log. The handler keeps the machine warm between
//! requests and reloads it when another writer moved the session's head.

mod bundle_ops;
mod config;
mod head;
mod history;
mod log;
mod reads;
mod run;
mod writes;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use gbcli_bundle::{Bundle, LOCK_WAIT, SessionStore, Start};
use gbcli_engine::{DebugError, LoadOptions, Machine};
use gbcli_protocol::{
    ErrorBody, ErrorCode, Op, Request, Response, ResultBody, Rev, SessionFile, StopRecord,
};

use crate::convert::{self, Gap, debug as debug_body};
use crate::watchdog::{WALL_GUARD, Watchdog};
use head::{HeadChange, save_head};

/// One server side: a project's bundle, a default session and actor, and the
/// machine kept warm between requests.
pub struct Handler {
    bundle: Bundle,
    default_session: Option<String>,
    default_actor: Option<String>,
    warm: Option<Warm>,
    watchdog: Watchdog,
    wall_guard: Duration,
    /// The current request's interrupt flag, which the watchdog sets when
    /// the request outlives `wall_guard`; runs poll it.
    interrupt: Arc<AtomicBool>,
}

/// The machine left by the last request, and which head it is: a session
/// removed and created again has the same name and may reach the same rev,
/// but never the same incarnation.
struct Warm {
    session: String,
    incarnation: u64,
    rev: Rev,
    machine: Machine,
}

/// A failed request: its body, and the gap-ledger entry it deserves.
struct Failure {
    /// Boxed: a stop record makes an `ErrorBody` large, and every `Result`
    /// here pays for its error type on the stack.
    body: Box<ErrorBody>,
    gap: Option<Gap>,
}

impl Failure {
    fn new(body: ErrorBody) -> Self {
        Failure {
            body: Box::new(body),
            gap: None,
        }
    }

    /// The same failure, carrying the stop record of the run it followed.
    fn with_stop(mut self, stop: StopRecord) -> Self {
        self.body = Box::new((*self.body).with_stop(stop));
        self
    }

    /// A core failure, with its gap-ledger entry when it is `Unsupported`.
    fn from_core(error: &gbcli_core::CoreError) -> Self {
        Failure {
            body: Box::new(convert::core_error(error)),
            gap: convert::gap_of_core(error),
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
    /// Inspect or configure: the head, or power-on when there is none; never
    /// the boot run.
    Head,
    /// Advance or change the state: from `at`, else the head, else the boot
    /// handoff, or power-on for `--from power-on`. Where `run` would start.
    Start(Start),
    /// The request replaces the whole state (`restore`, `undo`): any machine
    /// over the session's ROM. The head isn't loaded, so a head the ROM
    /// rebuild made unloadable doesn't block the restore that fixes it.
    Replace,
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
            watchdog: Watchdog::new(),
            wall_guard: WALL_GUARD,
            interrupt: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Sets the wall-clock guard (design §3) for the requests that follow.
    pub fn set_wall_guard(&mut self, after: Duration) {
        self.wall_guard = after;
    }

    pub fn bundle(&self) -> &Bundle {
        &self.bundle
    }

    /// Answers one request. Every request takes the session's lock, so two
    /// processes alternate on one head (design §1).
    pub fn handle(&mut self, request: &Request) -> Response {
        // Armed for every request; only emulation polls the flag.
        let armed = self.watchdog.arm(self.wall_guard);
        self.interrupt = armed.flag().clone();
        let id = request.id.clone();
        match &request.op {
            Op::Sessions => self.list_sessions(id),
            Op::Gaps => self.gaps(id),
            Op::Prune(args) => self.prune(id, args),
            Op::Rm(args) => self.rm(id, args),
            _ => self.session_request(id, request),
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
            let by = store.last_writer(meta.rev).ok().flatten();
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
            Op::Step(args) => self.step(store, meta, args, actor, warnings),
            Op::Show => self.show(store, meta),
            Op::Read(args) => self.read(store, meta, args),
            Op::Break(args) => self.breakpoint(store, meta, args),
            Op::Watch(args) => self.watch(store, meta, args),
            Op::Delete(args) => self.delete(store, meta, args),
            Op::Checkpoint(args) => self.checkpoint(store, meta, args, actor, warnings),
            Op::Restore(args) => self.restore(store, meta, args, actor, warnings),
            Op::Undo => self.undo(store, meta, actor, warnings),
            Op::Write(args) => self.write(store, meta, args, actor, warnings),
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

    /// The machine a request starts from: warm when it is this session's
    /// head, else rebuilt. `load` governs loading a stored state (the head or
    /// `at`): `--allow-rom-change` reaches here.
    fn machine_for(
        &mut self,
        store: &SessionStore,
        meta: &SessionFile,
        need: Need,
        at: Option<&str>,
        load: &LoadOptions,
    ) -> Result<(Machine, Vec<String>), Failure> {
        let cold = matches!(need, Need::Start(Start::PowerOn));
        let warm = self.warm.take().filter(|w| {
            at.is_none()
                && !cold
                && meta.rev.0 > 0
                && w.session == store.name()
                && w.incarnation == meta.incarnation
                && w.rev == meta.rev
        });
        let (mut machine, warnings) = match (warm, need) {
            (Some(warm), _) => Ok((warm.machine, Vec::new())),
            (None, Need::Start(from)) => {
                store.start_machine(meta, at, from, load, Some(&self.interrupt))
            }
            (None, Need::Head) => store.head_or_power_on(meta, load),
            (None, Need::Replace) => store.machine(meta).map(|m| (m, Vec::new())),
        }
        .map_err(|e| Failure::from_store(&e))?;
        *machine.debugger_config_mut() = meta.debugger.clone();
        Ok((machine, warnings))
    }

    fn keep_warm(&mut self, store: &SessionStore, meta: &SessionFile, machine: Machine) {
        self.warm = Some(Warm {
            session: store.name().to_string(),
            incarnation: meta.incarnation,
            rev: meta.rev,
            machine,
        });
    }

    /// Forgets the warm machine, whichever session it belongs to.
    pub(crate) fn forget_all_warm(&mut self) {
        self.warm = None;
    }

    /// Forgets the warm machine if it belongs to `session`.
    fn forget_warm(&mut self, session: &str) {
        if self.warm.as_ref().is_some_and(|w| w.session == session) {
            self.warm = None;
        }
    }
}

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod state_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::Fixture;
    use crate::handle::test_support::{handler, request, run_frames};
    use gbcli_protocol::{EventsArgs, RunArgs};

    #[test]
    fn a_stale_revision_is_refused_and_names_the_writer() {
        let f = Fixture::new("rev");
        let mut h = handler(&f);
        let mut first = request(Op::Run(RunArgs {
            frames: Some(1),
            ..RunArgs::default()
        }));
        first.actor = Some("alice".into());
        assert!(h.handle(&first).is_ok());

        let mut stale = request(Op::Run(RunArgs {
            frames: Some(1),
            ..RunArgs::default()
        }));
        stale.rev = Some(Rev(0));
        let response = h.handle(&stale);
        let error = response.outcome.expect_err("stale rev");
        assert_eq!(error.code, ErrorCode::RevMismatch);
        assert!(error.remedy.contains("alice"), "{}", error.remedy);
    }

    #[test]
    fn the_event_log_records_every_request_with_both_revisions() {
        let f = Fixture::new("events");
        let mut h = handler(&f);
        assert!(run_frames(&mut h, 1).is_ok());
        let response = h.handle(&request(Op::Events(EventsArgs { limit: None })));
        let rows = response.outcome.unwrap().events.unwrap();
        assert_eq!(rows.len(), 1);
        assert!(matches!(rows[0].op, Op::Run(_)), "{:?}", rows[0].op);
        assert_eq!(rows[0].rev_before, Rev(0));
        assert_eq!(rows[0].rev_after, Rev(1));
        assert_eq!(rows[0].actor.as_deref(), Some("tester"));
    }
}
