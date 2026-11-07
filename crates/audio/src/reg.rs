//! The sound registers by name, and the fields their bits hold.

use std::fmt;

use gbcli_core::ApuChannel;

/// A sound register or a wave RAM byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SoundReg {
    Nr10,
    Nr11,
    Nr12,
    Nr13,
    Nr14,
    Nr21,
    Nr22,
    Nr23,
    Nr24,
    Nr30,
    Nr31,
    Nr32,
    Nr33,
    Nr34,
    Nr41,
    Nr42,
    Nr43,
    Nr44,
    Nr50,
    Nr51,
    Nr52,
    /// Wave RAM byte 0-15 ($FF30-$FF3F): two 4-bit samples, high nibble first.
    Wave(u8),
}

impl SoundReg {
    /// The register at `addr`; `None` outside $FF10-$FF26 and wave RAM, and
    /// for the unmapped $FF15 and $FF1F.
    pub fn at(addr: u16) -> Option<SoundReg> {
        use SoundReg::*;
        Some(match addr {
            0xFF10 => Nr10,
            0xFF11 => Nr11,
            0xFF12 => Nr12,
            0xFF13 => Nr13,
            0xFF14 => Nr14,
            0xFF16 => Nr21,
            0xFF17 => Nr22,
            0xFF18 => Nr23,
            0xFF19 => Nr24,
            0xFF1A => Nr30,
            0xFF1B => Nr31,
            0xFF1C => Nr32,
            0xFF1D => Nr33,
            0xFF1E => Nr34,
            0xFF20 => Nr41,
            0xFF21 => Nr42,
            0xFF22 => Nr43,
            0xFF23 => Nr44,
            0xFF24 => Nr50,
            0xFF25 => Nr51,
            0xFF26 => Nr52,
            0xFF30..=0xFF3F => Wave((addr - 0xFF30) as u8),
            _ => return None,
        })
    }

    /// The channel the register belongs to; `None` for NR50-NR52 and wave RAM.
    pub fn channel(self) -> Option<ApuChannel> {
        use SoundReg::*;
        match self {
            Nr10 | Nr11 | Nr12 | Nr13 | Nr14 => Some(ApuChannel::Square1),
            Nr21 | Nr22 | Nr23 | Nr24 => Some(ApuChannel::Square2),
            Nr30 | Nr31 | Nr32 | Nr33 | Nr34 => Some(ApuChannel::Wave),
            Nr41 | Nr42 | Nr43 | Nr44 => Some(ApuChannel::Noise),
            Nr50 | Nr51 | Nr52 | Wave(_) => None,
        }
    }
}

impl fmt::Display for SoundReg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SoundReg::Wave(i) => write!(f, "WAVE{i:X}"),
            other => {
                let name = format!("{other:?}").to_uppercase();
                f.write_str(&name)
            }
        }
    }
}

/// A channel's display name.
pub fn channel_name(channel: ApuChannel) -> &'static str {
    match channel {
        ApuChannel::Square1 => "square 1",
        ApuChannel::Square2 => "square 2",
        ApuChannel::Wave => "wave",
        ApuChannel::Noise => "noise",
    }
}

/// Which way an envelope or the sweep moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    Up,
    Down,
}

impl Direction {
    pub fn as_str(self) -> &'static str {
        match self {
            Direction::Up => "up",
            Direction::Down => "down",
        }
    }
}

/// A square channel's duty cycle (NRx1 bits 6-7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Duty {
    Eighth,
    Quarter,
    Half,
    ThreeQuarters,
}

impl Duty {
    pub fn from_bits(bits: u8) -> Duty {
        match bits & 3 {
            0 => Duty::Eighth,
            1 => Duty::Quarter,
            2 => Duty::Half,
            _ => Duty::ThreeQuarters,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Duty::Eighth => "12.5%",
            Duty::Quarter => "25%",
            Duty::Half => "50%",
            Duty::ThreeQuarters => "75%",
        }
    }
}

/// A volume envelope (NRx2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Envelope {
    /// The volume at trigger, 0-15.
    pub initial: u8,
    pub direction: Direction,
    /// Steps of 1/64 s between volume changes; 0 holds the volume.
    pub pace: u8,
}

impl Envelope {
    pub fn from_bits(value: u8) -> Envelope {
        Envelope {
            initial: value >> 4,
            direction: if value & 8 != 0 {
                Direction::Up
            } else {
                Direction::Down
            },
            pace: value & 7,
        }
    }

    /// The DAC is on when any of bits 3-7 is set.
    pub fn dac_on(value: u8) -> bool {
        value & 0xF8 != 0
    }
}

impl fmt::Display for Envelope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.pace == 0 {
            write!(f, "envelope {} steady", self.initial)
        } else {
            write!(
                f,
                "envelope {} {}, pace {}",
                self.initial,
                self.direction.as_str(),
                self.pace
            )
        }
    }
}

/// Square 1's frequency sweep (NR10).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Sweep {
    /// Steps of 1/128 s between sweeps; 0 turns the sweep off.
    pub pace: u8,
    pub direction: Direction,
    pub shift: u8,
}

impl Sweep {
    pub fn from_bits(value: u8) -> Sweep {
        Sweep {
            pace: (value >> 4) & 7,
            direction: if value & 8 != 0 {
                Direction::Down
            } else {
                Direction::Up
            },
            shift: value & 7,
        }
    }
}

impl fmt::Display for Sweep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.pace == 0 {
            f.write_str("sweep off")
        } else {
            write!(
                f,
                "sweep {}, pace {}, shift {}",
                self.direction.as_str(),
                self.pace,
                self.shift
            )
        }
    }
}

/// The wave channel's output level (NR32 bits 5-6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WaveLevel {
    Mute,
    Full,
    Half,
    Quarter,
}

impl WaveLevel {
    pub fn from_bits(value: u8) -> WaveLevel {
        match (value >> 5) & 3 {
            0 => WaveLevel::Mute,
            1 => WaveLevel::Full,
            2 => WaveLevel::Half,
            _ => WaveLevel::Quarter,
        }
    }

    /// The level as a 0-15 volume, as `ApuChannelState` reports it.
    pub fn volume(self) -> u8 {
        match self {
            WaveLevel::Mute => 0,
            WaveLevel::Full => 15,
            WaveLevel::Half => 8,
            WaveLevel::Quarter => 4,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            WaveLevel::Mute => "0%",
            WaveLevel::Full => "100%",
            WaveLevel::Half => "50%",
            WaveLevel::Quarter => "25%",
        }
    }
}

/// The noise LFSR's width (NR43 bit 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LfsrWidth {
    Bits15,
    Bits7,
}

impl LfsrWidth {
    pub fn as_str(self) -> &'static str {
        match self {
            LfsrWidth::Bits15 => "15-bit",
            LfsrWidth::Bits7 => "7-bit",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_name_themselves_as_pan_docs_does() {
        assert_eq!(SoundReg::at(0xFF14).unwrap().to_string(), "NR14");
        assert_eq!(SoundReg::at(0xFF3A).unwrap().to_string(), "WAVEA");
        assert_eq!(SoundReg::at(0xFF15), None);
        assert_eq!(SoundReg::at(0xFF27), None);
    }

    #[test]
    fn envelope_and_sweep_bit_3_point_opposite_ways() {
        assert_eq!(Envelope::from_bits(0x08).direction, Direction::Up);
        assert_eq!(Sweep::from_bits(0x08).direction, Direction::Down);
    }
}
