//! Periods to frequencies, and frequencies to the nearest note.

use std::fmt;

use gbcli_core::ApuChannel;

/// The square channels' frequency is this over (2048 - period), in Hz.
const SQUARE_CLOCK: f64 = 131_072.0;
/// The wave channel steps through 32 samples, twice as slowly per period.
const WAVE_CLOCK: f64 = 65_536.0;
/// The noise channel's LFSR clock is this over (divisor × 2^(shift + 1)),
/// with divisor code 0 counting as 0.5 (Pan Docs, "Noise channel").
const NOISE_CLOCK: f64 = 524_288.0;

/// A4, the tuning reference, and its MIDI number.
const A4_HZ: f64 = 440.0;
const A4_MIDI: i32 = 69;

/// A pitch class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NoteName {
    C,
    CSharp,
    D,
    DSharp,
    E,
    F,
    FSharp,
    G,
    GSharp,
    A,
    ASharp,
    B,
}

impl NoteName {
    const ALL: [NoteName; 12] = [
        NoteName::C,
        NoteName::CSharp,
        NoteName::D,
        NoteName::DSharp,
        NoteName::E,
        NoteName::F,
        NoteName::FSharp,
        NoteName::G,
        NoteName::GSharp,
        NoteName::A,
        NoteName::ASharp,
        NoteName::B,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            NoteName::C => "C",
            NoteName::CSharp => "C#",
            NoteName::D => "D",
            NoteName::DSharp => "D#",
            NoteName::E => "E",
            NoteName::F => "F",
            NoteName::FSharp => "F#",
            NoteName::G => "G",
            NoteName::GSharp => "G#",
            NoteName::A => "A",
            NoteName::ASharp => "A#",
            NoteName::B => "B",
        }
    }
}

/// An equal-tempered note: a pitch class in an octave (scientific pitch
/// notation, so A4 is 440 Hz and C4 is middle C).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Note {
    pub name: NoteName,
    pub octave: i32,
}

impl Note {
    /// The note's MIDI number (A4 = 69).
    pub fn midi(self) -> i32 {
        (self.octave + 1) * 12 + self.name as i32
    }

    pub fn from_midi(midi: i32) -> Note {
        Note {
            name: NoteName::ALL[midi.rem_euclid(12) as usize],
            octave: midi.div_euclid(12) - 1,
        }
    }
}

impl fmt::Display for Note {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.name.as_str(), self.octave)
    }
}

/// A tone channel's period as a frequency and the nearest note.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pitch {
    /// The 11-bit period the registers hold.
    pub period: u16,
    pub hz: f64,
    pub note: Note,
    /// How far `hz` is from `note`, in cents (-50 to +50).
    pub cents: f64,
}

impl Pitch {
    /// The pitch of a tone channel's period; `None` for the noise channel,
    /// whose registers hold no period.
    pub fn of_period(channel: ApuChannel, period: u16) -> Option<Pitch> {
        let clock = match channel {
            ApuChannel::Square1 | ApuChannel::Square2 => SQUARE_CLOCK,
            ApuChannel::Wave => WAVE_CLOCK,
            ApuChannel::Noise => return None,
        };
        let period = period & 0x7FF;
        let hz = clock / f64::from(2048 - period);
        let (note, cents) = nearest_note(hz);
        Some(Pitch {
            period,
            hz,
            note,
            cents,
        })
    }
}

impl fmt::Display for Pitch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "period {} = {} {:+.1}c ({:.2} Hz)",
            self.period, self.note, self.cents, self.hz
        )
    }
}

/// The fractional MIDI number of a frequency (A4 = 69.0).
pub fn midi_of(hz: f64) -> f64 {
    f64::from(A4_MIDI) + 12.0 * (hz / A4_HZ).log2()
}

/// The equal-tempered note nearest `hz`, and the distance to it in cents.
pub fn nearest_note(hz: f64) -> (Note, f64) {
    let midi = midi_of(hz);
    let nearest = midi.round();
    (Note::from_midi(nearest as i32), 100.0 * (midi - nearest))
}

/// The noise channel's LFSR clock for NR43's shift and divisor code.
pub fn noise_hz(shift: u8, divisor: u8) -> f64 {
    let divisor = if divisor == 0 {
        0.5
    } else {
        f64::from(divisor)
    };
    NOISE_CLOCK / (divisor * f64::from(1u32 << (shift + 1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_square_period_of_1750_is_a4_slightly_flat() {
        let p = Pitch::of_period(ApuChannel::Square1, 1750).unwrap();
        assert_eq!(p.note.to_string(), "A4");
        assert!((p.hz - 439.8389).abs() < 1e-3);
        assert!((p.cents + 0.63).abs() < 0.01);
    }

    #[test]
    fn the_wave_channel_sounds_an_octave_below_a_square_at_the_same_period() {
        let square = Pitch::of_period(ApuChannel::Square2, 1714).unwrap();
        let wave = Pitch::of_period(ApuChannel::Wave, 1714).unwrap();
        assert_eq!(square.note.midi() - wave.note.midi(), 12);
    }

    #[test]
    fn notes_round_trip_through_midi_across_octaves() {
        for midi in [0, 11, 12, 60, 69, 127] {
            assert_eq!(Note::from_midi(midi).midi(), midi);
        }
        assert_eq!(Note::from_midi(60).to_string(), "C4");
        assert_eq!(Note::from_midi(61).to_string(), "C#4");
    }

    #[test]
    fn noise_divisor_zero_counts_as_a_half() {
        assert_eq!(noise_hz(5, 2), 4096.0);
        assert_eq!(noise_hz(0, 0), 524_288.0);
    }

    #[test]
    fn the_noise_channel_has_no_period_pitch() {
        assert!(Pitch::of_period(ApuChannel::Noise, 100).is_none());
    }
}
