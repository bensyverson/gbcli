//! The song → RGBDS `.asm` emitter, a port of hUGETracker's
//! `RenderSongToRGBDSAsm` (codegen.pas at 3fedced3, with `OptimizeSong`,
//! `FindUsedStuff`, `InstrumentToBytes` and the cell renderers it calls).
//! Each test builds a small song and checks the lines hUGETracker's code
//! would write for it; that the whole file plays the same as hUGETracker's
//! export is checked by register writes in `crates/build/tests/it/music_equiv.rs`.

use gbcli_music::{
    BLANK_ROWS, Cell, EXAMPLE, NO_NOTE, Pattern, Song, descriptor_for, global_symbols, parse_text,
    rgbds_asm,
};

fn text(song: &Song, descriptor: &str) -> String {
    String::from_utf8(rgbds_asm(song, descriptor)).unwrap()
}

fn lines(song: &Song) -> Vec<String> {
    text(song, "song_t").lines().map(str::to_string).collect()
}

/// The lines after the first line equal to `label`, up to the next blank.
fn block(song: &Song, label: &str) -> Vec<String> {
    let all = lines(song);
    let at = all
        .iter()
        .position(|l| l == label)
        .unwrap_or_else(|| panic!("no {label} in:\n{}", all.join("\n")));
    all[at + 1..]
        .iter()
        .take_while(|l| !l.is_empty())
        .cloned()
        .collect()
}

fn cell(note: i32, instrument: i32, code: i32, param: u8) -> Cell {
    Cell {
        note,
        instrument,
        volume: 0,
        effect_code: code,
        effect_param: param,
    }
}

/// A song whose four channels play `keys` once each, from blank patterns.
fn song(keys: [i32; 4]) -> Song {
    let mut s = Song::default();
    for key in keys {
        if s.pattern(key).is_none() {
            s.patterns.push(Pattern {
                key,
                rows: BLANK_ROWS,
            });
        }
    }
    s.orders = keys.map(|k| vec![k]);
    s
}

fn set(s: &mut Song, key: i32, row: usize, c: Cell) {
    s.patterns.iter_mut().find(|p| p.key == key).unwrap().rows[row] = c;
}

#[test]
fn the_descriptor_points_at_every_table_and_holds_the_speed() {
    let mut s = song([0, 1, 2, 3]);
    s.ticks_per_row = 6;
    let out = text(&s, "song_theme");
    let head: Vec<&str> = out.lines().take(12).collect();
    assert_eq!(
        head,
        [
            "include \"hUGE.inc\"",
            "",
            "SECTION \"song_theme Song Data\", ROMX",
            "",
            "song_theme::",
            "db 6",
            "dw song_theme.order_cnt",
            "dw song_theme.order1, song_theme.order2, song_theme.order3, song_theme.order4",
            "dw song_theme.duty_instruments, song_theme.wave_instruments, \
             song_theme.noise_instruments",
            "dw song_theme.routines",
            "dw song_theme.waves",
            "",
        ]
    );
}

#[test]
fn orders_count_two_per_order_and_name_the_first_identical_pattern() {
    // P5 and P9 hold the same rows, so every order naming P9 names P5
    // (song.pas `OptimizeSong`), and only P5 is written.
    let mut s = song([5, 9, 2, 3]);
    s.orders = [vec![5, 9], vec![9, 9], vec![2, 2], vec![3, 3]];
    let all = lines(&s);
    assert!(
        all.contains(&"song_t.order_cnt: db 4".to_string()),
        "{all:?}"
    );
    assert!(all.contains(&"song_t.order1: dw song_t.P5,song_t.P5".to_string()));
    assert!(all.contains(&"song_t.order2: dw song_t.P5,song_t.P5".to_string()));
    assert!(all.contains(&"song_t.P5:".to_string()));
    assert!(!all.contains(&"song_t.P9:".to_string()));
}

#[test]
fn patterns_differing_only_in_the_unused_volume_stay_apart() {
    let mut s = song([0, 1, 2, 3]);
    set(
        &mut s,
        1,
        0,
        Cell {
            volume: 5,
            ..Cell::BLANK
        },
    );
    let all = lines(&s);
    assert!(all.contains(&"song_t.order2: dw song_t.P1".to_string()));
    assert!(all.contains(&"song_t.P1:".to_string()));
}

