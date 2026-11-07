use gbcli_bundle::{SessionStore, Start};
use gbcli_engine::{Machine, Since, cond::Condition, cond::EvalCtx, cond::EvalWarning};
use gbcli_protocol::{
    EventsArgs, PpuView, ReadArgs, ReadValue, RegistersView, ResultBody, SessionFile, TimeMark,
};

use super::{Failure, Handler, Need};
use crate::convert::{self};
impl Handler {
    pub(super) fn show(
        &mut self,
        store: &SessionStore,
        meta: &SessionFile,
    ) -> Result<ResultBody, Failure> {
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

    pub(super) fn events(
        &self,
        store: &SessionStore,
        args: &EventsArgs,
    ) -> Result<ResultBody, Failure> {
        let rows = store
            .events(args.limit)
            .map_err(|e| Failure::from_store(&e))?;
        Ok(ResultBody {
            events: Some(rows),
            ..ResultBody::default()
        })
    }

    pub(super) fn read(
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

    /// The time base a request's `--since` names (design §9).
    pub(super) fn since(&self, store: &SessionStore, text: Option<&str>) -> Result<Since, Failure> {
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
}

pub(super) fn warning_text(warning: &EvalWarning) -> String {
    warning.to_string()
}

pub(super) fn read_values(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::Fixture;
    use crate::handle::test_support::{handler, request, run_frames};
    use gbcli_protocol::{ErrorCode, Op};

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
}
