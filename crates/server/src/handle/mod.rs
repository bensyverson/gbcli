//! Applying one request to one session (design §5, §11): lock, restore, act,
//! save the head atomically, log. The handler keeps the machine warm between
//! requests and reloads it when another writer moved the session's head.

mod assets;
mod battery;
mod build;
mod bundle_ops;
mod config;
mod failure;
mod head;
mod history;
mod listing;
mod log;
mod opcode;
mod reads;
mod run;
mod shot;
mod version;
mod view;
mod writes;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use gbcli_bundle::{Bundle, FilesStamp, LOCK_WAIT, SessionStore, Start};
use gbcli_engine::{LoadOptions, Machine};
use gbcli_protocol::{ErrorBody, ErrorCode, Op, Request, Response, ResultBody, Rev, SessionFile};

use crate::convert;
use crate::watchdog::{WALL_GUARD, Watchdog};
pub use build::{Host, Tooling};
use failure::Failure;
use head::{HeadChange, save_head};

/// One server side: a project's bundle, a default session and actor, and the
/// machine kept warm between requests.
pub struct Handler {
    bundle: Bundle,
    default_session: Option<String>,
    default_actor: Option<String>,
    warm: Option<Warm>,
    /// Where the current request's machine came from, for [`Handler::keep_warm`].
    loaded: Option<Loaded>,
    /// How the current request loads a stored state: its `allow_rom_change`.
    load: LoadOptions,
    watchdog: Watchdog,
    wall_guard: Duration,
    /// The current request's interrupt flag, which the watchdog sets when
    /// the request outlives `wall_guard`; runs poll it.
    interrupt: Arc<AtomicBool>,
    /// What the core said it can't do during the current request, for the
    /// gap ledger whether the request succeeds or fails (a run's raster
    /// timeline names them without failing).
    gaps_found: Vec<gbcli_core::Gap>,
    /// Where `build`, `doctor` and `new` resolve paths and find RGBDS.
    host: Host,
}

/// The machine left by the last request, and which head it is: a session
/// removed and created again has the same name and may reach the same rev,
/// but never the same incarnation. It is reused only while the ROM and `.sym`
/// are the files it was built from, and only when it is exactly what a cold
/// load of that head, with no flags, would build (design §1).
struct Warm {
    session: String,
    incarnation: u64,
    rev: Rev,
    files: FilesStamp,
    machine: Machine,
}

