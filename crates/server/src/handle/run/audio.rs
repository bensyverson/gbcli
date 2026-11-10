//! A run's audio (design §9): the sound-register write log and the samples
//! are collected on the run itself; per-channel WAVs replay it from its
//! starting state with the other channels muted (`record_takes_with`). Every
//! file is written after the reads and the shot, and only when they
//! succeeded, so a refused or failed run writes none.

use std::path::{Path, PathBuf};

use gbcli_audio::{
    ApuSnapshot, AudioError, PianoRoll, Takes, decode, name_writers, note_spans, on_channels,
    piano_roll, record_takes_with, spectrogram, summarize, wav,
};
use gbcli_core::{
    ApuChannel, AudioCapture, AudioSamples, Core, CoreError, Feature, Support, Ticks,
};
use gbcli_engine::{Machine, Since};
use gbcli_protocol::{
    ApuWriteLine, AudioArgs, AudioMade, ErrorBody, ErrorCode, ImageMade, ImageOut, WavArgs,
    WavChannels, WavMade, WavTake,
};
use gbcli_view::Image;

use super::super::Failure;
use super::super::files::Files;
use super::super::shot::check_image;

/// The piano roll's scale when the request gives none. At 1 a short clip is
/// a few dots (four columns a frame, three rows a semitone); its native width
/// is capped at `piano::MAX_WIDTH`, so 2 keeps a long clip bounded too.
const PIANO_SCALE: u32 = 2;

/// The spectrogram's scale when the request gives none. Its width grows
/// without a cap (64 columns a second), so a larger default would multiply a
/// long capture's pixels; `--scale 4` reads a short clip.
const SPECTROGRAM_SCALE: u32 = 1;

/// What a run collected, taken from the core right after it.
pub(super) struct Collected {
    snapshot: ApuSnapshot,
    from: Ticks,
    to: Ticks,
    writes: Vec<gbcli_core::ApuWrite>,
    samples: Option<AudioSamples>,
}

/// What can be refused before anything loads: the files' paths and scales.
pub(super) fn check(args: &AudioArgs, files: &Files) -> Result<(), Failure> {
    if !args.channels.is_empty() && !args.log && !args.summary {
        return Err(bad(
            "a channel filter narrows the write log or its summary, and the run asks for neither"
                .into(),
            "add --audio-log or --audio-summary, or drop --audio-channel".into(),
        ));
    }
    if let Some(w) = &args.wav {
        check_wav(&w.path, files)?;
    }
    let images = [
        (&args.piano, PIANO_SCALE),
        (&args.spectrogram, SPECTROGRAM_SCALE),
    ];
    for (image, default) in images {
        if let Some(image) = image {
            check_image(&image.path, Some(image.scale.unwrap_or(default)), files)?;
        }
    }
    Ok(())
}

fn check_wav(path_text: &str, files: &Files) -> Result<(), Failure> {
    let path = Path::new(path_text);
    if path_text.is_empty() || path.file_name().is_none() {
        return Err(bad(
            format!("`{path_text}` isn't a file path"),
            "name the WAV to write, e.g. --wav music.wav".into(),
        ));
    }
    match path.parent().filter(|d| !d.as_os_str().is_empty()) {
        Some(dir) if !files.is_dir(dir) => Err(bad(
            format!("the directory {} doesn't exist", dir.display()),
            format!("create {} first, or write the WAV elsewhere", dir.display()),
        )),
        _ => Ok(()),
    }
}

/// What the machine refuses: capture on a model whose samples don't fall on
/// whole ticks (the SGB). The write log needs no capture, so it is offered.
pub(super) fn check_machine(machine: &Machine, args: &AudioArgs) -> Result<(), Failure> {
    if !args.wants_samples() {
        return Ok(());
    }
    let feature = Feature::AudioSamples;
    let Support::No { reason } = machine.core().supports(feature) else {
        return Ok(());
    };
    let mut failure = Failure::from_core(&CoreError::Unsupported { feature, reason });
    failure.body.message = format!("this model can't capture audio samples: {reason}");
    failure.body.remedy = "the write log needs no samples: drop --wav and --spectrogram and \
                           use --audio-log or --piano, or capture on a DMG or CGB session \
                           (--model)"
        .into();
    Err(failure)
}

