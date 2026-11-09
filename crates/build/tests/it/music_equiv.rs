//! The register-write comparison that pins the emitter: two ROMs, each the
//! starter (its player and hUGEDriver a3cbd0ce) with one song's data, run
//! side by side by `equiv`, which compares every frame's sound-register
//! writes. Our emitter's output is compared with hUGETracker 1.0.11's own
//! RGBDS export (`scripts/music/export-reference.sh`): of every fetched
//! sample over its whole song (the pre-release suite; three of them for 600
//! frames here too), and of the committed test song
//! `data/every_effect_but_6xx.song` (CC0) over its whole length, against its
//! committed export. Every comparison starts at the boot ROM's handover, so
//! its frames are the song's. The harness is also checked on itself: storage
//! order doesn't matter, and one changed note is heard.

use crate::common;
use crate::player;

use std::fs;
use std::path::{Path, PathBuf};

use gbcli_build::build;
use gbcli_engine::equiv::{AudioCompare, Difference, EquivSpec, Verdict, compare};
use gbcli_music::{Song, descriptor_for, parse_text, read_song, rgbds_asm};

use common::{Temp, rgbds, starter};
use player::booted;

const TEST_SONG: &str = include_str!("../data/every_effect_but_6xx.song");

/// hUGETracker's export of `TEST_SONG`, committed with its provenance.
const TEST_SONG_EXPORT: &str = "tests/data/every_effect_but_6xx.asm";

/// The ROM the starter builds with `asm` as its only song data, assembled
/// from `src/` and declaring `song_theme`.
fn rom_with_asm(temp: &Temp, name: &str, asm: &[u8]) -> Vec<u8> {
    let (_keep, mut options) = starter(name);
    let project = options.project.clone();
    fs::remove_dir_all(project.join("assets/music")).unwrap();
    fs::write(project.join("src/song.asm"), asm).unwrap();
    options.display_base = temp.0.clone();
    build(&rgbds(), &options).unwrap();
    fs::read(project.join(format!("build/{}.gb", dir_name(&project)))).unwrap()
}

fn dir_name(p: &Path) -> String {
    p.file_name().unwrap().to_string_lossy().into_owned()
}

/// Frames past a song's computed end that a whole-song comparison still
/// runs, into its repeat.
const TAIL: u64 = 150;

/// The song's length in frames after the handover, rounded up, plus
/// [`TAIL`]: rows × ticks per row at the driver's tick rate (60 Hz per
/// frame, or the timer's 4096 / (256 − TMA) Hz).
fn frames(song: &Song, rows: u64) -> u64 {
    let ticks = rows * song.ticks_per_row as u64;
    let hz = if song.timer_enabled != 0 {
        4096.0 / (256 - (song.timer_divider & 0xFF)) as f64
    } else {
        59.7275
    };
    TAIL + (ticks as f64 / hz * 59.7275).ceil() as u64
}

/// Every order's 64 rows once, at the song's starting speed, plus [`TAIL`].
/// A pattern break or a faster Fxx makes the song shorter, so this runs
/// into its repeat; a slower Fxx would leave its end unheard.
fn whole(song: &Song) -> u64 {
    frames(song, song.orders[0].len() as u64 * 64)
}

/// Compares two ROMs' frames from the boot ROM's handover on. The boot runs
/// the same on both, and `compare` refuses machines that hand over at
/// different ticks.
fn same(a: &[u8], b: &[u8], frames: u64) -> Verdict {
    let (mut a, mut b) = (booted(a), booted(b));
    let spec = EquivSpec {
        audio: AudioCompare::Writes,
        ..EquivSpec::new(frames)
    };
    compare(&mut a, &mut b, &spec).unwrap()
}

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Where the pinned corpus lives: `$GBCLI_TEST_ROMS`, else
/// `tests/corpus/fetched/` at the repo root.
fn corpus_dir() -> PathBuf {
    match std::env::var_os("GBCLI_TEST_ROMS") {
        Some(dir) => PathBuf::from(dir),
        None => repo().join("tests/corpus/fetched"),
    }
}

/// `file` from the corpus, or a failure naming the commands that make it.
fn corpus_file(rel: &str) -> Vec<u8> {
    let full = corpus_dir().join(rel);
    fs::read(&full).unwrap_or_else(|err| {
        panic!(
            "{} is missing ({err}). From the repo root, fetch the samples, then export them \
             with hUGETracker's own export:\n    \
             python3 -I scripts/corpus/fetch.py --only hugetracker-samples\n    \
             scripts/music/export-reference.sh\n\
             (or point GBCLI_TEST_ROMS at a directory holding {rel})",
            full.display()
        )
    })
}

