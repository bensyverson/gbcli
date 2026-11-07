//! The write log decoded: each write's register, what it set, and the pitch a
//! period write or trigger makes.
//!
//! A write to one register often only means something with another's bits (a
//! period is split across NRx3 and NRx4), so decoding keeps a shadow of the
//! registers, starting from an [`ApuSnapshot`] taken when the log began and
//! updated by every write the APU took.

use std::fmt;

use gbcli_core::{ApuChannel, ApuWrite, ApuWriteEffect, Core, CoreError, IoReg, Region};

use crate::pitch::{Pitch, noise_hz};
use crate::reg::{Duty, Envelope, LfsrWidth, SoundReg, Sweep, WaveLevel, channel_name};

/// NR10 to NR52.
const REGS: usize = 0x17;

/// The sound registers as the APU holds them, for decoding a log that starts
/// here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApuSnapshot {
    /// $FF10-$FF26 as written (write-only bits included).
    pub regs: [u8; REGS],
    /// NR52 bit 7.
    pub powered: bool,
}

impl ApuSnapshot {
    /// The registers from the core's IO region, which holds the bits last
    /// written, and the power bit from NR52.
    pub fn from_core(core: &dyn Core) -> Result<ApuSnapshot, CoreError> {
        let io = core.region(Region::Io)?.bytes;
        let mut regs = [0; REGS];
        regs.copy_from_slice(&io[0x10..0x10 + REGS]);
        let powered = match core.io_register(IoReg(0x26)) {
            Ok(nr52) => nr52 & 0x80 != 0,
            // Unknown while the APU lags; the power bit only changes on a
            // write, so the stored byte has it.
            Err(_) => regs[0x16] & 0x80 != 0,
        };
        Ok(ApuSnapshot { regs, powered })
    }

    fn reg(&self, addr: u16) -> u8 {
        self.regs[usize::from(addr - 0xFF10)]
    }

    /// The 11-bit period of a tone channel, from its NRx3 and NRx4.
    fn period(&self, channel: ApuChannel) -> u16 {
        let low = match channel {
            ApuChannel::Square1 => 0xFF13,
            ApuChannel::Square2 => 0xFF18,
            ApuChannel::Wave => 0xFF1D,
            ApuChannel::Noise => return 0,
        };
        u16::from(self.reg(low)) | (u16::from(self.reg(low + 1) & 7) << 8)
    }
}

/// What a write set, decoded.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Change {
    Power {
        on: bool,
    },
    /// NR50: master volume per side (0-7, applied as 1-8) and the cartridge's VIN.
    Volume {
        left: u8,
        right: u8,
        vin_left: bool,
        vin_right: bool,
    },
    /// NR51: which channels each side hears, in [`ApuChannel::ALL`] order.
    Panning {
        left: [bool; 4],
        right: [bool; 4],
    },
    /// A wave RAM byte: two samples, high nibble first.
    WaveRam {
        index: u8,
        samples: [u8; 2],
    },
    Sweep(Sweep),
    /// NR11, NR21: duty and length. `length` is the steps of 1/256 s the
    /// channel plays for once length is on.
    DutyLength {
        channel: ApuChannel,
        duty: Duty,
        length: u16,
    },
    /// NR31, NR41, and NR11/NR21 while the APU is off on a DMG.
    Length {
        channel: ApuChannel,
        length: u16,
    },
    Envelope {
        channel: ApuChannel,
        envelope: Envelope,
    },
    /// NRx2 with bits 3-7 clear turns the channel's DAC off, and the channel with it.
    DacOff {
        channel: ApuChannel,
    },
    WaveDac {
        on: bool,
    },
    WaveLevel(WaveLevel),
    /// NRx3: the period's low byte, completed by the shadow's NRx4.
    Period {
        channel: ApuChannel,
        pitch: Pitch,
    },
    /// NRx4: trigger and length enable; for tone channels, the period too.
    Control {
        channel: ApuChannel,
        trigger: bool,
        length_enabled: bool,
        pitch: Option<Pitch>,
    },
    /// NR43.
    Noise {
        shift: u8,
        width: LfsrWidth,
        divisor: u8,
        hz: f64,
    },
}

