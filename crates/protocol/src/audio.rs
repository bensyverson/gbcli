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
    /// The frame the write fell in, counted as the stop's `frame` is: the
    /// vblanks since the time base.
    pub frame: i64,
    /// The line the PPU was on; null with the LCD off.
    pub ly: Option<u8>,
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

/// A sound channel, as the write log's filter names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioChannel {
    Square1,
    Square2,
    Wave,
    Noise,
}

impl AudioChannel {
    pub fn apu(self) -> ApuChannel {
        match self {
            AudioChannel::Square1 => ApuChannel::Square1,
            AudioChannel::Square2 => ApuChannel::Square2,
            AudioChannel::Wave => ApuChannel::Wave,
            AudioChannel::Noise => ApuChannel::Noise,
        }
    }

    /// The wire spelling.
    pub fn name(self) -> &'static str {
        WavTake::solo(self.apu()).name()
    }
}

/// How many writes the log holds, by register and by the code that made them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioSummary {
    pub writes: u64,
    /// In address order; wave RAM bytes are one register, `WAVE`.
    pub registers: Vec<RegisterWrites>,
    /// Most writes first, then by PC.
    pub writers: Vec<WriterWrites>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterWrites {
    /// `NR41`, or `WAVE` for wave RAM.
    pub reg: String,
    /// The register's address; wave RAM's first byte for `WAVE`.
    pub addr: u16,
    pub writes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriterWrites {
    /// The PC of the writing instruction.
    pub pc: u16,
    /// The `.sym` label containing `pc`, as `Name` or `Name+$3` (as a stop
    /// record spells it). The log keeps no bank, so a PC in a switchable
    /// area is named only when the `.sym` has labels there in one bank, and
    /// a PC that wrote from the boot ROM is never named.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    pub writes: u64,
    /// The registers it wrote, in address order.
    pub registers: Vec<String>,
}

/// What a run's audio flags produced.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioMade {
    /// What each write's `elapsed` counts from.
    pub since: TimeBase,
    /// The channels the log and summary were filtered to; empty for all.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub channels: Vec<AudioChannel>,
    /// Every write the run made, in order: with `audio_log`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log: Option<Vec<ApuWriteLine>>,
    /// The writes counted: with `summary`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<AudioSummary>,
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
    /// Return the writes counted by register and by writer PC.
    #[serde(default, skip_serializing_if = "crate::request::is_false")]
    pub summary: bool,
    /// Only these channels' writes in the log and the summary: a channel's
    /// own registers, and wave RAM for the wave channel. NR50-NR52 belong to
    /// none, so a filter leaves them out. Empty for every write.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub channels: Vec<AudioChannel>,
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
        self.log || self.summary || self.piano.is_some()
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
            frame: -1,
            ly: None,
            pc: 0x150,
            addr: 0xFF26,
            value: 0,
            effect: ApuWriteEffect::IgnoredApuOff,
            text: "NR52 $00: power off".into(),
        };
        let json = serde_json::to_string(&line).unwrap();
        assert!(json.contains(r#""effect":"ignored_apu_off""#), "{json}");
        assert!(json.contains(r#""frame":-1,"ly":null"#), "{json}");
        assert_eq!(serde_json::from_str::<ApuWriteLine>(&json).unwrap(), line);
    }

    #[test]
    fn a_run_filters_its_log_by_channel_and_asks_for_a_summary() {
        let line = r#"{"id":"1","op":"run","args":{"audio":{"summary":true,"channels":["noise","square2"]}}}"#;
        let request = parse_request(line).expect("parses");
        let Op::Run(RunArgs {
            audio: Some(audio), ..
        }) = &request.op
        else {
            panic!("{request:?}")
        };
        assert!(audio.summary && !audio.log);
        assert!(audio.wants_log(), "a summary counts the log");
        assert_eq!(
            audio.channels,
            vec![AudioChannel::Noise, AudioChannel::Square2]
        );
        assert_eq!(AudioChannel::Noise.apu(), ApuChannel::Noise);
        assert_eq!(serde_json::to_string(&request).unwrap(), line);
    }

    #[test]
    fn a_take_names_its_channel() {
        assert_eq!(WavTake::solo(ApuChannel::Square1).name(), "square1");
        assert_eq!(WavTake::solo(ApuChannel::Noise), WavTake::Noise);
    }
}
