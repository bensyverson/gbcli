//! A run's audio through the binary: the decoded log as rows and `--json`,
//! the WAVs and images, as golden transcripts; and the refusals.

use std::fs;

use crate::common::{Project, golden};

const TUNE: &[u8] = include_bytes!("../../../audio/tests/roms/tune.gb");

/// Each step's arguments; `--session tune` and the form's flag are added.
const SCRIPT: &[&[&str]] = &[
    &["checkpoint", "start", "--rom", "tune.gb"],
    &["run", "--at", "start", "--frames", "40", "--audio-log"],
    &[
        "run",
        "--at",
        "start",
        "--frames",
        "40",
        "--wav",
        "tune.wav",
        "--channels",
        "each",
        "--piano",
        "piano.png",
        "--spectrogram",
        "spectrogram.png",
    ],
];

fn transcript(name: &str, form: &[&str]) -> String {
    let project = Project::new(name);
    fs::write(project.path("tune.gb"), TUNE).unwrap();
    let mut out = String::new();
    for step in SCRIPT {
        let mut args: Vec<&str> = step.to_vec();
        args.extend(["--session", "tune"]);
        args.extend(form);
        out += &project.gbcli(&args).transcript(&args, &project.dir);
    }
    for name in [
        "tune.wav",
        "tune-square1.wav",
        "tune-noise.wav",
        "piano.png",
    ] {
        assert!(project.path(name).is_file(), "{name} was written");
    }
    out
}

#[test]
fn a_runs_audio_prints_rows() {
    golden("audio.rows.txt", &transcript("audio-rows", &[]));
}

#[test]
fn a_runs_audio_prints_the_serve_response_with_json() {
    golden("audio.json.txt", &transcript("audio-json", &["--json"]));
}

#[test]
fn channels_without_a_wav_is_a_usage_error() {
    let project = Project::new("audio-channels");
    project
        .gbcli(&["run", "--session", "demo", "--channels", "each"])
        .fails(2, "bad_request", "add --wav PATH");
}