impl fmt::Display for Change {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Change::Power { on } => write!(f, "APU {}", if *on { "on" } else { "off" }),
            Change::Volume {
                left,
                right,
                vin_left,
                vin_right,
            } => {
                write!(f, "volume left {left}, right {right}")?;
                if *vin_left || *vin_right {
                    write!(f, ", VIN left {vin_left}, right {vin_right}")?;
                }
                Ok(())
            }
            Change::Panning { left, right } => {
                write!(f, "panning left {}, right {}", side(left), side(right))
            }
            Change::WaveRam { samples, .. } => {
                write!(f, "wave RAM samples {}, {}", samples[0], samples[1])
            }
            Change::Sweep(sweep) => sweep.fmt(f),
            Change::DutyLength {
                channel,
                duty,
                length,
            } => write!(
                f,
                "{} duty {}, length {length}",
                channel_name(*channel),
                duty.as_str()
            ),
            Change::Length { channel, length } => {
                write!(f, "{} length {length}", channel_name(*channel))
            }
            Change::Envelope { channel, envelope } => {
                write!(f, "{} {envelope}", channel_name(*channel))
            }
            Change::DacOff { channel } => write!(f, "{} DAC off", channel_name(*channel)),
            Change::WaveDac { on } => write!(f, "wave DAC {}", if *on { "on" } else { "off" }),
            Change::WaveLevel(level) => write!(f, "wave level {}", level.as_str()),
            Change::Period { channel, pitch } => write!(f, "{} {pitch}", channel_name(*channel)),
            Change::Control {
                channel,
                trigger,
                length_enabled,
                pitch,
            } => {
                f.write_str(channel_name(*channel))?;
                if *trigger {
                    f.write_str(" trigger,")?;
                }
                write!(f, " length {}", if *length_enabled { "on" } else { "off" })?;
                if let Some(pitch) = pitch {
                    write!(f, ", {pitch}")?;
                }
                Ok(())
            }
            Change::Noise {
                shift,
                width,
                divisor,
                hz,
            } => write!(
                f,
                "noise shift {shift}, {}, divisor {divisor} = {hz} Hz",
                width.as_str()
            ),
        }
    }
}

/// A side's channels as digits 1-4, or `-` for none.
fn side(on: &[bool; 4]) -> String {
    let digits: String = (1..=4)
        .filter(|i| on[i - 1])
        .map(|i| i.to_string())
        .collect();
    if digits.is_empty() {
        "-".into()
    } else {
        digits
    }
}

/// One logged write, decoded.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecodedWrite {
    pub write: ApuWrite,
    pub reg: SoundReg,
    pub change: Change,
    /// The write landed while the APU was off, on a DMG length register: the
    /// length was taken, the rest ignored.
    pub length_only: bool,
}

impl fmt::Display for DecodedWrite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ${:02X}: {}", self.reg, self.write.value, self.change)?;
        match self.write.effect {
            ApuWriteEffect::Applied if self.length_only => {
                f.write_str(" (the APU is off: a DMG takes only the length)")
            }
            ApuWriteEffect::Applied => Ok(()),
            ApuWriteEffect::IgnoredApuOff => f.write_str(" (ignored: the APU is off)"),
            ApuWriteEffect::WaveRamWhilePlaying => f.write_str(
                " (channel 3 is playing: it may land on the byte being read, or nowhere)",
            ),
        }
    }
}

/// Decodes `writes`, a log that began at `start`. Writes outside the sound
/// registers (the unmapped $FF15, $FF1F, $FF27-$FF2F) are skipped.
pub fn decode(start: &ApuSnapshot, writes: &[ApuWrite]) -> Vec<DecodedWrite> {
    let mut shadow = *start;
    writes
        .iter()
        .filter_map(|w| decode_one(&mut shadow, w))
        .collect()
}

