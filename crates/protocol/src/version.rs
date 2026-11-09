//! `version`: which gbcli answers and which wire protocol it speaks, so a
//! client that drives `gbcli serve` can check it before it relies on it,
//! without parsing CLI output. Needs no session, ROM or toolchain.

use serde::{Deserialize, Serialize};

/// The protocol this gbcli speaks. Bumped by one on every breaking wire
/// change, including a new value of an enum a response carries, since the
/// enums are closed; a new op or optional request field leaves it. The
/// full rule is design §5's update of 2026-11-07 (leaf sUOJQb). A client
/// checks it for equality with the version it was built against. 2 since
/// leaf nin31Q: the `condition_error` stop reason, and `show`'s `ppu.mode`
/// null where the core can't know it (design §5's update of 2026-11-09).
pub const PROTOCOL_VERSION: u32 = 2;

/// What `version` answers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionMade {
    /// gbcli's own version, as Cargo has it (`0.1.0`).
    pub gbcli: String,
    /// [`PROTOCOL_VERSION`].
    pub protocol: u32,
    /// The vendored SameBoy commit, in full.
    pub sameboy: String,
    /// The hash of gbcli's patch set over that commit, in full.
    pub patch_set: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Op, ResultBody, parse_request};

    #[test]
    fn a_version_request_is_a_unit_op_that_needs_nothing() {
        let request = parse_request(r#"{"id":"1","op":"version"}"#).expect("parses");
        assert_eq!(request.op, Op::Version);
        assert_eq!(Op::Version.name(), "version");
        assert!(!Op::Version.needs_bundle());
        assert!(!Op::Version.changes_session() && !Op::Version.mutates_head());
        assert!(parse_request(r#"{"id":"1","op":"version","args":{"x":1}}"#).is_err());
    }

    #[test]
    fn the_result_carries_the_protocol_version_as_a_number() {
        let result = ResultBody {
            version: Some(VersionMade {
                gbcli: "0.1.0".into(),
                protocol: PROTOCOL_VERSION,
                sameboy: "abc".into(),
                patch_set: "def".into(),
            }),
            ..ResultBody::default()
        };
        assert_eq!(
            serde_json::to_string(&result).unwrap(),
            r#"{"version":{"gbcli":"0.1.0","protocol":2,"sameboy":"abc","patch_set":"def"}}"#
        );
    }
}
