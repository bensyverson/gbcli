//! Audio on the wire (design §9, leaf vd3ulF): what a run asks to collect —
//! the sound-register write log, WAVs, a piano roll and a spectrogram — and
//! what it wrote. A write's time is counted from the request's time base, as
//! the stop record's `t` and `frame_shown` are.

use gbcli_core::{ApuChannel, ApuWriteEffect};
use serde::{Deserialize, Serialize};

use crate::stop::TimeBase;
use crate::video::{ImageMade, ImageOut};

/// Which WAVs a run writes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WavChannels {
    /// One file: the mix.
    #[default]
    Mixed,
    /// The mix, and each channel alone beside it (`<stem>-square1.wav` …),
    /// rendered by replaying the run with the other channels muted.
    Each,
}

impl WavChannels {
    pub fn is_mixed(&self) -> bool {
        *self == WavChannels::Mixed
    }
}

/// A WAV to write after the run: 16-bit stereo PCM at 32,768 Hz.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WavArgs {
    /// Where to write the mix, relative to the server's working directory.
    pub path: String,
    #[serde(default, skip_serializing_if = "WavChannels::is_mixed")]
    pub channels: WavChannels,
}

/// What a WAV holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WavTake {
    Mixed,
    Square1,
    Square2,
    Wave,
    Noise,
}

impl WavTake {
    /// The take of one channel alone.
    pub fn solo(channel: ApuChannel) -> WavTake {
        match channel {
            ApuChannel::Square1 => WavTake::Square1,
            ApuChannel::Square2 => WavTake::Square2,
            ApuChannel::Wave => WavTake::Wave,
            ApuChannel::Noise => WavTake::Noise,
        }
    }

    /// The wire spelling, also the suffix of a per-channel file's stem.
    pub fn name(self) -> &'static str {
        match self {
            WavTake::Mixed => "mixed",
            WavTake::Square1 => "square1",
            WavTake::Square2 => "square2",
            WavTake::Wave => "wave",
            WavTake::Noise => "noise",
        }
    }
}

/// A WAV that was written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WavMade {
    pub path: String,
    pub take: WavTake,
    /// Stereo sample frames, at `rate` per second.
    pub samples: u64,
    pub rate: u32,
    /// When the first sample was rendered, since power-on in 2^23 Hz ticks;
    /// absent when the run rendered none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_ticks: Option<u64>,
}

/// One logged write to a sound register or wave RAM, decoded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApuWriteLine {
    /// Since power-on, in 2^23 Hz ticks.
    pub ticks: u64,
    /// Ticks (2^23 Hz) since the time base: negative before it. Not `at`,
    /// which other rows use for wall-clock time.
    pub elapsed: i64,
    pub pc: u16,
    pub addr: u16,
    pub value: u8,
    #[serde(with = "ApuWriteEffectDef")]
    pub effect: ApuWriteEffect,
    /// The register and what the write did, in words:
    /// `NR14 $86: square 1 trigger, length off, period 1750 = A4 -0.6c (439.84 Hz)`.
    pub text: String,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "ApuWriteEffect", rename_all = "snake_case")]
pub(crate) enum ApuWriteEffectDef {
    Applied,
    IgnoredApuOff,
    WaveRamWhilePlaying,
}

/// What a run's audio flags produced.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioMade {
    /// What each write's `elapsed` counts from.
    pub since: TimeBase,
    /// Every write the run made, in order: with `audio_log`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log: Option<Vec<ApuWriteLine>>,
    /// The mix first, then any channels alone.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wavs: Vec<WavMade>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub piano: Option<ImageMade>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectrogram: Option<ImageMade>,
}

/// The audio a run asks for. Every part is optional; none means no audio.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioArgs {
    /// Return the decoded write log: the cheap text sibling of the WAV.
    #[serde(default, skip_serializing_if = "crate::request::is_false")]
    pub log: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wav: Option<WavArgs>,
    /// A piano roll of the notes the run's writes played; scale default 2 (its width is capped, so a long
    /// clip stays bounded).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub piano: Option<ImageOut>,
    /// A spectrogram of the mix; scale default 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectrogram: Option<ImageOut>,
}

impl AudioArgs {
    /// Whether anything needs the write log.
    pub fn wants_log(&self) -> bool {
        self.log || self.piano.is_some()
    }

    /// Whether anything needs rendered samples.
    pub fn wants_samples(&self) -> bool {
        self.wav.is_some() || self.spectrogram.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_request;
    use crate::request::{Op, RunArgs};

    #[test]
    fn a_run_carries_its_audio_and_refuses_strangers_inside_it() {
        let line = r#"{"id":"1","op":"run","args":{"frames":3,"audio":{"log":true,"wav":{"path":"a.wav","channels":"each"},"piano":{"path":"p.png"},"spectrogram":{"path":"s.png","scale":2}}}}"#;
        let request = parse_request(line).expect("parses");
        let Op::Run(RunArgs {
            audio: Some(audio), ..
        }) = &request.op
        else {
            panic!("{request:?}")
        };
        assert!(audio.log && audio.wants_log() && audio.wants_samples());
        let wav = audio.wav.as_ref().unwrap();
        assert_eq!(
            (wav.path.as_str(), wav.channels),
            ("a.wav", WavChannels::Each)
        );
        assert_eq!(serde_json::to_string(&request).unwrap(), line);

        let typo =
            r#"{"id":"1","op":"run","args":{"audio":{"wav":{"path":"a.wav","chanels":"each"}}}}"#;
        let err = parse_request(typo).unwrap_err();
        assert!(err.remedy.contains("`channels`"), "{}", err.remedy);
    }

    #[test]
    fn mixed_is_the_default_and_left_off_the_wire() {
        let line = r#"{"id":"1","op":"run","args":{"audio":{"wav":{"path":"a.wav"}}}}"#;
        let request = parse_request(line).expect("parses");
        let Op::Run(RunArgs {
            audio: Some(audio), ..
        }) = &request.op
        else {
            panic!("{request:?}")
        };
        assert_eq!(audio.wav.as_ref().unwrap().channels, WavChannels::Mixed);
        assert!(!audio.wants_log());
        assert_eq!(serde_json::to_string(&request).unwrap(), line);
    }

    #[test]
    fn a_write_line_spells_its_effect_in_snake_case() {
        let line = ApuWriteLine {
            ticks: 100,
            elapsed: -4,
            pc: 0x150,
            addr: 0xFF26,
            value: 0,
            effect: ApuWriteEffect::IgnoredApuOff,
            text: "NR52 $00: power off".into(),
        };
        let json = serde_json::to_string(&line).unwrap();
        assert!(json.contains(r#""effect":"ignored_apu_off""#), "{json}");
        assert_eq!(serde_json::from_str::<ApuWriteLine>(&json).unwrap(), line);
    }

    #[test]
    fn a_take_names_its_channel() {
        assert_eq!(WavTake::solo(ApuChannel::Square1).name(), "square1");
        assert_eq!(WavTake::solo(ApuChannel::Noise), WavTake::Noise);
    }
}
