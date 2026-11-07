//! hUGETracker's sample songs, fetched with the test-ROM corpus and checked
//! against its SHA256SUMS, and the bytes a v6 round trip is allowed to change.
//!
//! Regenerate the goldens, after checking that a change is intended, with
//! `GBCLI_BLESS=1 cargo test -p gbcli-music`. Without it a mismatch fails
//! and writes the actual bytes beside the test's target directory.

#![allow(dead_code)] // each test binary uses a different subset

use std::path::PathBuf;

/// One sample song.
pub struct Sample {
    pub name: String,
    pub version: u32,
    pub bytes: Vec<u8>,
}

/// The `hugetracker-samples` corpus suite: each song's file name and the
/// `.uge` version it is saved in. The suite's provenance and license are in
/// `scripts/corpus/build_manifest.py`; why it is fetched rather than
/// committed is in `project/2026-11-06-test-rom-corpus.md`.
const SAMPLES: [(&str, u32); 15] = [
    ("Jester - FishNChips.uge", 1),
    ("Reed - Funkabeer's Revenge.uge", 1),
    ("SVL - UwU.uge", 3),
    ("Arachno - A Sad Touch.uge", 4),
    ("Maktone - Softworld.uge", 4),
    ("MelonadeM - Boatship for Rent.uge", 4),
    ("SVL - Yyna.uge", 4),
    ("raphaelgoulart - The Murderous Funk Machine (intro).uge", 4),
    ("Coffee Bat - Blue Ocean.uge", 5),
    ("Coffee Bat - Pilgrim's Peril Menu.uge", 5),
    ("Coffee Bat - Wyrmhole.uge", 5),
    ("Chavez - Monkeys on Mars.uge", 5),
    ("FADE - Microplastics in the Air.uge", 6),
    ("FADE - Strap in and Suit Up.uge", 6),
    ("Tempest - Kekri.uge", 6),
];

const SUITE: &str = "hugetracker-samples";

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

/// Every sample song, read from the corpus and checked against the SHA-256
/// `tests/corpus/SHA256SUMS` pins. Fails loudly, never skips.
pub fn samples() -> Vec<Sample> {
    let sums = std::fs::read_to_string(repo().join("tests/corpus/SHA256SUMS")).unwrap();
    SAMPLES
        .iter()
        .map(|&(name, version)| {
            let rel = format!("{SUITE}/{name}");
            let full = corpus_dir().join(&rel);
            let bytes = std::fs::read(&full).unwrap_or_else(|err| {
                panic!(
                    "sample song {} is missing ({err}). Fetch it from the repo root with:\n    \
                     python3 -I scripts/corpus/fetch.py --only {SUITE}\n\
                     (or point GBCLI_TEST_ROMS at a directory holding {rel})",
                    full.display()
                )
            });
            let sha = sums
                .lines()
                .find_map(|l| l.strip_suffix(rel.as_str())?.strip_suffix("  "))
                .unwrap_or_else(|| panic!("tests/corpus/SHA256SUMS has no line for {rel}"));
            assert_eq!(
                gbcli_engine::sha256(&bytes).to_string(),
                sha,
                "{rel} differs from the file SHA256SUMS pins"
            );
            Sample {
                name: name.to_string(),
                version,
                bytes,
            }
        })
        .collect()
}

const SHORT_STRING: usize = 256;
const INSTRUMENT: usize = 1385;
const HEADER: usize = 4 + 3 * SHORT_STRING;
/// The fixed part of a v6 file: through the timer divider.
const FIXED: usize = HEADER + 45 * INSTRUMENT + 16 * 32 + 4 + 1 + 4;
const PATTERN: usize = 4 + 64 * 17;

/// A v6 file with the bytes the format note in `gbcli_music::uge` exempts
/// zeroed: every `ShortString`'s bytes past its length, and each channel's
/// last (unread) order entry.
pub fn masked(file: &[u8]) -> Vec<u8> {
    let mut out = file.to_vec();
    let mut strings: Vec<usize> = (0..3).map(|i| 4 + i * SHORT_STRING).collect();
    strings.extend((0..45).map(|i| HEADER + i * INSTRUMENT + 4));
    for at in strings {
        let len = usize::from(out[at]);
        out[at + 1 + len..at + SHORT_STRING].fill(0);
    }
    let count = u32_at(&out, FIXED) as usize;
    let mut at = FIXED + 4 + count * PATTERN;
    for _ in 0..4 {
        let n = u32_at(&out, at) as usize;
        at += 4 + 4 * n;
        if n > 0 {
            out[at - 4..at].fill(0);
        }
    }
    out
}

/// How many bytes of `file` the mask changes: what the exemption covers.
pub fn exempt_bytes(file: &[u8]) -> usize {
    file.iter()
        .zip(masked(file))
        .filter(|(a, b)| **a != *b)
        .count()
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

/// Compares `bytes` with `tests/golden/<file>` byte for byte, or rewrites the
/// golden when `GBCLI_BLESS=1`.
pub fn assert_golden(file: &str, bytes: &[u8]) {
    let path = golden_dir().join(file);
    if std::env::var_os("GBCLI_BLESS").is_some_and(|v| v == "1") {
        std::fs::create_dir_all(golden_dir()).unwrap();
        std::fs::write(&path, bytes).unwrap();
        return;
    }
    let expected = std::fs::read(&path).unwrap_or_else(|err| {
        panic!(
            "golden {} is missing ({err}); after checking it, create it with\n    \
             GBCLI_BLESS=1 cargo test -p gbcli-music",
            path.display()
        )
    });
    if expected != bytes {
        let actual = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("actual-{file}"));
        std::fs::write(&actual, bytes).unwrap();
        panic!(
            "{file} differs from its golden; the actual bytes are at {}. If the change is \
             intended, regenerate with GBCLI_BLESS=1 cargo test -p gbcli-music",
            actual.display()
        );
    }
}
