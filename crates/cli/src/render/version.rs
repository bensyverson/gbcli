//! `version`'s line. It isn't a `key=value` row: it is the line `gbcli
//! --version` printed before the op existed, which scripts parse (LoadHigh's
//! doctor did), so the protocol version is `--json`'s to report.

use gbcli_protocol::ResultBody;

/// How much of the commit and patch-set hashes the line shows.
const COMMIT_SHOWN: usize = 7;
const PATCH_SET_SHOWN: usize = 8;

pub fn rows(r: &ResultBody) -> String {
    let Some(v) = &r.version else {
        return String::new();
    };
    format!(
        "gbcli {} (SameBoy {}+patches.{})\n",
        v.gbcli,
        prefix(&v.sameboy, COMMIT_SHOWN),
        prefix(&v.patch_set, PATCH_SET_SHOWN),
    )
}

fn prefix(hash: &str, n: usize) -> &str {
    hash.get(..n).unwrap_or(hash)
}
