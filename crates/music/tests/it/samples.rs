//! hUGETracker's sample songs through every conversion.

use crate::common;

use common::{exempt_bytes, masked, samples};
use gbcli_music::{FORMAT_VERSION, parse_text, read_uge, to_text, write_uge};

#[test]
fn the_samples_cover_every_version_but_2() {
    let mut versions: Vec<u32> = samples().iter().map(|s| s.version).collect();
    versions.sort();
    versions.dedup();
    assert_eq!(versions, [1, 3, 4, 5, 6]);
    for s in samples() {
        assert_eq!(read_uge(&s.bytes).unwrap().version, s.version, "{}", s.name);
    }
}

#[test]
fn text_is_lossless_and_text_to_uge_to_text_is_a_fixpoint() {
    for s in samples() {
        let song = read_uge(&s.bytes).unwrap().song;
        let text = to_text(&song);
        let parsed = parse_text(&text).unwrap_or_else(|e| panic!("{}: {e}", s.name));
        assert!(parsed == song, "{}: the text lost something", s.name);
        let uge = write_uge(&parsed);
        let again = read_uge(&uge).unwrap();
        assert_eq!(again.version, FORMAT_VERSION);
        assert!(
            to_text(&again.song) == text,
            "{}: text -> uge -> text changed",
            s.name
        );
    }
}

#[test]
fn a_v6_sample_through_text_and_back_matches_byte_for_byte_but_the_exempt_bytes() {
    let mut exempt = 0;
    for s in samples().into_iter().filter(|s| s.version == 6) {
        let song = parse_text(&to_text(&read_uge(&s.bytes).unwrap().song)).unwrap();
        let written = write_uge(&song);
        assert_eq!(written.len(), s.bytes.len(), "{}", s.name);
        assert!(
            masked(&written) == masked(&s.bytes),
            "{}: differs outside the exempt bytes",
            s.name
        );
        // What the writer clears is exactly what the mask covers.
        assert_eq!(exempt_bytes(&written), 0, "{}", s.name);
        exempt += exempt_bytes(&s.bytes);
    }
    // The samples do carry string-tail residue, so the exemption is needed.
    assert!(exempt > 0);
}

#[test]
fn an_older_sample_is_written_as_v6_and_reads_back_unchanged() {
    for s in samples().into_iter().filter(|s| s.version < 6) {
        let song = read_uge(&s.bytes).unwrap().song;
        let written = write_uge(&song);
        assert_eq!(
            u32::from_le_bytes(written[..4].try_into().unwrap()),
            FORMAT_VERSION
        );
        assert!(read_uge(&written).unwrap().song == song, "{}", s.name);
    }
}
