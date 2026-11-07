//! Music through the binary: `new` makes a project with a song, `build`
//! converts it, and `run --audio-log` shows its notes; the committed test
//! song's piano roll and spectrogram are pinned as PNG goldens a person can
//! look at. Needs the pinned RGBDS on `PATH`.

use std::fs;
use std::path::Path;

use crate::common::Project;

const EVERY_EFFECT: &str = include_str!("../../../build/tests/data/every_effect.song");

/// The starter's song (the help topic's example) plays: its first square-1
/// notes are triggered in order, at the driver's periods for C-5, E-5, G-5.
#[test]
fn a_new_projects_song_plays_in_the_audio_log() {
    let p = Project::new("music-plays");
    assert_eq!(p.rgbds(&["new", "game"]).code, 0);
    let built = p.rgbds(&["build", "game"]);
    assert_eq!(built.code, 0, "{}", built.stderr);
    assert!(
        built.stdout.contains(
            "song path=game/assets/music/theme.song asm=game/build/music/theme.asm \
             descriptor=song_theme\n"
        ),
        "{}",
        built.stdout
    );
    let run = p.rgbds(&[
        "run",
        "game/build/game.gb",
        "--session",
        "game",
        "--frames",
        "240",
        "--audio-log",
    ]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let periods: Vec<&str> = run
        .stdout
        .lines()
        .filter(|l| l.contains("square 1 trigger"))
        .filter_map(|l| l.split("period ").nth(1)?.split(' ').next())
        .take(3)
        .collect();
    assert_eq!(periods, ["1546", "1650", "1714"], "{}", run.stdout);
}

/// `data/every_effect.song` built into the starter and played to its end:
/// the piano roll and spectrogram of the run are goldens (bless with
/// `GBCLI_BLESS=1`, then look at them).
#[test]
fn the_test_songs_piano_roll_and_spectrogram_are_pinned() {
    let p = Project::new("music-goldens");
    assert_eq!(p.rgbds(&["new", "game"]).code, 0);
    fs::write(p.path("game/assets/music/theme.song"), EVERY_EFFECT).unwrap();
    let built = p.rgbds(&["build", "game"]);
    assert_eq!(built.code, 0, "{}", built.stderr);
    assert!(
        built.stdout.contains("descriptor=song_theme tma=192\n"),
        "{}",
        built.stdout
    );
    // About 150 frames of boot, then 128 rows of 4 ticks at 64 Hz (8 s).
    let run = p.rgbds(&[
        "run",
        "game/build/game.gb",
        "--session",
        "game",
        "--frames",
        "660",
        "--piano",
        "piano.png",
        "--spectrogram",
        "spectrogram.png",
    ]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    for (made, golden) in [
        ("piano.png", "every_effect_piano.png"),
        ("spectrogram.png", "every_effect_spectrogram.png"),
    ] {
        png_golden(&fs::read(p.path(made)).unwrap(), golden);
    }
}

fn png_golden(actual: &[u8], name: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/cli/golden")
        .join(name);
    if std::env::var_os("GBCLI_BLESS").is_some() {
        fs::write(&path, actual).unwrap();
        return;
    }
    let expected = fs::read(&path).unwrap_or_else(|_| {
        panic!(
            "no golden {}; write it with GBCLI_BLESS=1 and look at it",
            path.display()
        )
    });
    if expected != actual {
        let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
        fs::write(&out, actual).unwrap();
        panic!(
            "{} differs; the actual image is {}",
            path.display(),
            out.display()
        );
    }
}
