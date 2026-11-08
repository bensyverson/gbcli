//! `version`: the `version` op, gbcli's version and the protocol's (a help
//! verb).

use super::flags::JSON;
use super::{Arity, Class, FlagSpec, Verb, VerbSpec};

/// `--json` alone: the answer needs no session, so no `--dir` or `--as`.
const JSON_ONLY: &[FlagSpec] = &[JSON];

pub const VERSION_VERB: VerbSpec = VerbSpec {
    verb: Verb::Version,
    name: "version",
    class: Class::Help,
    summary: "gbcli's, the protocol's and the core's versions",
    usage: "[--json]",
    arity: Arity {
        min: 0,
        max: Some(0),
        name: "",
    },
    flags: &[],
    groups: &[JSON_ONLY],
    about: "Prints gbcli's version and the SameBoy commit and patch set it was built with, \
            the line `gbcli --version` prints. --json adds the protocol version: \
            result.version is {gbcli, protocol, sameboy, patch_set}, the `version` op's \
            answer, which a program driving `gbcli serve` reads by sending \
            {\"op\":\"version\"} (`gbcli schema`).",
    example: (
        "gbcli version --json",
        "prints the versions, the protocol's among them, as the response line",
    ),
};
