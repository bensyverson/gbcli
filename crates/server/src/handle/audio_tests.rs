//! Audio through the handler: the decoded write log, WAVs (mixed and per
//! channel), the piano roll and the spectrogram on a run (design §9).

use std::fs;
use std::path::Path;

use gbcli_core::{ApuWriteEffect, Model};
use gbcli_protocol::{
    AudioArgs, AudioMade, ErrorCode, ImageOut, Op, Response, Rev, RunArgs, TimeBase, WavArgs,
    WavChannels, WavTake,
};

use super::Handler;
use super::test_support::{handler, request};
use crate::fixture::{Fixture, TUNE};

/// Past the tune's last write.
const TUNE_FRAMES: u64 = 40;

fn run(h: &mut Handler, args: RunArgs) -> Response {
    h.handle(&request(Op::Run(args)))
}

fn tune_run(audio: AudioArgs) -> RunArgs {
    RunArgs {
        frames: Some(TUNE_FRAMES),
        audio: Some(Box::new(audio)),
        ..RunArgs::default()
    }
}

fn path_in(f: &Fixture, name: &str) -> String {
    f.dir.join(name).to_string_lossy().into_owned()
}

fn wav_at(path: String, channels: WavChannels) -> AudioArgs {
    AudioArgs {
        wav: Some(WavArgs { path, channels }),
        ..AudioArgs::default()
    }
}

fn audio_of(response: Response) -> AudioMade {
    response.outcome.expect("ran").audio.expect("audio")
}

/// The PCM values of a 16-bit WAV, left and right interleaved.
fn pcm(path: &Path) -> Vec<i16> {
    let bytes = fs::read(path).expect("the WAV was written");
    assert_eq!(&bytes[..4], b"RIFF");
    let at = bytes.windows(4).position(|w| w == b"data").expect("data") + 8;
    bytes[at..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| i16::from_le_bytes(*c))
        .collect()
}

#[test]
fn a_channel_filter_narrows_the_log_and_the_summary_counts_it() {
    use gbcli_protocol::AudioChannel;
    let f = Fixture::with_rom("audio-filter", TUNE);
    let audio = audio_of(run(
        &mut handler(&f),
        tune_run(AudioArgs {
            log: true,
            summary: true,
            channels: vec![AudioChannel::Noise],
            ..AudioArgs::default()
        }),
    ));
    assert_eq!(audio.channels, vec![AudioChannel::Noise]);
    let log = audio.log.expect("a log");
    assert!(!log.is_empty(), "the tune plays noise at frame 15");
    assert!(
        log.iter().all(|l| (0xFF20..=0xFF23).contains(&l.addr)),
        "{log:#?}"
    );
    let summary = audio.summary.expect("a summary");
    assert_eq!(summary.writes, log.len() as u64);
    assert!(summary.registers.iter().all(|r| r.reg.starts_with("NR4")));
    let by_writers: u64 = summary.writers.iter().map(|w| w.writes).sum();
    assert_eq!(by_writers, summary.writes);

    // A summary alone counts every write and lists none.
    let f = Fixture::with_rom("audio-summary", TUNE);
    let audio = audio_of(run(
        &mut handler(&f),
        tune_run(AudioArgs {
            summary: true,
            ..AudioArgs::default()
        }),
    ));
    assert_eq!(audio.log, None);
    assert_eq!(audio.summary.expect("a summary").writes, 43);
}

#[test]
fn a_channel_filter_with_nothing_to_filter_is_refused() {
    use gbcli_protocol::AudioChannel;
    let f = Fixture::with_rom("audio-filter-alone", TUNE);
    let response = run(
        &mut handler(&f),
        tune_run(AudioArgs {
            channels: vec![AudioChannel::Wave],
            piano: Some(ImageOut {
                path: path_in(&f, "p.png"),
                scale: None,
            }),
            ..AudioArgs::default()
        }),
    );
    let err = response.outcome.expect_err("refused");
    assert_eq!(err.code, ErrorCode::BadRequest);
    assert!(err.remedy.contains("--audio-log"), "{}", err.remedy);
    assert_eq!(response.rev, Some(Rev(0)), "refused before the run");
}

