//! Music through the binary: `new` makes a project with a song, `build`
//! converts it, and `run --audio-log` shows its notes; the committed test
//! song's piano roll and spectrogram are pinned as PNG goldens a person can
//! look at. Needs the pinned RGBDS on `PATH`.

use std::fs;

use crate::common::{Project, png_golden};

const TEST_SONG: &str = include_str!("../../../build/tests/data/every_effect_but_6xx.song");

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

/// `data/every_effect_but_6xx.song` built into the starter and played to its
/// end: the piano roll and spectrogram of the run are goldens (bless with
/// `GBCLI_BLESS=1`, then look at them).
#[test]
fn the_test_songs_piano_roll_and_spectrogram_are_pinned() {
    let p = Project::new("music-goldens");
    assert_eq!(p.rgbds(&["new", "game"]).code, 0);
    fs::write(p.path("game/assets/music/theme.song"), TEST_SONG).unwrap();
    let built = p.rgbds(&["build", "game"]);
    assert_eq!(built.code, 0, "{}", built.stderr);
    assert!(
        built.stdout.contains("descriptor=song_theme tma=192\n"),
        "{}",
        built.stdout
    );
    // About 150 frames of boot, then 688 ticks at 64 Hz (10.75 s): orders
    // 0 and 1 at 4 ticks a row, then order 2's 48 rows, 16 of them at 3.
    let run = p.rgbds(&[
        "run",
        "game/build/game.gb",
        "--session",
        "game",
        "--frames",
        "820",
        "--piano",
        "piano.png",
        "--spectrogram",
        "spectrogram.png",
        // Native pixels: the goldens pin what is drawn, not the default scale.
        "--scale",
        "1",
    ]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    for (made, golden) in [
        ("piano.png", "every_effect_but_6xx_piano.png"),
        ("spectrogram.png", "every_effect_but_6xx_spectrogram.png"),
    ] {
        png_golden(&fs::read(p.path(made)).unwrap(), golden);
    }
}