/// hUGETracker's export as the starter links it. The player reads the song's
/// timer setting from `song_theme_TIMER` and `song_theme_TMA`, which our
/// emitter exports and hUGETracker's leaves to the player, so they are
/// appended from the song. The export's unscoped labels (`order1`, `waves`)
/// are hUGEDriver's WRAM names too, but neither object exports them, so the
/// link keeps them apart.
fn linkable_export(export: &[u8], song: &Song) -> Vec<u8> {
    let mut asm = export.to_vec();
    let timer = u8::from(song.timer_enabled != 0);
    let tma = song.timer_divider & 0xFF;
    asm.extend_from_slice(
        format!(
            "\nDEF song_theme_TIMER EQU {timer}\nDEF song_theme_TMA EQU {tma}\n\
             EXPORT song_theme_TIMER, song_theme_TMA\n"
        )
        .as_bytes(),
    );
    asm
}

/// How long a comparison runs.
#[derive(Clone, Copy)]
enum Length {
    Frames(u64),
    /// Every order's 64 rows once, plus [`TAIL`].
    Whole,
}

/// Our emitter's output and hUGETracker's export of `song`, each linked into
/// the starter with the same driver and player, write the same sound
/// registers in every frame.
fn matches_reference(name: &str, song: &Song, export: &[u8], length: Length) {
    let id = descriptor_for(name);
    let temp = Temp::new(&format!("equiv-ref-{id}"));
    let ours = rom_with_asm(&temp, &format!("{id}-ours"), &rgbds_asm(song, "song_theme"));
    let theirs = rom_with_asm(&temp, &format!("{id}-ht"), &linkable_export(export, song));
    let n = match length {
        Length::Frames(n) => n,
        Length::Whole => whole(song),
    };
    assert_eq!(
        same(&ours, &theirs, n),
        Verdict::Same {
            frames: n,
            ended: None
        },
        "{name}: our emitter and hUGETracker's export write different sound registers"
    );
    eprintln!("{name}: the same sound-register writes over {n} frames");
}

/// A fetched hUGETracker sample against hUGETracker 1.0.11's export of it.
fn sample_matches_reference(stem: &str, length: Length) {
    let uge = corpus_file(&format!("hugetracker-samples/{stem}.uge"));
    let song = read_song(&uge).unwrap().song;
    let export = corpus_file(&format!("hugetracker-exports/{stem}.asm"));
    matches_reference(stem, &song, &export, length);
}

/// One whole-song test per sample, so they run in parallel, in the
/// pre-release suite: each is thousands of frames.
macro_rules! whole_songs {
    ($($whole:ident: $stem:literal;)*) => {$(
        #[test]
        #[ignore = "pre-release: the whole song, thousands of frames"]
        fn $whole() {
            sample_matches_reference($stem, Length::Whole);
        }
    )*};
}

whole_songs! {
    jester_fishnchips_whole: "Jester - FishNChips";
    reed_funkabeers_revenge_whole: "Reed - Funkabeer's Revenge";
    svl_uwu_whole: "SVL - UwU";
    arachno_a_sad_touch_whole: "Arachno - A Sad Touch";
    maktone_softworld_whole: "Maktone - Softworld";
    melonadem_boatship_for_rent_whole: "MelonadeM - Boatship for Rent";
    svl_yyna_whole: "SVL - Yyna";
    raphaelgoulart_murderous_funk_machine_whole:
        "raphaelgoulart - The Murderous Funk Machine (intro)";
    coffee_bat_blue_ocean_whole: "Coffee Bat - Blue Ocean";
    coffee_bat_pilgrims_peril_menu_whole: "Coffee Bat - Pilgrim's Peril Menu";
    coffee_bat_wyrmhole_whole: "Coffee Bat - Wyrmhole";
    chavez_monkeys_on_mars_whole: "Chavez - Monkeys on Mars";
    fade_microplastics_in_the_air_whole: "FADE - Microplastics in the Air";
    fade_strap_in_and_suit_up_whole: "FADE - Strap in and Suit Up";
    tempest_kekri_whole: "Tempest - Kekri";
}

/// Song frames a warm sample test compares. Three samples are compared warm,
/// chosen for how much of the driver their first 600 frames reach; the rest
/// only whole. No sample uses the timer; the test song does, and is compared
/// over its whole length below.
const WARM_FRAMES: u64 = 600;

