//! The register-write comparison that pins the emitter: two ROMs, each the
//! starter (its player and hUGEDriver) with one song's data, run side by
//! side by `equiv`, which compares every frame's sound-register writes.
//! Against hUGETracker's own export of a song this is the emitter's check;
//! here it is checked itself, on the committed test song
//! `data/every_effect.song` (CC0), which uses every effect group, all four
//! channels and the timer.

mod common;
mod player;

use std::fs;
use std::path::Path;

use gbcli_build::build;
use gbcli_engine::equiv::{AudioCompare, Difference, EquivSpec, Verdict, compare};
use gbcli_music::{Song, parse_text, rgbds_asm};

use common::{Temp, rgbds, starter};
use player::machine;

const EVERY_EFFECT: &str = include_str!("data/every_effect.song");

/// The ROM the starter builds with `asm` as its only song data, assembled
/// from `src/` and declaring `song_theme`.
fn rom_with_asm(temp: &Temp, name: &str, asm: &[u8]) -> Vec<u8> {
    let (_keep, mut options) = starter(name);
    let project = options.project.clone();
    fs::remove_dir_all(project.join("assets")).unwrap();
    fs::write(project.join("src/song.asm"), asm).unwrap();
    options.display_base = temp.0.clone();
    build(&rgbds(), &options).unwrap();
    fs::read(project.join(format!("build/{}.gb", dir_name(&project)))).unwrap()
}

fn dir_name(p: &Path) -> String {
    p.file_name().unwrap().to_string_lossy().into_owned()
}

/// The song's full length in frames, rounded up, plus the boot: rows ×
/// ticks per row at the driver's tick rate (60 Hz per frame, or the timer's
/// 4096 / (256 − TMA) Hz).
fn frames(song: &Song, rows: u64) -> u64 {
    let ticks = rows * song.ticks_per_row as u64;
    let hz = if song.timer_enabled != 0 {
        4096.0 / (256 - (song.timer_divider & 0xFF)) as f64
    } else {
        59.7275
    };
    300 + (ticks as f64 / hz * 59.7275).ceil() as u64
}

fn same(a: &[u8], b: &[u8], frames: u64) -> Verdict {
    let (mut a, mut b) = (machine(a, None), machine(b, None));
    let spec = EquivSpec {
        audio: AudioCompare::Writes,
        ..EquivSpec::new(frames)
    };
    compare(&mut a, &mut b, &spec).unwrap()
}

/// The emitter's output doesn't depend on where a pattern is stored: the
/// song with its patterns stored in reverse plays the same writes for its
/// whole length (both orders), timer and all.
#[test]
fn the_test_song_plays_the_same_whatever_its_storage_order() {
    let temp = Temp::new("equiv-storage");
    let song = parse_text(EVERY_EFFECT).unwrap();
    let mut reversed = song.clone();
    reversed.patterns.reverse();
    let a = rom_with_asm(&temp, "equiv-storage-a", &rgbds_asm(&song, "song_theme"));
    let b = rom_with_asm(
        &temp,
        "equiv-storage-b",
        &rgbds_asm(&reversed, "song_theme"),
    );
    assert_ne!(a, b, "the layouts differ");
    let n = frames(&song, 128);
    assert_eq!(
        same(&a, &b, n),
        Verdict::Same {
            frames: n,
            ended: None
        }
    );
}

/// One changed note is found, as a sound-register write, in the frame it
/// first plays: order 1's row 32 on square 2 (B-3 → C-4).
#[test]
fn one_changed_note_diverges_in_its_sound_register_writes() {
    let temp = Temp::new("equiv-note");
    let song = parse_text(EVERY_EFFECT).unwrap();
    let mut changed = song.clone();
    let p5 = changed.patterns.iter_mut().find(|p| p.key == 5).unwrap();
    assert_eq!(p5.rows[32].note, 11, "B-3");
    p5.rows[32].note = 12;
    let a = rom_with_asm(&temp, "equiv-note-a", &rgbds_asm(&song, "song_theme"));
    let b = rom_with_asm(&temp, "equiv-note-b", &rgbds_asm(&changed, "song_theme"));
    let Verdict::Diverged(d) = same(&a, &b, frames(&song, 128)) else {
        panic!("the change went unheard");
    };
    assert!(
        d.differences
            .iter()
            .all(|x| matches!(x, Difference::Apu { .. })),
        "{:?}",
        d.differences
    );
    // Row 96 of the song: (64 + 32) rows × 4 ticks at 64 Hz, 6 s after the
    // player's first write (MusicInit turning the APU on).
    let start = player::play(&a, 1).writes[0].at.0;
    let at = (d.ticks - start) as f64 / f64::from(1 << 23);
    assert!((5.95..6.1).contains(&at), "{at} s");
}