fn decode_one(shadow: &mut ApuSnapshot, write: &ApuWrite) -> Option<DecodedWrite> {
    let reg = SoundReg::at(write.addr)?;
    let v = write.value;
    let applied = write.effect == ApuWriteEffect::Applied;
    // A DMG takes NRx1's length while the APU is off (the core marks the
    // write applied); the duty stays as it was.
    let length_only = applied
        && !shadow.powered
        && matches!(
            reg,
            SoundReg::Nr11 | SoundReg::Nr21 | SoundReg::Nr41 | SoundReg::Nr31
        );
    if applied && !matches!(reg, SoundReg::Wave(_)) {
        let slot = usize::from(write.addr - 0xFF10);
        shadow.regs[slot] = if length_only && reg != SoundReg::Nr31 {
            (shadow.regs[slot] & 0xC0) | (v & 0x3F)
        } else {
            v
        };
    }
    let channel = reg.channel();
    let change = match reg {
        SoundReg::Nr52 => Change::Power { on: v & 0x80 != 0 },
        SoundReg::Nr50 => Change::Volume {
            left: (v >> 4) & 7,
            right: v & 7,
            vin_left: v & 0x80 != 0,
            vin_right: v & 0x08 != 0,
        },
        SoundReg::Nr51 => Change::Panning {
            left: std::array::from_fn(|i| v & (0x10 << i) != 0),
            right: std::array::from_fn(|i| v & (1 << i) != 0),
        },
        SoundReg::Wave(index) => Change::WaveRam {
            index,
            samples: [v >> 4, v & 15],
        },
        SoundReg::Nr10 => Change::Sweep(Sweep::from_bits(v)),
        SoundReg::Nr11 | SoundReg::Nr21 if !length_only => Change::DutyLength {
            channel: channel?,
            duty: Duty::from_bits(v >> 6),
            length: 64 - u16::from(v & 0x3F),
        },
        SoundReg::Nr11 | SoundReg::Nr21 | SoundReg::Nr41 => Change::Length {
            channel: channel?,
            length: 64 - u16::from(v & 0x3F),
        },
        SoundReg::Nr31 => Change::Length {
            channel: ApuChannel::Wave,
            length: 256 - u16::from(v),
        },
        SoundReg::Nr12 | SoundReg::Nr22 | SoundReg::Nr42 if !Envelope::dac_on(v) => {
            Change::DacOff { channel: channel? }
        }
        SoundReg::Nr12 | SoundReg::Nr22 | SoundReg::Nr42 => Change::Envelope {
            channel: channel?,
            envelope: Envelope::from_bits(v),
        },
        SoundReg::Nr30 => Change::WaveDac { on: v & 0x80 != 0 },
        SoundReg::Nr32 => Change::WaveLevel(WaveLevel::from_bits(v)),
        SoundReg::Nr13 | SoundReg::Nr23 | SoundReg::Nr33 => {
            let channel = channel?;
            Change::Period {
                channel,
                pitch: Pitch::of_period(channel, shadow_period(shadow, channel, write, v))?,
            }
        }
        SoundReg::Nr14 | SoundReg::Nr24 | SoundReg::Nr34 | SoundReg::Nr44 => {
            let channel = channel?;
            Change::Control {
                channel,
                trigger: v & 0x80 != 0,
                length_enabled: v & 0x40 != 0,
                pitch: Pitch::of_period(channel, shadow_period(shadow, channel, write, v)),
            }
        }
        SoundReg::Nr43 => Change::Noise {
            shift: v >> 4,
            width: if v & 8 != 0 {
                LfsrWidth::Bits7
            } else {
                LfsrWidth::Bits15
            },
            divisor: v & 7,
            hz: noise_hz(v >> 4, v & 7),
        },
    };
    if let Change::Power { on } = change
        && applied
    {
        if !on {
            // Powering off clears NR10-NR51 (a DMG keeps the lengths, which
            // the shadow doesn't need: lengths are reported per write).
            shadow.regs[..REGS - 1].fill(0);
        }
        shadow.powered = on;
    }
    Some(DecodedWrite {
        write: *write,
        reg,
        change,
        length_only,
    })
}

/// The period a write to NRx3 or NRx4 means: the shadow's, or, for a write
/// the APU ignored, the one it would have set.
fn shadow_period(shadow: &ApuSnapshot, channel: ApuChannel, write: &ApuWrite, v: u8) -> u16 {
    let mut probe = *shadow;
    probe.regs[usize::from(write.addr - 0xFF10)] = v;
    probe.period(channel)
}
