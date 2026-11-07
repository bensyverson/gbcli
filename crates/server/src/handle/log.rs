//! The event log line every session request leaves (design §10).

use gbcli_bundle::SessionStore;
use gbcli_protocol::{
    EventLogLine, Request, Response, ResultBody, Rev, SessionFile, StopRecord, StopSummary,
};

use super::{Failure, Handler};

impl Handler {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn log(
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
                if let Some(gap) = gap {
                    let _ = self.bundle.append_gap(
                        Some(store.name()),
                        request.op.name(),
                        gap.feature,
                        gap.reason,
                    );
                    if let Ok(counts) = self.bundle.session_gap_counts(store.name()) {
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
            op: request.op.clone(),
            allow_rom_change: request.allow_rom_change,
            ok: response.is_ok(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::{Fixture, V2, V2_SYM};
    use crate::handle::test_support::{handler, request, run_frames};
    use gbcli_protocol::{ErrorCode, Op, ReadArgs, RunArgs, WriteArg, WriteArgs, WriteKind};

    fn as_actor(actor: &str, op: Op, rev: Option<Rev>) -> Request {
        let mut r = request(op);
        r.actor = Some(actor.into());
        r.rev = rev;
        r
    }

    fn run_one() -> Op {
        Op::Run(RunArgs {
            frames: Some(1),
            ..RunArgs::default()
        })
    }

    #[test]
    fn a_stale_write_is_told_the_writer_not_the_last_reader() {
        let f = Fixture::new("log-writer");
        let mut h = handler(&f);
        assert!(h.handle(&as_actor("alice", run_one(), None)).is_ok());
        assert!(h.handle(&as_actor("bob", Op::Show, None)).is_ok());
        let stale = h.handle(&as_actor("carol", run_one(), Some(Rev(0))));
        let error = stale.outcome.expect_err("carol's rev is stale");
        assert_eq!(error.code, ErrorCode::RevMismatch);
        assert!(error.remedy.contains("alice"), "{}", error.remedy);
    }

    #[test]
    fn an_event_line_carries_the_ops_args_and_allow_rom_change() {
        let f = Fixture::with_sym("log-args");
        assert!(run_frames(&mut handler(&f), 1).is_ok());
        f.rebuild(V2, V2_SYM);
        let mut run = request(Op::Run(RunArgs {
            frames: Some(2),
            ..RunArgs::default()
        }));
        run.allow_rom_change = true;
        let forced = handler(&f).handle(&run);
        assert!(forced.is_ok(), "{:?}", forced.error());
        let rows = f.bundle.session("demo").unwrap().events(None).unwrap();
        let row = rows.last().unwrap();
        let Op::Run(args) = &row.op else {
            panic!("{row:?}")
        };
        assert_eq!(args.frames, Some(2));
        assert!(row.allow_rom_change, "--allow-rom-change isn't in the log");
    }

    #[test]
    fn an_unsupported_poke_lands_in_the_gap_ledger() {
        let f = Fixture::new("log-poke-gap");
        let poke = Op::Write(WriteArgs {
            writes: vec![WriteArg {
                kind: WriteKind::Poke,
                target: "$FEA0".into(),
                bytes: vec![1],
            }],
        });
        let response = handler(&f).handle(&request(poke));
        let error = response.outcome.expect_err("FEA0 has no backing store");
        assert_eq!(error.code, ErrorCode::Unsupported);
        let gaps = f.bundle.gap_counts().unwrap();
        assert_eq!(gaps.len(), 1, "{gaps:?}");
        assert_eq!(gaps[0].feature, "extra_oam");
    }

    #[test]
    fn an_events_gap_counts_are_this_sessions() {
        let f = Fixture::new("log-gap-session");
        f.bundle
            .session("other")
            .unwrap()
            .create(&gbcli_bundle::RomSource {
                rom: f.dir.join("demo.gb"),
                sym: None,
                model: Some(gbcli_core::Model::DmgB),
                seed: Some(0),
            })
            .unwrap();
        let read = || {
            Op::Read(ReadArgs {
                reads: vec!["[$FEA0]".into()],
                since: None,
                allow_uninit: false,
            })
        };
        let mut other = request(read());
        other.session = Some("other".into());
        assert!(!handler(&f).handle(&other).is_ok());
        assert!(!handler(&f).handle(&request(read())).is_ok());
        let rows = f.bundle.session("demo").unwrap().events(None).unwrap();
        assert_eq!(rows.last().unwrap().gaps.get("extra_oam"), Some(&1));
    }
}
