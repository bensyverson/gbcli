//! `build`'s music: songs under `assets/music` converted to hUGEDriver data
//! by convention, and the starter's player playing them. Needs the pinned
//! RGBDS on `PATH`.

mod common;
mod player;

use std::fs;

use gbcli_build::{BuildError, build};
use gbcli_music::{EXAMPLE, NO_NOTE, parse_text, period_of, rgbds_asm, write_uge};
use gbcli_protocol::{ErrorCode, SongBuilt};

use common::{rgbds, starter};
use player::{Played, play};

#[test]
fn the_starter_converts_its_song_and_says_where_it_went() {
    let (temp, options) = starter("music-starter");
    let made = build(&rgbds(), &options).unwrap();
    assert_eq!(
        made.songs,
        [SongBuilt {
            path: "demo/assets/music/theme.song".into(),
            asm: "demo/build/music/theme.asm".into(),
            descriptor: "song_theme".into(),
            timer: None,
        }]
    );
    assert!(
        made.sources
            .contains(&"demo/build/music/theme.asm".to_string()),
        "{:?}",
        made.sources
    );
    let song = parse_text(EXAMPLE).unwrap();
    assert_eq!(
        fs::read(temp.path("demo/build/music/theme.asm")).unwrap(),
        rgbds_asm(&song, "song_theme")
    );
}

/// The starter's song is the help topic's example; square 1 must sound its
/// notes in order: each trigger's period is the driver's for the note the
/// pattern names (P0 then P4, as the orders play them). A row with a C
/// (volume) effect triggers its note again: the channel restarts to take
/// the new volume.
#[test]
fn the_starter_plays_its_songs_notes() {
    let (temp, options) = starter("music-plays");
    build(&rgbds(), &options).unwrap();
    let rom = fs::read(temp.path("demo/build/demo.gb")).unwrap();
    let Played { triggers, .. } = play(&rom, 2 * 64 * 6 + 10);
    let song = parse_text(EXAMPLE).unwrap();
    let mut want = Vec::new();
    for c in song.orders[0]
        .iter()
        .flat_map(|k| song.pattern(*k).unwrap().rows)
    {
        if c.note != NO_NOTE {
            want.push(period_of(c.note).unwrap());
        }
        if c.effect_code == 0xC {
            want.push(*want.last().unwrap());
        }
    }
    let got: Vec<u16> = triggers[0].iter().map(|t| t.period).collect();
    assert!(got.len() >= want.len(), "{got:?}");
    assert_eq!(got[..want.len()], want);
}

#[test]
fn a_uge_converts_to_the_same_data_as_its_text() {
    let (temp, options) = starter("music-uge");
    let song = parse_text(EXAMPLE).unwrap();
    fs::remove_file(temp.path("demo/assets/music/theme.song")).unwrap();
    fs::write(temp.path("demo/assets/music/theme.uge"), write_uge(&song)).unwrap();
    let made = build(&rgbds(), &options).unwrap();
    assert_eq!(made.songs[0].path, "demo/assets/music/theme.uge");
    assert_eq!(
        fs::read(temp.path("demo/build/music/theme.asm")).unwrap(),
        rgbds_asm(&song, "song_theme")
    );
}

#[test]
fn a_song_that_does_not_parse_fails_the_build_at_its_line_and_column() {
    let (temp, options) = starter("music-bad");
    let path = temp.path("demo/assets/music/theme.song");
    let text = fs::read_to_string(&path).unwrap();
    fs::write(&path, text.replace("ticks_per_row 6", "ticks_per_row six")).unwrap();
    let err = build(&rgbds(), &options).unwrap_err();
    let body = err.body();
    assert_eq!(body.code, ErrorCode::SongUnreadable);
    assert!(
        body.message
            .starts_with("demo/assets/music/theme.song:4:15: "),
        "{}",
        body.message
    );
    let problem = body.result.unwrap().song_problem.unwrap();
    assert_eq!((problem.line, problem.column), (Some(4), Some(15)));
    assert!(!temp.path("demo/build/demo.gb").exists());
}

#[test]
fn two_song_files_with_one_name_are_refused_before_any_tool_runs() {
    let (temp, options) = starter("music-twice");
    let song = parse_text(EXAMPLE).unwrap();
    fs::write(temp.path("demo/assets/music/theme.uge"), write_uge(&song)).unwrap();
    match build(&rgbds(), &options) {
        Err(BuildError::Layout { message, remedy }) => {
            assert!(message.contains("theme.song"), "{message}");
            assert!(message.contains("theme.uge"), "{message}");
            assert!(message.contains("song_theme"), "{message}");
            assert!(remedy.contains("rename"), "{remedy}");
        }
        other => panic!("expected a layout error, got {other:?}"),
    }
    assert!(!temp.path("demo/build/obj").exists());
}

#[test]
fn songs_without_hugedriver_are_refused_with_the_way_to_get_it() {
    let (temp, options) = starter("music-no-driver");
    fs::remove_file(temp.path("demo/include/hUGE.inc")).unwrap();
    match build(&rgbds(), &options) {
        Err(BuildError::Layout { message, remedy }) => {
            assert!(message.contains("hUGE.inc"), "{message}");
            assert!(message.contains("assets/music"), "{message}");
            assert!(remedy.contains("gbcli new"), "{remedy}");
        }
        other => panic!("expected a layout error, got {other:?}"),
    }
}

#[test]
fn a_project_without_assets_music_builds_no_songs() {
    let (temp, options) = starter("music-none");
    common::without_music(&temp.path("demo"));
    let made = build(&rgbds(), &options).unwrap();
    assert!(made.songs.is_empty());
    assert!(!temp.path("demo/build/music").exists());
}

/// A song with the timer on is driven from the timer interrupt, at the rate
/// its TMA sets, and still plays its notes.
#[test]
fn a_timer_song_is_driven_by_the_timer_interrupt() {
    let (temp, options) = starter("music-timer");
    let path = temp.path("demo/assets/music/theme.song");
    let text = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        text.replace("ticks_per_row 6", "ticks_per_row 6\ntimer on 192"),
    )
    .unwrap();
    let made = build(&rgbds(), &options).unwrap();
    assert_eq!(made.songs[0].timer, Some(192));
    let rom = fs::read(temp.path("demo/build/demo.gb")).unwrap();
    // 4096 / (256 - 192) = 64 ticks a second, about 1.07 a frame.
    let played = play(&rom, 120);
    assert_eq!(played.ie & 0b101, 0b101, "VBlank and timer enabled");
    assert_eq!(played.tac & 0b111, 0b100, "the timer runs at 4096 Hz");
    assert_eq!(played.tma, 192);
    let first: Vec<u16> = played.triggers[0]
        .iter()
        .take(3)
        .map(|t| t.period)
        .collect();
    let want: Vec<u16> = [24, 28, 31].map(|n| period_of(n).unwrap()).to_vec(); // C-5 E-5 G-5
    assert_eq!(first, want);
    // Row 4 (E-5) starts 4 rows × 6 ticks after row 0, at 64 Hz: 0.375 s.
    let gap = played.triggers[0][1].ticks - played.triggers[0][0].ticks;
    let want = (1u64 << 23) * 24 / 64;
    assert!(gap.abs_diff(want) < 1 << 16, "{gap} vs {want}");
}