/// Turns on what `args` needs, just before the run.
pub(super) fn start(machine: &mut Machine, args: &AudioArgs) -> Result<ApuSnapshot, Failure> {
    let core = machine.core_mut();
    let snapshot = ApuSnapshot::from_core(core).map_err(|e| Failure::from_core(&e))?;
    if args.wants_log() {
        core.set_apu_write_log(true)
            .map_err(|e| Failure::from_core(&e))?;
    }
    if args.wants_samples() {
        core.set_audio_capture(Some(AudioCapture::MIXED))
            .map_err(|e| Failure::from_core(&e))?;
    }
    Ok(snapshot)
}

/// Takes what the run collected and turns collection off, so the machine
/// kept warm doesn't go on collecting. Called whatever the run's outcome.
pub(super) fn finish(
    machine: &mut Machine,
    args: &AudioArgs,
    snapshot: ApuSnapshot,
    from: Ticks,
) -> Result<Collected, Failure> {
    let core = machine.core_mut();
    let to = core.now();
    let fail = |e: CoreError| Failure::from_core(&e);
    let writes = if args.wants_log() {
        let writes = core.take_apu_writes().map_err(fail)?;
        core.set_apu_write_log(false).map_err(fail)?;
        writes
    } else {
        Vec::new()
    };
    let samples = if args.wants_samples() {
        let samples = core.take_audio().map_err(fail)?;
        core.set_audio_capture(None).map_err(fail)?;
        Some(samples)
    } else {
        None
    };
    Ok(Collected {
        snapshot,
        from,
        to,
        writes,
        samples,
    })
}

/// Writes the files and builds the result. `replay` runs the same request on
/// a machine in the run's starting state, for the per-channel takes.
pub(super) fn deliver(
    args: &AudioArgs,
    collected: Collected,
    machine: &Machine,
    since: &Since,
    files: &Files,
    replay: impl FnMut() -> Result<Takes, Failure>,
) -> Result<AudioMade, Failure> {
    let (base, mark) = machine.named_base(since);
    let log = decode(&collected.snapshot, &collected.writes);
    let channels: Vec<ApuChannel> = args.channels.iter().map(|c| c.apu()).collect();
    let shown = on_channels(&log, &channels);
    let summary = args.summary.then(|| {
        let mut summary = summarize(&shown);
        if let Some(symbols) = machine.symbols() {
            name_writers(&mut summary, &shown, |pc| symbols.label_any_bank(pc));
        }
        summary
    });
    let lines = args.log.then(|| {
        shown
            .iter()
            .map(|w| ApuWriteLine {
                ticks: w.write.at.0,
                elapsed: w.write.at.0 as i64 - mark.ticks as i64,
                frame: w.write.frame as i64 - mark.frame as i64,
                ly: w.write.line,
                pc: w.write.pc,
                addr: w.write.addr,
                value: w.write.value,
                effect: w.write.effect,
                text: w.to_string(),
            })
            .collect()
    });
    let mut wavs = Vec::new();
    if let (Some(w), Some(mixed)) = (&args.wav, &collected.samples) {
        wavs = write_wavs(w, mixed, files, replay)?;
    }
    let piano = match &args.piano {
        Some(out) => {
            let spans = note_spans(&collected.snapshot, &log, collected.to);
            let roll = piano_roll(&spans, collected.from, collected.to, &PianoRoll::default());
            Some(write_image(
                out,
                roll.map_err(audio_failure)?,
                PIANO_SCALE,
                files,
            )?)
        }
        None => None,
    };
    let spectrogram = match (&args.spectrogram, &collected.samples) {
        (Some(out), Some(mixed)) => {
            let image = spectrogram(mixed, "mixed").map_err(audio_failure)?;
            Some(write_image(out, image, SPECTROGRAM_SCALE, files)?)
        }
        _ => None,
    };
    Ok(AudioMade {
        since: base,
        channels: args.channels.clone(),
        summary,
        log: lines,
        wavs,
        piano,
        spectrogram,
    })
}

