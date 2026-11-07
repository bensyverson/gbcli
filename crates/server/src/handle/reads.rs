use gbcli_bundle::SessionStore;
use gbcli_engine::cond::{Condition, EvalCtx, EvalWarning, ExprError, oam_entry};
use gbcli_engine::inspect::video::{ObjectAttributes, object_attributes};
use gbcli_engine::{Machine, Since};
use gbcli_protocol::{
    EventsArgs, OamEntryRead, PpuView, ReadArgs, ReadValue, RegistersView, ResultBody, SessionFile,
    TimeBase, TimeMark,
};

use super::{Failure, Handler, Need};
use crate::convert::{self};
impl Handler {
    pub(super) fn show(
        &mut self,
        store: &SessionStore,
        meta: &SessionFile,
        warnings: &mut Vec<String>,
    ) -> Result<ResultBody, Failure> {
        let (machine, loaded) = self.machine_for(store, meta, Need::Head, None)?;
        warnings.extend(loaded);
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
        warnings: &mut Vec<String>,
    ) -> Result<ResultBody, Failure> {
        let since = self.since(store, args.since.as_ref())?;
        let (machine, loaded) = self.machine_for(store, meta, Need::Head, None)?;
        warnings.extend(loaded);
        let compiled = compile_reads(&machine, &args.reads);
        let reads =
            compiled.and_then(|c| eval_reads(&machine, &c, &since, args.allow_uninit, warnings));
        self.keep_warm(store, meta, machine);
        let reads = reads?;
        Ok(ResultBody {
            reads: Some(reads),
            ..ResultBody::default()
        })
    }

    /// The time base a request's `--since` names (design §9).
    pub(super) fn since(
        &self,
        store: &SessionStore,
        base: Option<&TimeBase>,
    ) -> Result<Since, Failure> {
        match base {
            None | Some(TimeBase::Boot) => Ok(Since::Boot),
            Some(TimeBase::PowerOn) => Ok(Since::PowerOn),
            Some(TimeBase::Checkpoint(name)) => {
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

/// A `--read` expression, compiled.
pub(super) struct CompiledRead {
    text: String,
    form: ReadForm,
}

enum ReadForm {
    Value(Condition),
    /// `oam[N]`, printed as fields (design §7).
    OamEntry(u8),
}

/// Compiles every read against the machine's symbols, before it runs.
pub(super) fn compile_reads(
    machine: &Machine,
    reads: &[String],
) -> Result<Vec<CompiledRead>, Failure> {
    let bad = |e: ExprError| Failure::new(convert::expr(&e));
    reads
        .iter()
        .map(|text| {
            let form = match oam_entry(text).map_err(bad)? {
                Some(index) => ReadForm::OamEntry(index),
                None => ReadForm::Value(Condition::compile(text, machine.symbols()).map_err(bad)?),
            };
            Ok(CompiledRead {
                text: text.clone(),
                form,
            })
        })
        .collect()
}

fn oam_view(e: &ObjectAttributes) -> OamEntryRead {
    OamEntryRead {
        index: e.index,
        y: e.y,
        x: e.x,
        tile: e.tile,
        attr: e.attr,
        priority: e.priority,
        yflip: e.y_flip,
        xflip: e.x_flip,
        palette: e.palette,
        cgb_bank: e.cgb.map(|c| c.bank),
        cgb_palette: e.cgb.map(|c| c.palette),
    }
}

/// Evaluates compiled reads; `allow_uninit` reads a never-written byte's
/// power-on value with a warning instead of failing (design §8).
pub(super) fn eval_reads(
    machine: &Machine,
    reads: &[CompiledRead],
    since: &Since,
    allow_uninit: bool,
    warnings: &mut Vec<String>,
) -> Result<Vec<ReadValue>, Failure> {
    let base = machine.time_base(since);
    let mut values = Vec::new();
    for CompiledRead { text, form } in reads {
        let condition = match form {
            ReadForm::Value(condition) => condition,
            ReadForm::OamEntry(index) => {
                let entry = object_attributes(machine.core(), *index)
                    .map_err(|e| Failure::from_core(&e))?;
                values.push(ReadValue {
                    expr: text.clone(),
                    value: None,
                    oam: Some(oam_view(&entry)),
                });
                continue;
            }
        };
        let ctx = EvalCtx {
            core: machine.core(),
            symbols: machine.symbols(),
            access: None,
            base,
            allow_uninit,
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
            value: Some(value),
            oam: None,
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
            allow_uninit: false,
        })));
        let error = response.outcome.expect_err("FEA0 has no backing store");
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
        assert!(run_frames(&mut h, 1).is_ok());
        let shown = h.handle(&request(Op::Show));
        let result = shown.outcome.unwrap();
        assert!(result.registers.is_some());
        assert!(result.ppu.is_some());
        assert!(result.stop.is_some(), "the last stop is still there");
    }
}