/// How the current request's machine was obtained.
struct Loaded {
    files: FilesStamp,
    /// The head's rev when it was loaded: a request that saved has moved it.
    rev: Rev,
    /// Whether the machine is the head as a flagless cold load builds it, with
    /// nothing to report. A load that warned (a compatible rebuild, another
    /// core commit) or that `--allow-rom-change` forced is not: reusing it
    /// would hide the warning, or carry the flag into requests without it.
    exact: bool,
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
            loaded: None,
            load: LoadOptions::default(),
            watchdog: Watchdog::new(),
            wall_guard: WALL_GUARD,
            interrupt: Arc::new(AtomicBool::new(false)),
            gaps_found: Vec::new(),
            host: Host::from_process(),
        }
    }

    /// Sets where `build`, `doctor` and `new` resolve paths and find RGBDS;
    /// the default is this process's working directory and `PATH`.
    pub fn set_host(&mut self, host: Host) {
        self.host = host;
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
        if let Some(response) = self.toolchain_op(request) {
            return response;
        }
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
                "pass --session NAME or set GBCLI_SESSION (a request line names it in \"session\")",
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

        // A change may name the revision it read; a mismatch names the writer.
        if request.op.changes_session()
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

        self.load = LoadOptions {
            allow_rom_change: request.allow_rom_change,
            ..LoadOptions::default()
        };
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
            Op::Show => self.show(store, meta, warnings),
            Op::Read(args) => self.read(store, meta, args, warnings),
            Op::Break(args) => self.breakpoint(store, meta, args, warnings),
            Op::Watch(args) => self.watch(store, meta, args, warnings),
            Op::Delete(args) => self.delete(store, meta, args, warnings),
            Op::Checkpoint(args) => self.checkpoint(store, meta, args, actor, warnings),
            Op::Restore(args) => self.restore(store, meta, args, actor, warnings),
            Op::Undo => self.undo(store, meta, actor, warnings),
            Op::Write(args) => self.write(store, meta, args, actor, warnings),
            Op::Events(args) => self.events(store, args),
            Op::Shot(args) => self.shot(store, meta, args, warnings),
            Op::View(args) => self.view(store, meta, args, warnings),
            Op::Sym(args) => self.sym(store, meta, args, warnings),
            Op::Checkpoints => self.checkpoints(store, meta),
            Op::Measure(args) => self.measure(store, meta, args, warnings),
            Op::Equiv(args) => self.equiv(store, meta, args, warnings),
            Op::PowerCycle(args) => self.power_cycle(store, meta, args, actor, warnings),
            Op::Sav(args) => self.sav(store, meta, args, warnings),
            Op::Sessions
            | Op::Gaps
            | Op::Prune(_)
            | Op::Rm(_)
            | Op::Build(_)
            | Op::Doctor
            | Op::New(_)
            | Op::Assets(_)
            | Op::Opcode(_)
            | Op::Version => Err(Failure::new(ErrorBody::new(
                ErrorCode::BadRequest,
                "that op doesn't name a session",
                ErrorCode::BadRequest.default_remedy(),
            ))),
        }
    }

    /// The machine a request starts from: warm when it is this session's
    /// head over unchanged files, else rebuilt. A stored state (the head or
    /// `at`) loads with the request's options: `allow_rom_change` reaches here.
    fn machine_for(
        &mut self,
        store: &SessionStore,
        meta: &SessionFile,
        need: Need,
        at: Option<&str>,
    ) -> Result<(Machine, Vec<String>), Failure> {
        let load = &self.load;
        self.loaded = None;
        let cold = matches!(need, Need::Start(Start::PowerOn));
        let warm = self.warm.take().filter(|w| {
            at.is_none()
                && !cold
                && meta.rev.0 > 0
                && w.session == store.name()
                && w.incarnation == meta.incarnation
                && w.rev == meta.rev
        });
        let warm = warm.and_then(|mut w| w.files.still_current().then_some(w));
        let (mut machine, warnings, files) = match (warm, need) {
            (Some(warm), _) => (warm.machine, Vec::new(), warm.files),
            (None, need) => {
                // Stamped before the files are read: a rebuild in between
                // reads as a change next time.
                let files = store.files_stamp(meta);
                let (machine, warnings) = match need {
                    Need::Start(from) => {
                        store.start_machine(meta, at, from, load, Some(&self.interrupt))
                    }
                    Need::Head => store.head_or_power_on(meta, load),
                    Need::Replace => store.machine(meta).map(|m| (m, Vec::new())),
                }
                .map_err(|e| Failure::from_store(&e))?;
                (machine, warnings, files)
            }
        };
        self.loaded = Some(Loaded {
            files,
            rev: meta.rev,
            exact: warnings.is_empty(),
        });
        *machine.debugger_config_mut() = meta.debugger.clone();
        // On every request, warm or cold: the ROM is the same file either way.
        let mut warnings = warnings;
        warnings.extend(battery::header_warnings(&machine));
        Ok((machine, warnings))
    }

    /// Keeps `machine` for the next request when it is the session's head as
    /// a flagless cold load would build it: the request saved it as the head,
    /// or loaded the head exactly.
    fn keep_warm(&mut self, store: &SessionStore, meta: &SessionFile, machine: Machine) {
        let Some(loaded) = self.loaded.take() else {
            return;
        };
        if !(loaded.exact || meta.rev != loaded.rev) {
            return;
        }
        self.warm = Some(Warm {
            session: store.name().to_string(),
            incarnation: meta.incarnation,
            rev: meta.rev,
            files: loaded.files,
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
mod battery_tests;

#[cfg(test)]
mod flash_tests;

#[cfg(test)]
mod state_tests;

#[cfg(test)]
mod warm_tests;

#[cfg(test)]
mod review_tests;

#[cfg(test)]
mod rom_change_tests;

#[cfg(test)]
mod shot_tests;

#[cfg(test)]
mod audio_tests;

#[cfg(test)]
mod filmstrip_tests;

#[cfg(test)]
mod keys_tests;

#[cfg(test)]
mod view_tests;

#[cfg(test)]
mod equiv_tests;
#[cfg(test)]
mod listing_tests;
#[cfg(test)]
mod measure_tests;
#[cfg(test)]
mod oam_read_tests;
#[cfg(test)]
mod profile_tests;
#[cfg(test)]
mod raster_tests;

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
