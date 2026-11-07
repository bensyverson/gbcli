//! Note names, hUGETracker's period table and its effect codes.

use crate::model::HIGHEST_NOTE;

const NAMES: [&str; 12] = [
    "C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-",
];

/// hUGETracker's spelling of note `note` (0 is C-3, 71 is B-8), or `None`
/// outside that range.
pub fn note_name(note: i32) -> Option<String> {
    (0..=HIGHEST_NOTE)
        .contains(&note)
        .then(|| format!("{}{}", NAMES[note as usize % 12], 3 + note / 12))
}

/// The note a spelling such as `C#5` names.
pub fn note_of(name: &str) -> Option<i32> {
    let (pitch, octave) = name.split_at_checked(2)?;
    let index = NAMES.iter().position(|n| *n == pitch)? as i32;
    let octave: i32 = match octave.as_bytes() {
        [d @ b'3'..=b'8'] => i32::from(d - b'0'),
        _ => return None,
    };
    Some((octave - 3) * 12 + index)
}

/// The tone channels' period register value for each note
/// (constants.pas `NotesToFreqs`); the driver plays these.
const PERIODS: [u16; 72] = [
    44, 156, 262, 363, 457, 547, 631, 710, 786, 854, 923, 986, 1046, 1102, 1155, 1205, 1253, 1297,
    1339, 1379, 1417, 1452, 1486, 1517, 1546, 1575, 1602, 1627, 1650, 1673, 1694, 1714, 1732, 1750,
    1767, 1783, 1798, 1812, 1825, 1837, 1849, 1860, 1871, 1881, 1890, 1899, 1907, 1915, 1923, 1930,
    1936, 1943, 1949, 1954, 1959, 1964, 1969, 1974, 1978, 1982, 1985, 1988, 1992, 1995, 1998, 2001,
    2004, 2006, 2009, 2011, 2013, 2015,
];

/// The period hUGEDriver writes for `note` on a tone channel.
pub fn period_of(note: i32) -> Option<u16> {
    usize::try_from(note)
        .ok()
        .and_then(|i| PERIODS.get(i))
        .copied()
}

/// One of hUGETracker's effect codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Effect {
    pub code: u8,
    pub name: &'static str,
    /// The cell spelling, with x and y for the parameter's nibbles.
    pub spelling: &'static str,
    pub meaning: &'static str,
}

/// Every effect hUGEDriver plays, by code: the names from hUGETracker's
/// utils.pas `EffectToExplanation`, the parameters as hUGEDriver.asm
/// (727ab8e, the driver hUGETracker 3fedced3 pins) reads them.
pub const EFFECTS: &[Effect] = &[
    Effect {
        code: 0x0,
        name: "arpeggio",
        spelling: "0xy",
        meaning: "cycle the note, +x and +y semitones each tick",
    },
    Effect {
        code: 0x1,
        name: "slide up",
        spelling: "1xx",
        meaning: "raise the period by xx each tick",
    },
    Effect {
        code: 0x2,
        name: "slide down",
        spelling: "2xx",
        meaning: "lower the period by xx each tick",
    },
    Effect {
        code: 0x3,
        name: "tone portamento",
        spelling: "3xx",
        meaning: "glide toward this row's note by xx each tick",
    },
    Effect {
        code: 0x4,
        name: "vibrato",
        spelling: "4xy",
        meaning: "wobble the pitch: x speed, y depth",
    },
    Effect {
        code: 0x5,
        name: "master volume",
        spelling: "5xy",
        meaning: "set NR50 to xx: the left (x) and right (y) output volume, 0-7 each",
    },
    Effect {
        code: 0x6,
        name: "call routine",
        spelling: "6xx",
        meaning: "run routine xx (a `routine` block) each tick",
    },
    Effect {
        code: 0x7,
        name: "note delay",
        spelling: "7xx",
        meaning: "start the note xx ticks into the row",
    },
    Effect {
        code: 0x8,
        name: "panning",
        spelling: "8xx",
        meaning: "set NR51 (which channels reach which speaker) to xx",
    },
    Effect {
        code: 0x9,
        name: "duty cycle",
        spelling: "9xx",
        meaning: "square: set NRx1 to xx (00 12.5%, 40 25%, 80 50%, C0 75%); wave: switch to \
                  wave xx; noise: xx's bit 3 sets the 7-bit mode",
    },
    Effect {
        code: 0xA,
        name: "volume slide",
        spelling: "Axy",
        meaning: "retrigger the note with its volume raised by x or lowered by y (on the row's \
                  first tick)",
    },
    Effect {
        code: 0xB,
        name: "position jump",
        spelling: "Bxx",
        meaning: "after this row, go to order xx-1 (B01 is order 00); B00 goes to the next \
                  order",
    },
    Effect {
        code: 0xC,
        name: "set volume",
        spelling: "Cxy",
        meaning: "set the volume to y; x 0 keeps the envelope, 1-7 fades down at pace x, 8 holds, \
                  9-F fades up at pace x-8",
    },
    Effect {
        code: 0xD,
        name: "pattern break",
        spelling: "Dxx",
        meaning: "after this row, go to the next order, starting at row xx-1 (D01 starts at \
                  row 00); D00 does nothing",
    },
    Effect {
        code: 0xE,
        name: "note cut",
        spelling: "Exx",
        meaning: "silence the note after xx ticks",
    },
    Effect {
        code: 0xF,
        name: "set speed",
        spelling: "Fxx",
        meaning: "set the ticks per row to xx",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_names_round_trip_over_the_whole_range() {
        assert_eq!(note_name(0).as_deref(), Some("C-3"));
        assert_eq!(note_name(25).as_deref(), Some("C#5"));
        assert_eq!(note_name(71).as_deref(), Some("B-8"));
        assert_eq!(note_name(72), None);
        assert_eq!(note_name(-1), None);
        for n in 0..=71 {
            assert_eq!(note_of(&note_name(n).unwrap()), Some(n));
        }
        assert_eq!(note_of("C-9"), None);
        assert_eq!(note_of("H-4"), None);
        assert_eq!(note_of("C-"), None);
    }

    #[test]
    fn every_code_has_one_effect() {
        for (i, e) in EFFECTS.iter().enumerate() {
            assert_eq!(usize::from(e.code), i);
        }
        assert_eq!(EFFECTS.len(), 16);
    }
}
