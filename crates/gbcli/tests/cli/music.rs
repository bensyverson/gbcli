//! Music through the binary: `new` makes a project with a song, `build`
//! converts it, and `run --audio-log` shows its notes; the committed test
//! song's piano roll and spectrogram are pinned as PNG goldens a person can
//! look at. Needs the pinned RGBDS on `PATH`.

use std::fs;

use crate::common::{Project, png_golden};

const TEST_SONG: &str = include_str!("../../../build/tests/data/every_effect_but_6xx.song");

/// The starter's song (the help topic's example) plays: its first square-1
/// notes are triggered in order, at the driver's periods for C-5, E-5, G-5.
/// Read from `--json`: the text log is capped, and these writes come after
/// its first rows.
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
        "--json",
    ]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let response: serde_json::Value = serde_json::from_str(&run.stdout).unwrap();
    let log = response["result"]["audio"]["log"].as_array().unwrap();
    let periods: Vec<&str> = log
        .iter()
        .filter_map(|w| w["text"].as_str())
        .filter(|t| t.contains("square 1 trigger"))
        .filter_map(|t| t.split("period ").nth(1)?.split(' ').next())
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

/// One note, A-3, on square 1 and, a row later, on the wave channel. Not
/// on the same row: two patterns with equal cells are stored once, and the
/// export counts a pattern's instruments for the first channel that plays
/// it (hUGETracker's quirk, `gbcli_music::asm`), so the wave instrument
/// would be left out.
const OCTAVE_PROBE: &str = r#"format gbcli-song 1
name "Octave probe"
ticks_per_row 6

wave 0 FFFFFFFFFFFFFFFF0000000000000000

instrument square 1 "Tone"
  envelope 15 down 0
  duty 50%

instrument wave 1 "Tone"
  level 100%
  waveform 0

orders
00 P0 P1 P2 P3

pattern P0 P1 P2 P3
00 A-3 01 ... ... | ... .. ... ... | ... .. ... ... | ... .. ... ...
01 ... .. ... ... | ... .. ... ... | A-3 01 ... ... | ... .. ... ...
"#;

/// The fundamental of a 16-bit stereo WAV this gbcli wrote, from the left
/// channel's crossings of its mean over its last half second, with
/// hysteresis so ripple can't count twice.
fn fundamental_hz(wav: &[u8]) -> f64 {
    assert_eq!(&wav[36..40], b"data");
    let left: Vec<f64> = wav[44..]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|f| f64::from(i16::from_le_bytes([f[0], f[1]])))
        .collect();
    let rate = 32_768;
    let tail = &left[left.len() - rate / 2..];
    let mean = tail.iter().sum::<f64>() / tail.len() as f64;
    let peak = tail.iter().map(|s| (s - mean).abs()).fold(0.0, f64::max);
    assert!(peak > 500.0, "silent: peak {peak}");
    let (mut high, mut rises) = (tail[0] > mean, 0u32);
    for &s in tail {
        if !high && s > mean + peak / 4.0 {
            high = true;
            rises += 1;
        } else if high && s < mean - peak / 4.0 {
            high = false;
        }
    }
    f64::from(rises) * 2.0
}

/// Probe for `help music` (leaf qjAObe): hUGEDriver writes one period table
/// to every tone channel, so A-3 is period 854 on square 1 and on the wave
/// channel alike; the wave channel steps through 32 samples where a square
/// steps through 8 duty steps, so at one period it sounds an octave lower:
/// A2 (110 Hz) on square 1, A1 (55 Hz) on the wave. The audio log names
/// each by the pitch heard, and SameBoy's own output agrees.
#[test]
fn the_wave_channel_sounds_an_octave_below_a_square_at_the_same_note() {
    let p = Project::new("music-octaves");
    assert_eq!(p.rgbds(&["new", "game"]).code, 0);
    fs::write(p.path("game/assets/music/theme.song"), OCTAVE_PROBE).unwrap();
    let built = p.rgbds(&["build", "game"]);
    assert_eq!(built.code, 0, "{}", built.stderr);
    let run = p.rgbds(&[
        "run",
        "game/build/game.gb",
        "--session",
        "game",
        "--frames",
        "120",
        "--audio-log",
        "--wav",
        "probe.wav",
        "--channels",
        "each",
        "--json",
    ]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let response: serde_json::Value = serde_json::from_str(&run.stdout).unwrap();
    let log = response["result"]["audio"]["log"].as_array().unwrap();
    let trigger = |channel: &str| {
        log.iter()
            .filter_map(|w| w["text"].as_str())
            .find(|t| t.contains(&format!("{channel} trigger")))
            .unwrap_or_else(|| panic!("no {channel} trigger in {}", run.stdout))
            .to_string()
    };
    let square = trigger("square 1");
    let wave = trigger("wave");
    assert!(square.contains("period 854 = A2 "), "{square}");
    assert!(wave.contains("period 854 = A1 "), "{wave}");
    let square_hz = fundamental_hz(&fs::read(p.path("probe-square1.wav")).unwrap());
    let wave_hz = fundamental_hz(&fs::read(p.path("probe-wave.wav")).unwrap());
    assert!(
        (square_hz - 109.8).abs() <= 4.0,
        "square 1 sounds {square_hz} Hz"
    );
    assert!(
        (wave_hz - 54.9).abs() <= 4.0,
        "the wave sounds {wave_hz} Hz"
    );
}
