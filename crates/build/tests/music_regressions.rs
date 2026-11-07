//! Regression tests for the review of the music build loop (1ffe1f6, leaf
//! pruzN6): each was a probe, red on that commit, and names what it checks.
//! Needs the pinned RGBDS on `PATH`.

mod common;
mod player;

use std::fs;

use gbcli_build::{BuildError, build};
use gbcli_music::{EXAMPLE, NO_NOTE, Pattern, parse_text, to_text};
use gbcli_protocol::ErrorCode;

use common::{rgbds, starter};

/// Two routines that each use a local label: in hUGETracker's export each
/// routine opens a global label (`__hUGE_Routine_0:`), so `.loop` is
/// `__hUGE_Routine_0.loop` and `__hUGE_Routine_1.loop`. Scoped to the
/// descriptor, both became `song_theme.loop`.
#[test]
fn two_routines_with_the_same_local_label_build() {
    let (temp, options) = starter("regress-routines");
    let path = temp.path("demo/assets/music/theme.song");
    let text = fs::read_to_string(&path).unwrap();
    let routines =
        "ticks_per_row 6\n\nroutine 0\n| .loop\n|   ret\nend\n\nroutine 1\n| .loop\n|   ret\nend\n";
    fs::write(&path, text.replace("ticks_per_row 6\n", routines)).unwrap();
    let made = build(&rgbds(), &options);
    assert!(made.is_ok(), "{:?}", made.err().map(|e| e.body().message));
}

/// A second song whose stem has a dot (`theme.v2.song`) beside the
/// starter's `theme.song`: their labels differ (song_theme, song_theme_v2),
/// and each gets its own `.asm` (both used to be written to
/// build/music/theme.asm).
#[test]
fn a_dotted_stem_gets_its_own_asm() {
    let (temp, options) = starter("regress-dotted");
    let text = fs::read_to_string(temp.path("demo/assets/music/theme.song")).unwrap();
    fs::write(temp.path("demo/assets/music/theme.v2.song"), text).unwrap();
    let made = match build(&rgbds(), &options) {
        Ok(m) => m,
        Err(e) => panic!("build failed: {}", e.body().message),
    };
    let asms: Vec<&str> = made.songs.iter().map(|s| s.asm.as_str()).collect();
    assert_eq!(
        asms,
        [
            "demo/build/music/theme.asm",
            "demo/build/music/theme.v2.asm"
        ]
    );
}

/// The example song with `extra` more orders of distinct patterns appended,
/// so its data grows by about 4 × 192 bytes an order.
fn grown(extra: i32, salt: i32) -> String {
    let mut song = parse_text(EXAMPLE).unwrap();
    // A salted song plays other notes from its first row.
    for p in &mut song.patterns {
        for c in &mut p.rows {
            if c.note != NO_NOTE && salt != 0 {
                c.note = (c.note + 1) % 72;
            }
        }
    }
    let base = song.patterns[0].rows;
    for o in 0..extra {
        for c in 0..4 {
            let key = 100 + o * 4 + c;
            let mut rows = base;
            rows[1].note = (o * 4 + c + salt) % 72;
            rows[2].note = salt % 72;
            song.patterns.push(Pattern { key, rows });
            song.orders[c as usize].push(key);
        }
    }
    to_text(&song)
}

/// The starter is ROM-only (no MBC) and each song is a ROMX section. Two
/// songs that don't fit bank 1 together put one in bank 2, which a ROM-only
/// cartridge can't map: hUGEDriver then read bank 1's bytes as the song,
/// silently. The build now fails, naming the section past bank 1, and
/// leaves no ROM.
#[test]
fn songs_past_bank_1_of_a_rom_only_cart_fail_the_build() {
    let (temp, options) = starter("regress-banks");
    fs::write(temp.path("demo/assets/music/theme.song"), grown(11, 0)).unwrap();
    fs::write(temp.path("demo/assets/music/big.song"), grown(16, 7)).unwrap();
    let err = build(&rgbds(), &options).unwrap_err();
    assert!(matches!(err, BuildError::Failed { .. }), "{err:?}");
    let body = err.body();
    assert_eq!(body.code, ErrorCode::BuildFailed);
    assert!(
        body.message.contains("song_theme Song Data"),
        "{}",
        body.message
    );
    let overflow = body.result.unwrap().build.unwrap().overflow.unwrap();
    let names: Vec<&str> = overflow.sections.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["song_theme Song Data"]);
    assert_eq!(overflow.sections[0].bank, 2);
    assert!(!temp.path("demo/build/demo.gb").exists());
}

/// Effect 600 calls routine 0. hUGEDriver's `fx_call_routine` reads a
/// pointer from `routines + 2 * n`; the RGBDS export wrote the routines'
/// code at `routines`, so the driver jumped to an address made of the
/// routine's first two code bytes. The routine here stores $42 at $DF00.
#[test]
fn effect_6xx_runs_its_routine() {
    let (temp, options) = starter("regress-6xx");
    let path = temp.path("demo/assets/music/theme.song");
    let text = fs::read_to_string(&path).unwrap();
    let text = text.replace(
        "ticks_per_row 6\n",
        "ticks_per_row 6\n\nroutine 0\n|   ld a, $42\n|   ld [$DF00], a\nend\n",
    );
    let text = text.replacen("00 C-5 01 ... ...", "00 C-5 01 ... 600", 1);
    fs::write(&path, text).unwrap();
    build(&rgbds(), &options).unwrap();
    let rom = fs::read(temp.path("demo/build/demo.gb")).unwrap();
    let mut m = player::machine(&rom, None);
    let core = m.core_mut();
    core.run(&mut gbcli_core::RunRequest::new(
        gbcli_core::Budget::frames(400),
    ))
    .expect("run");
    let sym = fs::read_to_string(temp.path("demo/build/demo.sym")).unwrap();
    let wframe = sym
        .lines()
        .find(|l| l.ends_with(" wFrame"))
        .and_then(|l| u16::from_str_radix(&l[3..7], 16).ok())
        .unwrap();
    let frames = u16::from(core.bus_read(wframe).unwrap())
        | (u16::from(core.bus_read(wframe + 1).unwrap()) << 8);
    assert!(frames > 300, "the game kept running: wFrame {frames}");
    assert_eq!(core.bus_read(0xDF00).unwrap(), 0x42, "routine 0 never ran");
}

/// A song file named `theme_TIMER.song` beside `theme.song`: its label,
/// song_theme_TIMER, is also the constant theme's data exports. That used
/// to reach rgblink as a clash; the build now refuses it before any tool
/// runs, naming both files and the symbol.
#[test]
fn a_stem_that_clashes_with_another_songs_constant_is_refused() {
    let (temp, options) = starter("regress-timer-stem");
    let text = fs::read_to_string(temp.path("demo/assets/music/theme.song")).unwrap();
    fs::write(temp.path("demo/assets/music/theme_TIMER.song"), text).unwrap();
    match build(&rgbds(), &options) {
        Err(BuildError::Layout { message, remedy }) => {
            assert!(message.contains("theme.song"), "{message}");
            assert!(message.contains("theme_TIMER.song"), "{message}");
            assert!(message.contains("song_theme_TIMER"), "{message}");
            assert!(remedy.contains("rename"), "{remedy}");
        }
        other => panic!("expected a layout error, got {other:?}"),
    }
    assert!(!temp.path("demo/build/obj").exists());
}
