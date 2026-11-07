//! Applying one request to one session (design §5, §11): lock, restore, act,
//! save the head atomically, log. The handler keeps the machine warm between
//! requests and reloads it when another writer moved the session's head.

mod bundle_ops;
mod config;
mod history;
mod reads;
mod run;
mod writes;

use gbcli_bundle::{Bundle, LOCK_WAIT, SessionStore, Start, next};
use gbcli_engine::{DebugError, LoadOptions, Machine, SaveMeta};
use gbcli_protocol::{
    ErrorBody, ErrorCode, EventLogLine, Op, Request, Response, ResultBody, Rev, SessionFile,
    StopRecord, StopSummary,
};

use crate::convert::{self, debug as debug_body};

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

fn summary(stop: &StopRecord) -> StopSummary {
    StopSummary {
        reason: stop.reason.clone(),
        pc: stop.pc,
        frame: stop.time.frame,
        ticks: stop.time.ticks,
    }
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

#[cfg(test)]
mod test_support;

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
}