#[test]
fn a_cell_is_a_dn_line_with_the_note_instrument_and_effect() {
    let mut s = song([0, 1, 2, 3]);
    set(&mut s, 0, 0, cell(24, 1, 0xC, 0x0F)); // C-5, instrument 1, C0F
    set(&mut s, 0, 1, cell(13, 16, 0x4, 0x21)); // instrument 16 → 0
    set(&mut s, 0, 2, cell(72, 0, 0, 0)); // past B-8 → no note
    let rows = block(&s, "song_t.P0:");
    assert_eq!(rows.len(), 64);
    assert_eq!(rows[0], " dn C_5,1,$C0F");
    assert_eq!(rows[1], " dn C#4,0,$421");
    assert_eq!(rows[2], " dn ___,0,$000");
    assert_eq!(rows[3], " dn ___,0,$000");
}

#[test]
fn a_square_instrument_is_its_sweep_length_envelope_table_and_high_mask() {
    let mut s = song([0, 1, 2, 3]);
    set(&mut s, 0, 0, cell(24, 2, 0, 0));
    let i = &mut s.instruments[1]; // square 2
    i.sweep_time = 3;
    i.sweep_direction = 1; // down
    i.sweep_shift = 5;
    i.duty = 1;
    i.length = 20;
    i.length_enabled = 1;
    i.initial_volume = 12;
    i.volume_direction = 0; // up
    i.volume_pace = 2;
    let insts = block(&s, "song_t.duty_instruments:");
    // Square 1 is hUGETracker's default: no sweep, 50%, volume 15 down.
    assert_eq!(
        insts,
        [
            "song_t.itSquareinst1:",
            "db 8",
            "db 128",
            "db 240",
            "dw 0",
            "db 128",
        ]
    );
    let two = block(&s, "song_t.itSquareinst2:");
    assert_eq!(
        two,
        [
            &format!("db {}", (3 << 4) | 8 | 5),
            &format!("db {}", (1 << 6) | 20),
            &format!("db {}", (12 << 4) | 8 | 2),
            "dw 0",
            &format!("db {}", 0x80 | 0x40),
        ]
    );
}

#[test]
fn a_wave_instrument_is_its_length_level_waveform_table_and_high_mask() {
    let mut s = song([0, 1, 2, 3]);
    set(&mut s, 2, 0, cell(24, 1, 0, 0));
    let i = &mut s.instruments[15]; // wave 1
    i.length = 200;
    i.output_level = 2;
    i.waveform = 4;
    assert_eq!(
        block(&s, "song_t.itWaveinst1:"),
        ["db 200", "db 64", "db 4", "dw 0", "db 128"]
    );
}

#[test]
fn a_noise_instrument_is_its_envelope_table_mask_and_two_spare_bytes() {
    let mut s = song([0, 1, 2, 3]);
    set(&mut s, 3, 0, cell(24, 1, 0, 0));
    let i = &mut s.instruments[30]; // noise 1
    i.length = 10;
    i.length_enabled = 1;
    i.counter_step = 1; // 7-bit
    i.initial_volume = 9;
    i.volume_direction = 1; // down
    i.volume_pace = 3;
    i.subpattern_enabled = 1;
    assert_eq!(
        block(&s, "song_t.itNoiseinst1:"),
        [
            &format!("db {}", (9 << 4) | 3),
            "dw song_t.itNoiseSP1",
            &format!("db {}", 0x80 | 0x40 | 10),
            "ds 2",
        ]
    );
}

#[test]
fn a_subpattern_is_its_first_32_rows_and_the_last_jumps_to_row_1() {
    let mut s = song([0, 1, 2, 3]);
    set(&mut s, 0, 0, cell(24, 1, 0, 0));
    let i = &mut s.instruments[0];
    i.subpattern_enabled = 1;
    i.subpattern[0] = cell(36, 0, 0, 0);
    i.subpattern[1] = Cell {
        volume: 1,
        ..cell(NO_NOTE, 0, 0xC, 0x08)
    };
    i.subpattern[40] = cell(40, 0, 0, 0); // never exported
    let rows = block(&s, "song_t.itSquareSP1:");
    assert_eq!(rows.len(), 32);
    assert_eq!(rows[0], " dn 36,0,$000");
    assert_eq!(rows[1], " dn ___,1,$C08");
    assert_eq!(rows[30], " dn ___,0,$000");
    assert_eq!(rows[31], " dn ___,1,$000");
    assert!(block(&s, "song_t.itSquareinst1:").contains(&"dw song_t.itSquareSP1".to_string()));
}

