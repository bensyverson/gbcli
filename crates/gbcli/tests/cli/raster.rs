//! `run --raster` through the binary, as a golden transcript: the writes by
//! frame and line as rows and `--json`, a run cut short with its stop, and
//! the warning a run ending in STOP prints.

use std::fs;

use crate::common::{Project, golden};

const RASTER: &[u8] = include_bytes!("../../../engine/tests/roms/raster.gb");
const RASTER_SYM: &[u8] = include_bytes!("../../../engine/tests/roms/raster.sym");
const ILLEGAL: &[u8] = include_bytes!("../../../sameboy/tests/roms/illegal.gb");
const STOP_WAKE: &[u8] = include_bytes!("../../../sameboy/tests/roms/stop_wake.gb");

#[test]
fn run_raster_prints_each_write_on_its_line() {
    let project = Project::new("raster");
    fs::write(project.path("raster.gb"), RASTER).unwrap();
    fs::write(project.path("raster.sym"), RASTER_SYM).unwrap();
    fs::write(project.path("illegal.gb"), ILLEGAL).unwrap();
    fs::write(project.path("stop.gb"), STOP_WAKE).unwrap();
    let mut out = String::new();
    for args in [
        &[
            "run",
            "raster.gb",
            "--frames",
            "2",
            "--raster",
            "--session",
            "ras",
        ][..],
        &[
            "run",
            "--frames",
            "1",
            "--raster",
            "--json",
            "--session",
            "ras",
        ],
        &[
            "run",
            "illegal.gb",
            "--frames",
            "5",
            "--raster",
            "--session",
            "ill",
        ],
        &["run", "stop.gb", "--frames", "2", "--session", "stop"],
    ] {
        out += &project.gbcli(args).transcript(args, &project.dir);
    }
    golden("raster.txt", &out);
}
