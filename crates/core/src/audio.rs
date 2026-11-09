//! What the APU was told and what it played: the sound-register write log and
//! the sample stream (design §9, leaf aDOL0w).

use crate::apu::ApuChannel;
use crate::time::Ticks;

/// The sound registers, NR10 ($FF10) to NR52 ($FF26).
pub const SOUND_REGISTERS: std::ops::RangeInclusive<u16> = 0xFF10..=0xFF26;
/// Wave RAM, 32 four-bit samples.
pub const WAVE_RAM: std::ops::RangeInclusive<u16> = 0xFF30..=0xFF3F;

/// Whether `addr` is a sound register or wave RAM: an address the write log records.
pub fn is_sound_addr(addr: u16) -> bool {
    SOUND_REGISTERS.contains(&addr) || WAVE_RAM.contains(&addr)
}

/// What the hardware did with a write to a sound register, as far as the core
/// can tell at the moment of the write.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ApuWriteEffect {
    /// The APU took it.
    Applied,
    /// The APU was off (NR52 bit 7 clear), so the register ignored it. A DMG
    /// still takes the length half of NR11, NR21, NR31 and NR41 while off, and
    /// wave RAM is always writable: those are `Applied`.
    IgnoredApuOff,
    /// Wave RAM written while channel 3 plays: the write goes to the byte the
    /// channel is reading (CGB, and DMG only on the cycle it reads), or is
    /// dropped, so `addr` is not where it landed.
    WaveRamWhilePlaying,
}

/// One CPU write to a sound register or wave RAM.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ApuWrite {
    /// When the write happened, to within the M-cycle of the write.
    pub at: Ticks,
    /// The frame it fell in: the number of the last frame published before
    /// it, as [`crate::Core::last_complete_frame`] would have said then (0
    /// before the first).
    pub frame: u64,
    /// The line the PPU was on; `None` with the LCD off.
    pub line: Option<u8>,
    /// The instruction that wrote it.
    pub pc: u16,
    /// Whether `pc` was the boot ROM's: it was mapped over the address, so
    /// the instruction was no cart byte and no `.sym` label names it.
    pub boot_rom: bool,
    pub addr: u16,
    pub value: u8,
    pub effect: ApuWriteEffect,
}

/// One stereo output sample: the core's mixer output, signed 16-bit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct StereoSample {
    pub left: i16,
    pub right: i16,
}

/// How the core renders samples while capture is on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AudioCapture {
    /// Channels silenced in the output, indexed in [`ApuChannel::ALL`] order.
    /// Muting changes only what is rendered, never what the APU does.
    pub muted: [bool; 4],
}

impl AudioCapture {
    /// Samples per second. 2^15 divides the DMG/CGB clock exactly, so every
    /// sample lands on a whole tick ([`TICKS_PER_SAMPLE`](Self::TICKS_PER_SAMPLE)).
    pub const SAMPLE_RATE: u32 = 32_768;
    /// Ticks between two samples.
    pub const TICKS_PER_SAMPLE: u64 = Ticks::PER_SECOND / Self::SAMPLE_RATE as u64;

    /// Every channel audible.
    pub const MIXED: AudioCapture = AudioCapture { muted: [false; 4] };

    /// Only `channel` audible.
    pub fn solo(channel: ApuChannel) -> AudioCapture {
        let mut muted = [true; 4];
        muted[channel as usize] = false;
        AudioCapture { muted }
    }
}

/// The samples rendered since capture began or was last taken.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AudioSamples {
    /// When the first sample was rendered; `None` when there are none.
    pub first: Option<Ticks>,
    pub samples: Vec<StereoSample>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sample_is_a_whole_number_of_ticks() {
        assert_eq!(
            AudioCapture::TICKS_PER_SAMPLE * u64::from(AudioCapture::SAMPLE_RATE),
            Ticks::PER_SECOND
        );
    }

    #[test]
    fn the_log_covers_the_sound_registers_and_wave_ram_only() {
        assert!(is_sound_addr(0xFF10) && is_sound_addr(0xFF26));
        assert!(is_sound_addr(0xFF30) && is_sound_addr(0xFF3F));
        assert!(!is_sound_addr(0xFF0F) && !is_sound_addr(0xFF27) && !is_sound_addr(0xFF40));
    }

    #[test]
    fn solo_mutes_every_other_channel() {
        assert_eq!(
            AudioCapture::solo(ApuChannel::Wave).muted,
            [true, true, false, true]
        );
    }
}
