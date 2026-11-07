//! The cycle profiler through the binary, as a golden transcript: `measure`
//! (rows, `--json`, `--max` as an assertion) and run's `--measure`,
//! `--profile` and `--frame-budget`.

use crate::common::{Project, golden};

#[test]
fn measure_and_the_run_probes_print_their_rows() {
    let project = Project::new("profile");
    let mut out = String::new();
    for args in [
        &["run", "demo.gb", "--frames", "2"][..],
        &["measure", "Main.loop..Main.loop", "--iterations", "3"],
        &[
            "measure",
            "Main.loop..Main.loop",
            "--iterations",
            "2",
            "--json",
        ],
        &["measure", "Main.loop..Main.loop", "--max", "100"],
        &["measure", "Main.loop", "--max", "100"],
        &[
            "run",
            "--frames",
            "3",
            "--measure",
            "Main.loop..Work",
            "--profile",
            "--top",
            "2",
            "--frame-budget",
        ],
        &["run", "--frames", "1", "--top", "2"],
    ] {
        let mut args = args.to_vec();
        args.extend(["--session", "demo"]);
        out += &project.gbcli(&args).transcript(&args, &project.dir);
    }
    golden("profile.txt", &out);
}
