//! Rows for a run's audio: the decoded write log, its summary and loudness,
//! the WAVs and the images.

use gbcli_protocol::{AudioMade, ImageMade};

use super::row::{Row, addr, capped_see};
use super::stop::since;

/// The channels a log was filtered to, `noise,square2`; none when unfiltered.
fn channels(audio: &AudioMade) -> Option<String> {
    let names: Vec<&str> = audio.channels.iter().map(|c| c.name()).collect();
    (!names.is_empty()).then(|| names.join(","))
}

/// How the audio log's `more` row says to see the rows it hides.
const APU_SEE: &str = "--json for every write, --piano for the notes";

pub fn rows(audio: &AudioMade) -> String {
    let mut out = String::new();
    if let Some(log) = &audio.log {
        out += &Row::new("audio")
            .field("since", since(&audio.since))
            .field("writes", log.len())
            .opt("channels", channels(audio))
            .line();
        // Capped: a song writes every frame, so a long run's log is
        // thousands of rows; `--audio-summary` sums them and `--json` has
        // every one, and `--piano` draws the notes.
        out += &capped_see("apu", log, APU_SEE, |line| {
            Row::new("apu")
                .field("elapsed", line.elapsed)
                .field("frame", line.frame)
                .field(
                    "ly",
                    line.ly.map_or_else(|| "off".into(), |ly| ly.to_string()),
                )
                .field("pc", addr(line.pc))
                .field("text", &line.text)
                .line()
        });
    }
    if let Some(summary) = &audio.summary {
        out += &Row::new("audio_summary")
            .field("since", since(&audio.since))
            .field("writes", summary.writes)
            .opt("channels", channels(audio))
            .line();
        // The mix first, then the channels loudest first: as the server ranks them.
        for l in &summary.loudness {
            out += &Row::new("audio_loudness")
                .field("take", l.take.name())
                .field(
                    "rms_dbfs",
                    l.rms_dbfs
                        .map_or_else(|| "silent".into(), |r| format!("{:.1}", r.0)),
                )
                .field("sounding_s", format!("{:.3}", l.sounding_s.0))
                .line();
        }
        for r in &summary.registers {
            out += &Row::new("audio_register")
                .field("reg", &r.reg)
                .field("addr", addr(r.addr))
                .field("writes", r.writes)
                .line();
        }
        for w in &summary.writers {
            out += &Row::new("audio_writer")
                .field("pc", addr(w.pc))
                .opt("symbol", w.symbol.as_deref())
                .field("writes", w.writes)
                .field("registers", w.registers.join(","))
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
