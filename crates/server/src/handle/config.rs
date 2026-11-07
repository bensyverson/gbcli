use gbcli_bundle::SessionStore;
use gbcli_protocol::{
    BreakArgs, BreakpointMade, DeleteArgs, Deleted, ResultBody, SessionFile, WatchArgs, WatchMade,
};

use super::writes::parse_location;
use super::{Failure, Handler, Need};
use crate::convert;
impl Handler {
    pub(super) fn breakpoint(
        &mut self,
        store: &SessionStore,
        meta: &mut SessionFile,
        args: &BreakArgs,
        warnings: &mut Vec<String>,
    ) -> Result<ResultBody, Failure> {
        let (mut machine, loaded) = self.machine_for(store, meta, Need::Head, None)?;
        warnings.extend(loaded);
        let at = parse_location(&machine, &args.at)?;
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

    pub(super) fn watch(
        &mut self,
        store: &SessionStore,
        meta: &mut SessionFile,
        args: &WatchArgs,
        warnings: &mut Vec<String>,
    ) -> Result<ResultBody, Failure> {
        let (mut machine, loaded) = self.machine_for(store, meta, Need::Head, None)?;
        warnings.extend(loaded);
        let at = parse_location(&machine, &args.at)?;
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

    pub(super) fn delete(
        &mut self,
        store: &SessionStore,
        meta: &mut SessionFile,
        args: &DeleteArgs,
        warnings: &mut Vec<String>,
    ) -> Result<ResultBody, Failure> {
        let (mut machine, loaded) = self.machine_for(store, meta, Need::Head, None)?;
        warnings.extend(loaded);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::Fixture;
    use crate::handle::test_support::{handler, request};
    use gbcli_protocol::{Op, RunArgs, StopReason};

    #[test]
    fn a_breakpoint_stops_the_run_where_it_is() {
        let f = Fixture::new("break");
        let mut h = handler(&f);
        assert!(
            h.handle(&request(Op::Break(BreakArgs {
                at: "$0170".into(),
                condition: None
            })))
            .is_ok()
        );
        let response = h.handle(&request(Op::Run(RunArgs {
            frames: Some(10),
            ..RunArgs::default()
        })));
        let stop = response.outcome.unwrap().stop.unwrap();
        assert!(matches!(stop.reason, StopReason::Breakpoint { .. }));
        assert_eq!(stop.pc, 0x0170);
    }
}
