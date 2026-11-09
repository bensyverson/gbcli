//! Observations across a run and of palette RAM, through the binary, as one
//! golden transcript: `run --oam-log` as rows and `--json`, the audio log
//! filtered to one channel with its summary, and `view palettes` on a DMG and
//! a GBC.

use std::fs;

use crate::common::{Project, golden};

const V1: &[u8] = include_bytes!("../../../engine/tests/roms/rebuild_v1.gb");
const TUNE: &[u8] = include_bytes!("../../../audio/tests/roms/tune.gb");
const GBC_PALETTE: &[u8] = include_bytes!("../../../engine/tests/roms/gbc_palette.gb");

/// OAM with twelve objects on lines 64-71, so the 10-per-line limit drops
/// the last two, and the rest off screen: `oam:0=…` for `--poke`.
fn crowded_oam() -> String {
    let mut bytes = Vec::new();
    for i in 0..40u32 {
        let entry = if i < 12 {
            [80, 8 + 8 * i, i, 0]
        } else {
            [0; 4]
        };
        bytes.extend(entry.map(|b| b.to_string()));
    }
    format!("oam:0={}", bytes.join(","))
}

#[test]
fn observations_print_rows_and_json() {
    let project = Project::new("observe");
    fs::write(project.path("v1.gb"), V1).unwrap();
    fs::write(project.path("tune.gb"), TUNE).unwrap();
    fs::write(project.path("palette.gb"), GBC_PALETTE).unwrap();
    let oam = crowded_oam();
    let mut out = String::new();
    let steps: &[&[&str]] = &[
        &["run", "v1.gb", "--frames", "2", "--session", "oam"],
        &[
            "run",
            "--frames",
            "3",
            "--poke",
            &oam,
            "--oam-log",
            "--session",
            "oam",
        ],
        &[
            "run",
            "--frames",
            "1",
            "--oam-log",
            "--json",
            "--session",
            "oam",
        ],
        &[
            "run",
            "tune.gb",
            "--frames",
            "40",
            "--audio-log",
            "--audio-summary",
            "--audio-channel",
            "4",
            "--session",
            "tune",
        ],
        &["view", "palettes", "--session", "oam"],
        &[
            "run",
            "palette.gb",
            "--model",
            "gbc",
            "--frames",
            "30",
            "--session",
            "gbc",
        ],
        &["view", "palettes", "--session", "gbc"],
        &["view", "palettes", "--json", "--session", "gbc"],
    ];
    for args in steps {
        out += &project.gbcli(args).transcript(args, &project.dir);
    }
    golden("observe.txt", &out);
}

#[test]
fn a_channel_filter_alone_is_refused() {
    let project = Project::new("observe-filter");
    fs::write(project.path("tune.gb"), TUNE).unwrap();
    project
        .gbcli(&[
            "run",
            "tune.gb",
            "--audio-channel",
            "noise",
            "--session",
            "tune",
        ])
        .fails(2, "bad_request", "add --audio-log or --audio-summary");
}
