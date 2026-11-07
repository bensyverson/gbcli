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

    pub(super) fn rm(&self, id: String, args: &RmArgs) -> Response {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::Fixture;
    use crate::handle::test_support::{handler, request};
    use gbcli_protocol::{CheckpointArgs, ErrorCode, Op, Rev};

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
}