fn write_wavs(
    args: &WavArgs,
    mixed: &AudioSamples,
    files: &Files,
    mut replay: impl FnMut() -> Result<Takes, Failure>,
) -> Result<Vec<WavMade>, Failure> {
    let mut takes = vec![(WavTake::Mixed, PathBuf::from(&args.path), mixed.clone())];
    if args.channels == WavChannels::Each {
        let replayed = replay()?;
        // The replays start from a copy of the run's starting state; if their
        // mix isn't the run's, the copies weren't the run.
        if replayed.mixed != *mixed {
            return Err(Failure::new(ErrorBody::plain(
                ErrorCode::Internal,
                "the replays for the per-channel WAVs didn't render the run's own mix",
            )));
        }
        for channel in ApuChannel::ALL {
            let take = WavTake::solo(channel);
            let path = channel_path(Path::new(&args.path), take);
            takes.push((take, path, replayed.channel(channel).clone()));
        }
    }
    let mut made = Vec::new();
    for (take, path, audio) in takes {
        files.write(&path, &wav(&audio.samples))?;
        made.push(WavMade {
            path: path.to_string_lossy().into_owned(),
            take,
            samples: audio.samples.len() as u64,
            rate: AudioCapture::SAMPLE_RATE,
            first_ticks: audio.first.map(|t| t.0),
        });
    }
    Ok(made)
}

/// `music.wav` → `music-square1.wav`, beside it.
fn channel_path(mixed: &Path, take: WavTake) -> PathBuf {
    let stem = mixed.file_stem().unwrap_or_default().to_string_lossy();
    let name = match mixed.extension() {
        Some(ext) => format!("{stem}-{}.{}", take.name(), ext.to_string_lossy()),
        None => format!("{stem}-{}", take.name()),
    };
    mixed.with_file_name(name)
}

fn write_image(
    out: &ImageOut,
    image: Image,
    default: u32,
    files: &Files,
) -> Result<ImageMade, Failure> {
    let scale = out.scale.unwrap_or(default);
    let image = image
        .scaled(scale)
        .map_err(|e| audio_failure(AudioError::Image(e)))?;
    let bytes = image
        .png()
        .map_err(|e| audio_failure(AudioError::Image(e)))?;
    files.write(&out.path, &bytes)?;
    Ok(ImageMade {
        path: out.path.clone(),
        width: image.width,
        height: image.height,
        scale,
    })
}

pub(super) fn audio_failure(e: AudioError) -> Failure {
    match e {
        AudioError::Core(core) => Failure::from_core(&core),
        AudioError::NoSamples | AudioError::EmptySpan => bad(
            format!("the run was too short to draw: {e}"),
            "run at least one frame: --frames 1".into(),
        ),
        AudioError::ReplayDiverged { .. } | AudioError::Image(_) => {
            Failure::new(ErrorBody::plain(ErrorCode::Internal, e.to_string()))
        }
    }
}

impl From<AudioError> for Failure {
    fn from(e: AudioError) -> Self {
        audio_failure(e)
    }
}

fn bad(message: String, remedy: String) -> Failure {
    Failure::new(ErrorBody::new(ErrorCode::BadRequest, message, remedy))
}

/// The per-channel takes: five runs of the same request, each from a copy of
/// `start` (the run's starting state, after its writes).
pub(super) fn takes(
    start: Option<&mut Machine>,
    mut drive: impl FnMut(&mut Machine) -> Result<(), Failure>,
) -> Result<Takes, Failure> {
    let start = start.ok_or_else(|| {
        Failure::new(ErrorBody::plain(
            ErrorCode::Internal,
            "per-channel WAVs were asked for without the run's starting state",
        ))
    })?;
    record_takes_with(
        || start.try_clone().map_err(|e| Failure::from_core(&e)),
        |m: &mut Machine| drive(m),
        |m: &mut Machine| m.core_mut() as &mut dyn Core,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_channel_file_is_named_after_the_mix_with_the_channel() {
        let path = |p: &str, t| channel_path(Path::new(p), t);
        assert_eq!(
            path("out/music.wav", WavTake::Square1),
            PathBuf::from("out/music-square1.wav")
        );
        assert_eq!(path("music", WavTake::Noise), PathBuf::from("music-noise"));
    }
}
