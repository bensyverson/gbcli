//! APU channel state beyond the registers.

/// One of the four sound channels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ApuChannel {
    Square1,
    Square2,
    Wave,
    Noise,
}

impl ApuChannel {
    pub const ALL: [ApuChannel; 4] = [
        ApuChannel::Square1,
        ApuChannel::Square2,
        ApuChannel::Wave,
        ApuChannel::Noise,
    ];
}

/// What one channel is producing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ApuChannelState {
    /// Current volume, 0–15 (the envelope's level; for the wave channel, the
    /// output level as 15, 8, 4 or 0).
    pub volume: u8,
    /// The 4-bit sample the channel outputs now; 0 while it's off.
    pub amplitude: u8,
    /// The core's period counter reload value, in the core's own units.
    pub period: u16,
}

/// Whether the APU state is as of now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApuSync {
    /// The APU has run every cycle that has passed.
    Current,
    /// The core batches APU cycles and hasn't run the latest ones: the state is
    /// exact as of a slightly earlier moment, not now.
    Lagging,
}

/// The four channels and wave RAM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApuState {
    /// Indexed in [`ApuChannel::ALL`] order; see [`ApuState::channel`].
    pub channels: [ApuChannelState; 4],
    /// Wave RAM as 32 nibbles, high nibble of each byte first.
    pub wave_table: [u8; 32],
    pub sync: ApuSync,
}

impl ApuState {
    pub fn channel(&self, channel: ApuChannel) -> ApuChannelState {
        self.channels[channel as usize]
    }
}
