//! The `song` op's library half: a song file read from the working
//! directory and answered as the RGBDS source `build` would write for it,
//! for a client that assembles it in a build of its own (LoadHigh). Its
//! assembling test needs the pinned RGBDS on `PATH`.

use std::fs;

use gbcli_build::{build, song};
use gbcli_music::{EXAMPLE, parse_text, rgbds_asm, write_uge};
use gbcli_protocol::{ErrorCode, SongArgs};

use crate::common::{Temp, rgbds, starter};
use crate::player::{Played, play};

fn args(path: &str, descriptor: &str) -> SongArgs {
    SongArgs {
        path: path.into(),
        descriptor: descriptor.into(),
    }
}

#[test]
fn a_song_text_is_answered_as_the_source_build_writes_for_it() {
    let temp = Temp::new("song-op-text");
    fs::write(temp.path("theme.song"), EXAMPLE).unwrap();
    let made = song(&temp.0, &args("theme.song", "song_overworld")).unwrap();
    assert_eq!(made.path, "theme.song");
    assert_eq!(made.descriptor, "song_overworld");
    assert_eq!(made.timer, None);
    let want = rgbds_asm(&parse_text(EXAMPLE).unwrap(), "song_overworld");
    assert_eq!(made.asm.as_bytes(), want);
}

#[test]
fn a_uge_of_the_same_song_is_answered_with_the_same_source() {
    let temp = Temp::new("song-op-uge");
    let parsed = parse_text(EXAMPLE).unwrap();
    fs::write(temp.path("theme.song"), EXAMPLE).unwrap();
    fs::write(temp.path("theme.uge"), write_uge(&parsed)).unwrap();
    let text = song(&temp.0, &args("theme.song", "song_t")).unwrap();
    let uge = song(&temp.0, &args("theme.uge", "song_t")).unwrap();
    assert_eq!(uge.asm, text.asm);
}

#[test]
fn a_timer_song_says_its_modulo() {
    let temp = Temp::new("song-op-timer");
    let timed = EXAMPLE.replace("ticks_per_row 6", "ticks_per_row 6\ntimer on 192");
    fs::write(temp.path("t.song"), timed).unwrap();
    let made = song(&temp.0, &args("t.song", "song_t")).unwrap();
    assert_eq!(made.timer, Some(192));
    assert!(made.asm.contains("DEF song_t_TIMER EQU 1"), "{}", made.asm);
}

#[test]
fn a_missing_file_is_song_unreadable_naming_it() {
    let temp = Temp::new("song-op-missing");
    let body = song(&temp.0, &args("no/such.uge", "song_t")).unwrap_err();
    assert_eq!(body.code, ErrorCode::SongUnreadable);
    assert!(
        body.message.starts_with("no/such.uge: "),
        "{}",
        body.message
    );
    assert!(body.remedy.contains("check the path"), "{}", body.remedy);
}

#[test]
fn a_file_of_neither_form_is_song_unreadable_with_its_problem() {
    let temp = Temp::new("song-op-neither");
    fs::write(temp.path("notes.txt"), "la la la\n").unwrap();
    let body = song(&temp.0, &args("notes.txt", "song_t")).unwrap_err();
    assert_eq!(body.code, ErrorCode::SongUnreadable);
    assert!(body.message.starts_with("notes.txt"), "{}", body.message);
    let problem = body.result.unwrap().song_problem.unwrap();
    assert_eq!(problem.path, "notes.txt");
}

#[test]
fn a_descriptor_that_is_no_symbol_is_a_bad_request() {
    let temp = Temp::new("song-op-descriptor");
    fs::write(temp.path("theme.song"), EXAMPLE).unwrap();
    let body = song(&temp.0, &args("theme.song", "song.theme")).unwrap_err();
    assert_eq!(body.code, ErrorCode::BadRequest);
    assert!(body.message.contains("song.theme"), "{}", body.message);
    assert!(body.remedy.contains("letters"), "{}", body.remedy);
}

/// The answer, written as a source of the starter's own in place of its
/// converted song, assembles and links, and the starter's player plays it.
#[test]
fn the_answer_assembles_and_plays_as_a_source_of_its_own() {
    let (temp, options) = starter("song-op-assembles");
    fs::remove_dir_all(temp.path("demo/assets/music")).unwrap();
    fs::write(temp.path("theme.song"), EXAMPLE).unwrap();
    let made = song(&temp.0, &args("theme.song", "song_theme")).unwrap();
    fs::write(temp.path("demo/src/theme_song.asm"), &made.asm).unwrap();
    let built = build(&rgbds(), &options).unwrap();
    assert!(built.songs.is_empty(), "{:?}", built.songs);
    let rom = fs::read(temp.path("demo/build/demo.gb")).unwrap();
    let Played { triggers, .. } = play(&rom, 64 * 6);
    assert!(!triggers[0].is_empty(), "square 1 never sounded");
}
