//! The equivalence check through the binary, as a golden transcript: the
//! same build, a patched build that diverges (exit 1, both sides saved), and
//! restoring side B.

use crate::common::{Project, golden};

#[test]
fn equiv_prints_the_verdict_and_saves_both_sides() {
    let project = Project::new("equiv");
    let mut out = String::new();
    for args in [
        &["run", "demo.gb", "--frames", "2"][..],
        &["equiv", "demo.gb", "--frames", "30", "--ram", "wFrame,2"],
        &[
            "equiv", "--frames", "30", "--patch", "$016B=0", "--ram", "wFrame,2",
        ],
        &["equiv", "--frames", "3", "--patch", "$016B=0", "--json"],
        &["restore", "equiv-b"],
        &["read", "[wFrame]"],
        &["equiv", "--frames", "3", "--ram", "wNowhere"],
        &["equiv", "--ram", "wFrame"],
    ] {
        let mut args = args.to_vec();
        args.extend(["--session", "demo"]);
        out += &project.gbcli(&args).transcript(&args, &project.dir);
    }
    golden("equiv.txt", &out);
}
