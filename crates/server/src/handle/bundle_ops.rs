use gbcli_bundle::LOCK_WAIT;
use gbcli_protocol::{PruneArgs, Response, ResultBody, RmArgs};

use super::Handler;
use crate::convert::{self};
impl Handler {
    pub(super) fn list_sessions(&self, id: String) -> Response {
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

    pub(super) fn gaps(&self, id: String) -> Response {
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

    pub(super) fn prune(&self, id: String, args: &PruneArgs) -> Response {
        let unused_for = match gbcli_bundle::parse_duration(&args.unused_for) {
            Ok(d) => d,
            Err(e) => return Response::failed(id, None, convert::store(&e)),
        };
        match self.bundle.prune(unused_for, LOCK_WAIT) {
            Ok(report) => Response::ok(
                id,
                None,
                ResultBody {
                    pruned: Some(report.removed),
                    kept: Some(report.kept),
                    busy: Some(report.busy),
                    ..ResultBody::default()
                },
            ),
            Err(e) => Response::failed(id, None, convert::store(&e)),
        }
    }

    pub(super) fn rm(&mut self, id: String, args: &RmArgs) -> Response {
        let store = match self.bundle.session(&args.name) {
            Ok(store) => store,
            Err(e) => return Response::failed(id, None, convert::store(&e)),
        };
        self.forget_warm(&args.name);
        match store.remove(args.with_checkpoints, LOCK_WAIT) {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::Fixture;
    use crate::handle::test_support::{handler, request};
    use gbcli_protocol::{CheckpointArgs, ErrorCode, Op, Rev};

    #[test]
    fn a_session_row_counts_its_head_frame_since_boot_as_stop_records_do() {
        let f = Fixture::new("sessions-frame");
        let mut h = handler(&f);
        let ran = crate::handle::test_support::run_frames(&mut h, 3);
        let stop = ran.outcome.unwrap().stop.unwrap();
        let listed = h.handle(&request(Op::Sessions));
        let row = &listed.outcome.unwrap().sessions.unwrap()[0];
        assert_eq!(row.head_frame, Some(stop.time.frame));
        assert_eq!(row.head_frame, Some(3));
        assert_eq!(row.head_since, Some(gbcli_protocol::TimeBase::Boot));
    }

    #[test]
    fn sessions_lists_the_row_and_removal_waits_for_named_checkpoints() {
        let f = Fixture::new("sessions");
        let mut h = handler(&f);
        let listed = h.handle(&request(Op::Sessions));
        let rows = listed.outcome.unwrap().sessions.unwrap();
        assert_eq!(rows[0].name, "demo");
        assert_eq!(rows[0].rev, Rev(0));

        assert!(
            h.handle(&request(Op::Checkpoint(CheckpointArgs {
                name: "keep".into()
            })))
            .is_ok()
        );
        let refused = h.handle(&request(Op::Rm(RmArgs {
            name: "demo".into(),
            with_checkpoints: false,
        })));
        assert_eq!(refused.outcome.unwrap_err().code, ErrorCode::Refused);
        let removed = h.handle(&request(Op::Rm(RmArgs {
            name: "demo".into(),
            with_checkpoints: true,
        })));
        assert!(removed.is_ok());
        assert_eq!(removed.outcome.unwrap().removed.as_deref(), Some("demo"));
    }

    #[test]
    fn prune_reports_the_sessions_it_kept_for_their_checkpoints() {
        let f = Fixture::new("prune-kept");
        let mut h = handler(&f);
        let made = h.handle(&request(Op::Checkpoint(CheckpointArgs {
            name: "keep".into(),
        })));
        assert!(made.is_ok());
        // Last use is the lock file's mtime (design §1): date it a day back.
        let lock = f.bundle.root().join("locks").join("demo.lock");
        // Now is the bundle's clock, as the prune's cut reads it.
        let two_days_ago = f.bundle.now() - 2 * 86_400;
        let day_ago = std::time::UNIX_EPOCH + std::time::Duration::from_secs(two_days_ago);
        std::fs::File::options()
            .write(true)
            .open(&lock)
            .and_then(|file| file.set_modified(day_ago))
            .expect("backdate the lock");
        let pruned = h.handle(&request(Op::Prune(PruneArgs {
            unused_for: "1d".into(),
        })));
        let result = pruned.outcome.unwrap();
        assert_eq!(result.pruned, Some(Vec::new()));
        assert_eq!(result.kept, Some(vec!["demo".to_string()]));
    }
}
