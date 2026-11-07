//! Per-channel audio by replay: the one part of this crate that runs a core.
//!
//! SameBoy renders only the mix, so each channel is a replay of the same span
//! from the same state with the other three muted. Muting changes only the
//! rendered output, and starting capture restarts the renderer from silence,
//! so the replays are exact: `a_replay_is_exactly_deterministic` and
//! `per_channel_wavs_sum_to_the_mixed_wav_exactly` (tests/views.rs) pin it.

use gbcli_core::{ApuChannel, AudioCapture, AudioSamples, Core, CoreError, Registers, Ticks};

use crate::AudioError;

/// The mix and each channel alone, over the same span.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Takes {
    pub mixed: AudioSamples,
    /// Indexed in [`ApuChannel::ALL`] order; see [`Takes::channel`].
    pub channels: [AudioSamples; 4],
}

impl Takes {
    pub fn channel(&self, channel: ApuChannel) -> &AudioSamples {
        &self.channels[channel as usize]
    }

    /// The largest difference, on either side, between the channels' sum and
    /// the mix; 0 when they add up exactly.
    pub fn residual(&self) -> u32 {
        let mut worst = 0;
        for (i, m) in self.mixed.samples.iter().enumerate() {
            let side = |pick: fn(&gbcli_core::StereoSample) -> i16| {
                let sum: i32 = self
                    .channels
                    .iter()
                    .map(|c| c.samples.get(i).map_or(0, |s| i32::from(pick(s))))
                    .sum();
                (sum - i32::from(pick(m))).unsigned_abs()
            };
            worst = worst.max(side(|s| s.left)).max(side(|s| s.right));
        }
        worst
    }
}

/// Renders the mix and each channel alone: five runs, each on a core from
/// `make` (which must give the same state every time), driven by `drive`
/// (which must run the same span every time). A run that ends anywhere else
/// than the first, or renders another number of samples, is refused as
/// [`AudioError::ReplayDiverged`].
pub fn record_takes<C: Core>(
    mut make: impl FnMut() -> Result<C, CoreError>,
    mut drive: impl FnMut(&mut C) -> Result<(), CoreError>,
) -> Result<Takes, AudioError> {
    let mut take =
        |capture: AudioCapture| -> Result<(AudioSamples, (Ticks, Registers)), AudioError> {
            let mut core = make()?;
            core.set_audio_capture(Some(capture))?;
            drive(&mut core)?;
            let audio = core.take_audio()?;
            Ok((audio, (core.now(), core.registers())))
        };
    let (mixed, end) = take(AudioCapture::MIXED)?;
    let mut channels: [AudioSamples; 4] = Default::default();
    for channel in ApuChannel::ALL {
        let (audio, solo_end) = take(AudioCapture::solo(channel))?;
        if solo_end != end || audio.samples.len() != mixed.samples.len() {
            return Err(AudioError::ReplayDiverged { channel });
        }
        channels[channel as usize] = audio;
    }
    Ok(Takes { mixed, channels })
}