#[test]
fn the_audio_log_is_the_decoded_writes_timed_from_the_session_base() {
    let f = Fixture::with_rom("audio-log", TUNE);
    let response = run(
        &mut handler(&f),
        tune_run(AudioArgs {
            log: true,
            ..AudioArgs::default()
        }),
    );
    let result = response.outcome.expect("ran");
    let stop = result.stop.expect("stop");
    let audio = result.audio.expect("audio");
    assert_eq!(audio.since, TimeBase::Boot);
    let log = audio.log.expect("a log");
    assert_eq!(log.len(), 43, "the tune's writes");
    assert_eq!(log[0].text, "NR52 $80: APU on");
    assert_eq!(
        log[42].text,
        "NR11 $BF: square 1 length 1 (the APU is off: a DMG takes only the length)"
    );
    assert_eq!(log[41].effect, ApuWriteEffect::IgnoredApuOff);
    assert!(log.windows(2).all(|w| w[0].ticks <= w[1].ticks), "in order");
    // Rebased as the stop record is: the boot mark is the stop's ticks less
    // its t (two ticks per T-cycle at single speed).
    let mark = stop.time.ticks as i64 - 2 * stop.time.t;
    for line in &log {
        assert_eq!(line.elapsed, line.ticks as i64 - mark, "{line:?}");
        assert!(line.elapsed > 0 && line.ticks <= stop.time.ticks);
    }
    assert!(audio.wavs.is_empty() && audio.piano.is_none());
}

#[test]
fn a_log_counted_from_power_on_is_its_ticks() {
    let f = Fixture::with_rom("audio-log-power-on", TUNE);
    let mut args = tune_run(AudioArgs {
        log: true,
        ..AudioArgs::default()
    });
    args.since = Some(TimeBase::PowerOn);
    let audio = audio_of(run(&mut handler(&f), args));
    assert_eq!(audio.since, TimeBase::PowerOn);
    let log = audio.log.unwrap();
    assert!(log.iter().all(|l| l.elapsed == l.ticks as i64));
}

#[test]
fn a_mixed_wav_holds_one_sample_every_256_ticks_of_the_run() {
    let f = Fixture::with_rom("audio-wav", TUNE);
    let path = path_in(&f, "tune.wav");
    let response = run(
        &mut handler(&f),
        tune_run(wav_at(path.clone(), WavChannels::Mixed)),
    );
    let result = response.outcome.expect("ran");
    let stop = result.stop.expect("stop");
    let audio = result.audio.expect("audio");
    assert!(audio.log.is_none(), "no log unless asked");
    assert_eq!(audio.wavs.len(), 1);
    let made = &audio.wavs[0];
    assert_eq!(
        (made.path.as_str(), made.take),
        (path.as_str(), WavTake::Mixed)
    );
    assert_eq!(made.rate, 32_768);
    // The run starts mid-frame and ends at its 40th vblank: 140,448 ticks a
    // frame, 256 a sample, and the last sample within a few of the stop.
    let ticks = made.samples * 256;
    assert!(
        ((TUNE_FRAMES - 1) * 140_448..=TUNE_FRAMES * 140_448).contains(&ticks),
        "{made:?}"
    );
    let last = made.first_ticks.expect("a first sample") + 256 * (made.samples - 1);
    assert!(
        last <= stop.time.ticks && stop.time.ticks - last < 256 * 8,
        "{made:?} {stop:?}"
    );
    let values = pcm(Path::new(&path));
    assert_eq!(values.len() as u64, 2 * made.samples);
    assert!(values.iter().any(|&v| v != values[0]), "not silence");
}

#[test]
fn per_channel_wavs_are_named_by_channel_and_sum_to_the_mix() {
    let f = Fixture::with_rom("audio-each", TUNE);
    let path = path_in(&f, "tune.wav");
    let audio = audio_of(run(
        &mut handler(&f),
        tune_run(wav_at(path.clone(), WavChannels::Each)),
    ));
    let takes: Vec<WavTake> = audio.wavs.iter().map(|w| w.take).collect();
    assert_eq!(
        takes,
        [
            WavTake::Mixed,
            WavTake::Square1,
            WavTake::Square2,
            WavTake::Wave,
            WavTake::Noise
        ]
    );
    let mixed = pcm(Path::new(&path));
    let mut sum = vec![0i32; mixed.len()];
    for made in &audio.wavs[1..] {
        let expected = path_in(&f, &format!("tune-{}.wav", made.take.name()));
        assert_eq!(made.path, expected);
        assert_eq!(made.samples, audio.wavs[0].samples);
        let channel = pcm(Path::new(&made.path));
        assert!(
            channel.iter().any(|&v| v != channel[0]),
            "{:?} plays",
            made.take
        );
        for (s, v) in sum.iter_mut().zip(channel) {
            *s += i32::from(v);
        }
    }
    let mixed: Vec<i32> = mixed.into_iter().map(i32::from).collect();
    assert_eq!(sum, mixed, "the channels add up to the mix exactly");
}

