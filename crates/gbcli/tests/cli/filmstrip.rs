//! A run's filmstrip through the binary, as a golden transcript.

use crate::common::{Project, golden};

#[test]
fn a_filmstrip_prints_a_row_per_frame() {
    let project = Project::new("filmstrip");
    let mut out = String::new();
    for args in [
        &[
            "run",
            "demo.gb",
            "--frames",
            "6",
            "--filmstrip",
            "f.png",
            "--every",
            "2",
        ][..],
        &["run", "--frames", "18", "--filmstrip", "g.png", "--json"],
        &["run", "--every", "2"],
        &[
            "run",
            "--frames",
            "4",
            "--filmstrip",
            "c.png",
            "--every",
            "2",
            "--crop",
            "0,0,16,8",
        ],
    ] {
        let mut args = args.to_vec();
        args.extend(["--session", "demo"]);
        out += &project.gbcli(&args).transcript(&args, &project.dir);
    }
    assert!(project.path("f.png").is_file() && project.path("g.png").is_file());
    // Two 16x8 frames and the one-pixel gap between them.
    assert!(
        out.contains("filmstrip path=c.png width=33 height=8"),
        "{out}"
    );
    golden("filmstrip.txt", &out);
}