#[test]
fn instruments_are_written_up_to_the_highest_each_bank_plays() {
    let mut s = song([0, 1, 2, 3]);
    set(&mut s, 1, 0, cell(24, 3, 0, 0));
    let all = lines(&s);
    for n in 1..=3 {
        assert!(
            all.contains(&format!("song_t.itSquareinst{n}:")),
            "square {n}"
        );
    }
    assert!(!all.contains(&"song_t.itSquareinst4:".to_string()));
    assert!(!all.iter().any(|l| l.starts_with("song_t.itWaveinst")));
    assert!(!all.iter().any(|l| l.starts_with("song_t.itNoiseinst")));
}

#[test]
fn a_pattern_two_channels_share_counts_only_for_the_first_that_plays_it() {
    // hUGETracker's FindUsedStuff visits each pattern once, for the first
    // channel whose orders name it, so instrument 4 in P0 counts as a square
    // instrument (channel 1) and never as a wave one (channel 3).
    let mut s = song([0, 1, 0, 3]);
    set(&mut s, 0, 0, cell(24, 4, 0, 0));
    let all = lines(&s);
    assert!(all.contains(&"song_t.itSquareinst4:".to_string()));
    assert!(!all.iter().any(|l| l.starts_with("song_t.itWaveinst")));
}

#[test]
fn waves_are_written_up_to_the_highest_any_wave_source_names() {
    let mut s = song([0, 1, 2, 3]);
    // Wave instrument 2 plays wave 3; a 9xx on the wave channel names wave 6.
    set(&mut s, 2, 0, cell(24, 2, 0, 0));
    s.instruments[16].waveform = 3;
    let all = lines(&s);
    assert!(all.iter().any(|l| l.starts_with("song_t.wave3: db ")));
    assert!(!all.iter().any(|l| l.starts_with("song_t.wave4:")));
    set(&mut s, 2, 1, cell(NO_NOTE, 0, 9, 6));
    let all = lines(&s);
    assert!(all.iter().any(|l| l.starts_with("song_t.wave6: db ")));
    // A 9xx in any wave instrument's subpattern counts, played or not.
    s.instruments[20].subpattern_enabled = 1;
    s.instruments[20].subpattern[3] = cell(NO_NOTE, 0, 9, 9);
    let all = lines(&s);
    assert!(all.iter().any(|l| l.starts_with("song_t.wave9: db ")));
}

#[test]
fn a_wave_is_16_bytes_of_two_samples_each() {
    let mut s = song([0, 1, 2, 3]);
    set(&mut s, 2, 0, cell(24, 1, 0, 0)); // wave 1 plays wave 0
    let w0 = lines(&s)
        .into_iter()
        .find(|l| l.starts_with("song_t.wave0: "))
        .unwrap();
    // hUGETracker's default wave 0: 0000ffff… → $00, $00, $FF, …
    let bytes: Vec<String> = std::iter::repeat_n("0".to_string(), 2)
        .chain(std::iter::repeat_n("255".to_string(), 14))
        .collect();
    assert_eq!(w0, format!("song_t.wave0: db {}", bytes.join(",")));
}

/// Each routine opens a global label of its own, unique to the song, so a
/// routine's local labels (`.loop`) are scoped to it, as in hUGETracker's
/// export, where each opens `__hUGE_Routine_N:`.
#[test]
fn every_routine_is_written_under_its_own_global_label_with_a_ret() {
    let mut s = song([0, 1, 2, 3]);
    s.routines[2] = b"  ld a, 1\n  ld [$C000], a".to_vec();
    let all = lines(&s);
    let at = all
        .iter()
        .position(|l| l == "song_t__hUGE_Routine_2:")
        .unwrap();
    assert_eq!(
        all[at..at + 5],
        [
            "song_t__hUGE_Routine_2:",
            "  ld a, 1",
            "  ld [$C000], a",
            "song_t__end_hUGE_Routine_2:",
            "ret",
        ]
    );
    assert!(all.contains(&"song_t__hUGE_Routine_15:".to_string()));
}

