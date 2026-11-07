//! Rows for a run's audio: the decoded write log, the WAVs and the images.

use gbcli_protocol::{AudioMade, ImageMade};

use super::row::{Row, addr};
use super::stop::since;

pub fn rows(audio: &AudioMade) -> String {
    let mut out = String::new();
    if let Some(log) = &audio.log {
        out += &Row::new("audio")
            .field("since", since(&audio.since))
            .field("writes", log.len())
            .line();
        for line in log {
            out += &Row::new("apu")
                .field("elapsed", line.elapsed)
                .field("pc", addr(line.pc))
                .field("text", &line.text)
                .line();
        }
    }
    for wav in &audio.wavs {
        out += &Row::new("wav")
            .field("take", wav.take.name())
            .field("path", &wav.path)
            .field("samples", wav.samples)
            .field("rate", wav.rate)
            .line();
    }
    let image = |kind: &str, made: &ImageMade| {
        Row::new(kind)
            .field("path", &made.path)
            .field("width", made.width)
            .field("height", made.height)
            .field("scale", made.scale)
            .line()
    };
    if let Some(piano) = &audio.piano {
        out += &image("piano", piano);
    }
    if let Some(spectrogram) = &audio.spectrogram {
        out += &image("spectrogram", spectrogram);
    }
    out
}
