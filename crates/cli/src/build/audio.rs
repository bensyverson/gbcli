//! The audio flags of `run` → the protocol's `AudioArgs`.

use gbcli_protocol::{AudioArgs, AudioChannel, ImageOut, WavArgs, WavChannels};

use crate::argv::Invocation;
use crate::usage::Usage;

/// `--audio-log`, `--audio-summary`, `--audio-channel N`, `--wav PATH
/// [--channels mixed|each]`, `--piano PATH` and `--spectrogram PATH`, or
/// `None` when the run asks for no audio.
pub fn run_audio(inv: &Invocation) -> Result<Option<AudioArgs>, Usage> {
    let channels = inv.one("channels").map(channels).transpose()?;
    let wav = match (inv.one("wav"), channels) {
        (Some(path), channels) => Some(WavArgs {
            path: path.to_string(),
            channels: channels.unwrap_or_default(),
        }),
        (None, Some(_)) => {
            return Err(Usage::new(
                "--channels only applies with --wav",
                "add --wav PATH, or drop it",
            ));
        }
        (None, None) => None,
    };
    let scale = super::video::scale(inv)?;
    let image = |flag| {
        inv.one(flag).map(|path| ImageOut {
            path: path.to_string(),
            scale,
        })
    };
    let audio = AudioArgs {
        log: inv.has("audio-log"),
        summary: inv.has("audio-summary"),
        channels: inv
            .all("audio-channel")
            .map(channel)
            .collect::<Result<_, _>>()?,
        wav,
        piano: image("piano"),
        spectrogram: image("spectrogram"),
    };
    Ok((audio != AudioArgs::default()).then_some(audio))
}

/// `--audio-channel`: a channel by number (1-4) or by name.
fn channel(text: &str) -> Result<AudioChannel, Usage> {
    match text {
        "1" | "square1" => Ok(AudioChannel::Square1),
        "2" | "square2" => Ok(AudioChannel::Square2),
        "3" | "wave" => Ok(AudioChannel::Wave),
        "4" | "noise" => Ok(AudioChannel::Noise),
        other => Err(Usage::new(
            format!("--audio-channel takes 1-4 or a channel's name, not `{other}`"),
            "1 square1, 2 square2, 3 wave, 4 noise: `--audio-channel 4` or `--audio-channel noise`",
        )),
    }
}

fn channels(text: &str) -> Result<WavChannels, Usage> {
    match text {
        "mixed" => Ok(WavChannels::Mixed),
        "each" => Ok(WavChannels::Each),
        other => Err(Usage::new(
            format!("--channels takes mixed or each, not `{other}`"),
            "each also writes every channel alone beside the mix: `--channels each`",
        )),
    }
}