/// hUGEDriver's 6xx reads a pointer from `routines + 2 * x` and jumps to
/// it, so `routines:` is a table of the 16 routines' addresses after the
/// routines, as hUGETracker's own preview builds it (hUGEDriver's
/// `song.asm`), not the routines' code as its RGBDS export writes it.
#[test]
fn routines_is_a_table_of_the_sixteen_routines_addresses() {
    let s = song([0, 1, 2, 3]);
    let all = lines(&s);
    let table = block(&s, "song_t.routines:");
    let names = |range: std::ops::Range<usize>| {
        let n: Vec<String> = range.map(|i| format!("song_t__hUGE_Routine_{i}")).collect();
        format!("dw {}", n.join(", "))
    };
    assert_eq!(table, [names(0..8), names(8..16)]);
    let at = |l: &str| all.iter().position(|x| x == l).unwrap();
    assert!(at("song_t__end_hUGE_Routine_15:") < at("song_t.routines:"));
    assert!(at("song_t.routines:") < at("song_t.waves:"));
}

/// Every global symbol a song's `.asm` defines, for `build`'s check that no
/// two songs define one name.
#[test]
fn a_songs_global_symbols_are_its_descriptor_constants_and_routines() {
    let symbols = global_symbols("song_t");
    assert_eq!(symbols.len(), 3 + 32);
    for name in ["song_t", "song_t_TIMER", "song_t_TMA"] {
        assert!(symbols.contains(&name.to_string()), "{name}");
    }
    let out = text(&song([0, 1, 2, 3]), "song_t");
    for name in &symbols {
        let defined = out.contains(&format!("\n{name}:")) || out.contains(&format!("DEF {name} "));
        assert!(defined || out.contains(&format!("\n{name}::")), "{name}");
    }
}

#[test]
fn the_timer_setting_is_exported_beside_the_descriptor() {
    let mut s = song([0, 1, 2, 3]);
    let out = text(&s, "song_theme");
    assert!(out.contains("DEF song_theme_TIMER EQU 0\n"), "{out}");
    s.timer_enabled = 1;
    s.timer_divider = 200;
    let out = text(&s, "song_theme");
    assert!(out.contains("DEF song_theme_TIMER EQU 1\n"));
    assert!(out.contains("DEF song_theme_TMA EQU 200\n"));
    assert!(out.contains("EXPORT song_theme_TIMER, song_theme_TMA\n"));
}

#[test]
fn shorter_orders_are_padded_with_pattern_0_as_hugetracker_loads_them() {
    let mut s = song([0, 1, 2, 3]);
    s.orders[3] = vec![3, 3];
    let all = lines(&s);
    assert!(all.contains(&"song_t.order_cnt: db 4".to_string()));
    assert!(all.contains(&"song_t.order1: dw song_t.P0,song_t.P0".to_string()));
}

#[test]
fn a_descriptor_is_song_and_the_file_stem_as_a_symbol() {
    assert_eq!(descriptor_for("theme"), "song_theme");
    assert_eq!(descriptor_for("Tempest - Kekri"), "song_Tempest___Kekri");
    assert_eq!(descriptor_for("2nd.boss"), "song_2nd_boss");
}

#[test]
fn the_example_song_emits_its_golden() {
    let song = parse_text(EXAMPLE).unwrap();
    let out = rgbds_asm(&song, "song_example");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/example.asm");
    let actual = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("example.actual.asm");
    if std::env::var_os("GBCLI_BLESS").is_some() {
        std::fs::write(&path, &out).unwrap();
    }
    let want = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("{}: {e}; bless with GBCLI_BLESS=1", path.display()));
    if out != want {
        std::fs::write(&actual, &out).unwrap();
        panic!(
            "the example's .asm differs from {}; wrote {}",
            path.display(),
            actual.display()
        );
    }
}
