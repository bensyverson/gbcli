use gbcli_bundle::SessionStore;
use gbcli_protocol::{
    Arming, BreakArgs, BreakpointMade, DebuggerConfig, DeleteArgs, Deleted, ErrorBody, ErrorCode,
    Location, ResultBody, SessionFile, WatchAccess, WatchArgs, WatchMade,
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
        let (mut machine, loaded) = self.machine_for(store, meta, Need::Rom, None)?;
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
        Ok(ResultBody {
            breakpoint: Some(BreakpointMade {
                id,
                at: made,
                condition: args.condition.clone(),
            }),
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
        let (mut machine, loaded) = self.machine_for(store, meta, Need::Rom, None)?;
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
        Ok(ResultBody {
            watch: Some(WatchMade {
                id,
                access: args.access,
                at: made,
                condition: args.condition.clone(),
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
        let (mut machine, loaded) = self.machine_for(store, meta, Need::Rom, None)?;
        warnings.extend(loaded);
        let found = machine.delete(args.id);
        if !found {
            let remedy = existing_ids(machine.debugger_config());
            return Err(Failure::new(ErrorBody::new(
                ErrorCode::BadRequest,
                format!("there is no breakpoint or watch #{}", args.id),
                remedy,
            )));
        }
        meta.debugger = machine.debugger_config().clone();
        store
            .write_meta(meta)
            .map_err(|e| Failure::from_store(&e))?;
        Ok(ResultBody {
            deleted: Some(Deleted { id: args.id, found }),
            ..ResultBody::default()
        })
    }
}

/// The ids `delete` could take, as a remedy, each disarmed one said so.
fn existing_ids(config: &DebuggerConfig) -> String {
    let place = |at: &Location, arming: Arming| {
        let place = at.symbol.clone().unwrap_or_else(|| match at.bank {
            Some(bank) => format!("{bank:02X}:{:04X}", at.addr),
            None => format!("${:04X}", at.addr),
        });
        match arming {
            Arming::Armed => place,
            Arming::Disarmed => format!("{place} (disarmed)"),
        }
    };
    let mut ids: Vec<(u32, String)> = config
        .breakpoints
        .iter()
        .map(|b| (b.id, format!("#{} break {}", b.id, place(&b.at, b.arming))))
        .chain(config.watches.iter().map(|w| {
            let access = match w.access {
                WatchAccess::Read => "read",
                WatchAccess::Write => "write",
                WatchAccess::Access => "access",
            };
            (
                w.id,
                format!("#{} watch {access} {}", w.id, place(&w.at, w.arming)),
            )
        }))
        .collect();
    if ids.is_empty() {
        return "none are set; `gbcli break <addr>` or `gbcli watch write <addr>` makes one".into();
    }
    ids.sort();
    let listed: Vec<String> = ids.into_iter().map(|(_, text)| text).collect();
    format!("delete one of: {}", listed.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::Fixture;
    use crate::handle::test_support::{handler, request};
    use gbcli_protocol::{Op, RunArgs, StopReason};

    #[test]
    fn deleting_an_id_that_doesnt_exist_is_refused_listing_the_ids_that_do() {
        let f = Fixture::new("delete-missing");
        let mut h = handler(&f);
        let none = h.handle(&request(Op::Delete(DeleteArgs { id: 7 })));
        let error = none.outcome.expect_err("nothing to delete");
        assert_eq!(error.code, gbcli_protocol::ErrorCode::BadRequest);
        assert!(error.message.contains("#7"), "{}", error.message);
        assert!(error.remedy.contains("none are set"), "{}", error.remedy);

        let made = h.handle(&request(Op::Break(BreakArgs {
            at: "$0170".into(),
            condition: None,
        })));
        let id = made.outcome.unwrap().breakpoint.unwrap().id;
        let missing = h.handle(&request(Op::Delete(DeleteArgs { id: id + 5 })));
        let error = missing.outcome.expect_err("no such id");
        assert!(error.remedy.contains(&format!("#{id}")), "{}", error.remedy);
        assert!(error.remedy.contains("$0170"), "{}", error.remedy);
        let meta = f.bundle.session("demo").unwrap().read_meta().unwrap();
        assert_eq!(meta.debugger.breakpoints.len(), 1, "nothing deleted");

        let deleted = h.handle(&request(Op::Delete(DeleteArgs { id })));
        assert!(deleted.outcome.unwrap().deleted.unwrap().found);
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