/// The most effects of any sample in its first 600 frames (2, 3, 4, 5, 8,
/// A, C, E, F) on all four channels.
#[test]
fn svl_yyna() {
    sample_matches_reference("SVL - Yyna", Length::Frames(WARM_FRAMES));
}

/// The most subpatterns of any sample (21), with wave and noise.
#[test]
fn fade_strap_in_and_suit_up() {
    sample_matches_reference("FADE - Strap in and Suit Up", Length::Frames(WARM_FRAMES));
}

/// The fastest sample (2 ticks a row: 300 rows in 600 frames), on all four
/// channels with subpatterns.
#[test]
fn coffee_bat_wyrmhole() {
    sample_matches_reference("Coffee Bat - Wyrmhole", Length::Frames(WARM_FRAMES));
}

/// The test song is of reasonable complexity: all four channels, a wave
/// instrument and a wave change, noise drums, a subpattern, the timer, more
/// than one order, and every effect group but 6 in the orders it plays.
#[test]
fn the_test_song_uses_every_effect_but_6xx() {
    let song = parse_text(TEST_SONG).unwrap();
    assert!(song.orders[0].len() >= 2, "an order change");
    assert_ne!(song.timer_enabled, 0, "a timer tempo");
    assert!(
        song.instruments.iter().any(|i| i.subpattern_enabled != 0),
        "a subpattern"
    );
    let mut effects = [false; 16];
    for (channel, order) in song.orders.iter().enumerate() {
        let mut notes = 0;
        for key in order {
            for c in song.pattern(*key).unwrap().rows {
                notes += usize::from(c.note != gbcli_music::NO_NOTE);
                if c.effect_code != 0 || c.effect_param != 0 {
                    effects[c.effect_code as usize] = true;
                }
            }
        }
        assert!(notes > 0, "channel {} plays", channel + 1);
    }
    let missing: Vec<String> = (0..16)
        .filter(|&e| e != 6 && !effects[e])
        .map(|e| format!("{e:X}"))
        .collect();
    assert!(missing.is_empty(), "effects never used: {missing:?}");
    assert!(
        !effects[6],
        "6xx diverges from hUGETracker's export by design"
    );
}

/// The test song through our emitter and through hUGETracker's export
/// (committed, so this needs no Free Pascal) writes the same sound
/// registers over its whole length.
#[test]
fn the_test_song_matches_hugetrackers_export_over_its_whole_length() {
    let song = parse_text(TEST_SONG).unwrap();
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(TEST_SONG_EXPORT);
    let export = fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "{} ({e}): export the test song with `gbcli song \
             crates/build/tests/data/every_effect_but_6xx.song --uge SONG.uge`, then \
             `scripts/music/export-reference.sh --out DIR SONG.uge`",
            path.display()
        )
    });
    matches_reference("every_effect_but_6xx", &song, &export, Length::Whole);
}

/// The emitter's output doesn't depend on where a pattern is stored: the
/// song with its patterns stored in reverse plays the same writes for its
/// first two orders, timer and all (its whole length is compared with
/// hUGETracker's export above).
#[test]
fn the_test_song_plays_the_same_whatever_its_storage_order() {
    let temp = Temp::new("equiv-storage");
    let song = parse_text(TEST_SONG).unwrap();
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
    let song = parse_text(TEST_SONG).unwrap();
    let mut changed = song.clone();
    let p5 = changed.patterns.iter_mut().find(|p| p.key == 5).unwrap();
    assert_eq!(p5.rows[32].note, 11, "B-3");
    p5.rows[32].note = 12;
    let a = rom_with_asm(&temp, "equiv-note-a", &rgbds_asm(&song, "song_theme"));
    let b = rom_with_asm(&temp, "equiv-note-b", &rgbds_asm(&changed, "song_theme"));
    // Up to row 100, just past the change.
    let Verdict::Diverged(d) = same(&a, &b, frames(&song, 100)) else {
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
    // player's first write (MusicInit turning the APU on). The divergence's
    // ticks count from the handover, the write log's from power-on.
    let handover = booted(&a).core().now().0;
    // Three frames: the starter draws its line of text, with the LCD off
    // for most of a frame, before MusicInit (leaf RDlaLo).
    let start = player::play(&a, 3).writes[0].at.0;
    let at = (handover + d.ticks - start) as f64 / f64::from(1 << 23);
    assert!((5.95..6.1).contains(&at), "{at} s");
}