#[test]
fn a_run_with_and_without_capture_saves_byte_identical_heads() {
    let plain = Fixture::with_rom("audio-head-plain", TUNE);
    let captured = Fixture::with_rom("audio-head-captured", TUNE);
    let path = path_in(&captured, "tune.wav");
    // A budget at which the batched APU cycles differ with capture on.
    for (f, audio) in [
        (&plain, None),
        (&captured, Some(wav_at(path, WavChannels::Mixed))),
    ] {
        let mut args = tune_run(AudioArgs::default());
        args.frames = Some(35);
        args.audio = audio.map(Box::new);
        run(&mut handler(f), args).outcome.expect("ran");
    }
    let head = |f: &Fixture| {
        let head = f.bundle.session("demo").unwrap().read_head(Rev(1));
        head.unwrap().expect("a head").blob
    };
    assert!(head(&plain) == head(&captured), "the heads differ");
}

#[test]
fn the_piano_roll_and_spectrogram_are_written_and_labeled() {
    let f = Fixture::with_rom("audio-images", TUNE);
    let piano = path_in(&f, "piano.png");
    let spectrogram = path_in(&f, "spec.png");
    let audio = audio_of(run(
        &mut handler(&f),
        tune_run(AudioArgs {
            piano: Some(ImageOut {
                path: piano.clone(),
                scale: None,
            }),
            spectrogram: Some(ImageOut {
                path: spectrogram.clone(),
                scale: Some(2),
            }),
            ..AudioArgs::default()
        }),
    ));
    assert!(
        audio.log.is_none() && audio.wavs.is_empty(),
        "only what was asked"
    );
    let roll = audio.piano.expect("a piano roll");
    // The roll's default scale is 2 (a short clip at 1 was a few dots, agent
    // eval run 3); its native width is capped, so a long clip stays bounded.
    assert_eq!((roll.path.as_str(), roll.scale), (piano.as_str(), 2));
    // Four native columns a frame, at scale 2; the run starts mid-frame.
    let width = u64::from(roll.width);
    assert!(
        (8 * (TUNE_FRAMES - 1)..=8 * TUNE_FRAMES).contains(&width),
        "{roll:?}"
    );
    let spec = audio.spectrogram.expect("a spectrogram");
    assert_eq!(spec.scale, 2);
    assert_eq!(spec.height, 2 * 256);
    for (path, view) in [(&piano, "piano"), (&spectrogram, "spectrogram")] {
        let bytes = fs::read(path).expect("written");
        let label = format!("gbcli:view\0{view}");
        assert!(
            bytes.windows(label.len()).any(|w| w == label.as_bytes()),
            "{path} is labeled {view}"
        );
    }
}

#[test]
fn audio_refusals_come_before_the_machine_moves() {
    let f = Fixture::with_rom("audio-refusals", TUNE);
    let mut h = handler(&f);
    let missing = path_in(&f, "no-such-dir/tune.wav");
    let failure = run(&mut h, tune_run(wav_at(missing, WavChannels::Mixed)))
        .outcome
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::BadRequest);
    assert!(
        failure.message.contains("no-such-dir"),
        "{}",
        failure.message
    );
    assert!(failure.stop.is_none(), "refused before the run");

    let bad_scale = AudioArgs {
        spectrogram: Some(ImageOut {
            path: path_in(&f, "s.png"),
            scale: Some(0),
        }),
        ..AudioArgs::default()
    };
    let failure = run(&mut h, tune_run(bad_scale)).outcome.unwrap_err();
    assert_eq!(failure.code, ErrorCode::BadRequest);
    assert!(failure.stop.is_none());
}

#[test]
fn the_sgb_refuses_capture_before_the_run_and_records_the_gap() {
    let f = Fixture::with_model("audio-sgb", TUNE, Model::Sgb);
    let mut h = handler(&f);
    let path = path_in(&f, "tune.wav");
    let failure = run(&mut h, tune_run(wav_at(path, WavChannels::Mixed)))
        .outcome
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::Unsupported);
    assert!(failure.stop.is_none(), "refused before the run");
    assert!(failure.message.contains("SGB"), "{}", failure.message);
    assert!(failure.remedy.contains("--audio-log"), "{}", failure.remedy);
    let gaps = h.handle(&request(Op::Gaps)).outcome.expect("gaps");
    let gaps = gaps.gaps.expect("rows");
    assert!(
        gaps.iter().any(|g| g.feature == "audio_samples"),
        "{gaps:?}"
    );

    // The write log needs no capture, so it still works on the SGB.
    let log = AudioArgs {
        log: true,
        ..AudioArgs::default()
    };
    let audio = audio_of(run(&mut h, tune_run(log)));
    assert_eq!(audio.log.expect("a log").len(), 43);
}
