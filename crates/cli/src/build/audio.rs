//! The audio flags of `run` → the protocol's `AudioArgs`.

use gbcli_protocol::{AudioArgs, ImageOut, WavArgs, WavChannels};

use crate::argv::Invocation;
use crate::usage::Usage;

/// `--audio-log`, `--wav PATH [--channels mixed|each]`, `--piano PATH` and
/// `--spectrogram PATH`, or `None` when the run asks for no audio.
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
    let image = |flag| {
        inv.one(flag).map(|path| ImageOut {
            path: path.to_string(),
            scale: None,
        })
    };
    let audio = AudioArgs {
        log: inv.has("audio-log"),
        wav,
        piano: image("piano"),
        spectrogram: image("spectrogram"),
    };
    Ok((audio != AudioArgs::default()).then_some(audio))
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
