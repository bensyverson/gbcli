//! Hostile input (review-music, 2026-11-07): cut and bit-flipped sample
//! `.uge` files and mutated text never panic, and whatever reads survives
//! text and `.uge` unchanged and draws a piano roll.

use crate::common;

use common::samples;
use gbcli_music::{Song, parse_text, piano_roll, read_song, read_uge, to_text, write_uge};

/// xorshift64: a fixed sequence, so a failure reproduces.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// What must hold for any song a reader returns.
#[track_caller]
fn check_song(song: &Song, what: &str) {
    let text = to_text(song);
    let parsed = parse_text(&text).unwrap_or_else(|e| panic!("{what}: own text refused: {e}"));
    assert!(parsed == *song, "{what}: text changed the song");
    let back =
        read_uge(&write_uge(song)).unwrap_or_else(|e| panic!("{what}: own uge refused: {e}"));
    assert!(back.song == *song, "{what}: .uge changed the song");
    let _ = piano_roll(song);
}

/// Every sample cut at each byte below 4096 that is a multiple of `step`,
/// and every 61st byte after.
fn cut_samples(step: usize) {
    for s in samples() {
        let mut cut = 0;
        while cut < s.bytes.len() {
            let _ = read_song(&s.bytes[..cut]);
            cut += if cut < 4096 { step } else { 61 };
        }
    }
}

#[test]
fn a_cut_sample_never_panics() {
    cut_samples(7);
}

#[test]
#[ignore = "pre-release: every byte of the header cut, about 4 s"]
fn a_sample_cut_at_any_header_byte_never_panics() {
    cut_samples(1);
}

#[test]
fn a_bit_flipped_sample_never_panics_and_what_reads_survives_both_forms() {
    flip_samples(12);
}

#[test]
#[ignore = "pre-release: 300 flips of each sample, about 40 s"]
fn many_bit_flipped_samples_never_panic_and_what_reads_survives_both_forms() {
    flip_samples(300);
}

/// `rounds` copies of each sample with one to four bits flipped.
fn flip_samples(rounds: usize) {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for s in samples() {
        for i in 0..rounds {
            let mut b = s.bytes.clone();
            for _ in 0..1 + rng.below(4) {
                let at = 4 + rng.below(b.len() - 4);
                b[at] ^= 1 << rng.below(8);
            }
            if let Ok(loaded) = read_song(&b) {
                check_song(&loaded.song, &format!("{} flip {i}", s.name));
            }
        }
    }
}

#[test]
fn mutated_text_never_panics_and_what_parses_survives_both_forms() {
    let mut rng = Rng(12345);
    let s = &samples()[12];
    let chars: Vec<char> = to_text(&read_uge(&s.bytes).unwrap().song).chars().collect();
    let pool = [
        '|', '"', '\\', '!', '-', ' ', '\n', 'x', '9', 'P', '#', '+', 'é',
    ];
    for _ in 0..400 {
        let mut c = chars.clone();
        for _ in 0..1 + rng.below(3) {
            let at = rng.below(c.len());
            match rng.below(3) {
                0 => c[at] = pool[rng.below(pool.len())],
                1 => {
                    c.remove(at);
                }
                _ => c.insert(at, pool[rng.below(pool.len())]),
            }
        }
        let text: String = c.into_iter().collect();
        if let Ok(song) = parse_text(&text) {
            check_song(&song, "mutated text");
        }
    }
}
