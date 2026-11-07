//! WAV export, per-channel takes, the piano roll and the spectrograms, on the
//! tune ROM. Images and WAVs are pinned byte for byte; see `common` to
//! regenerate them.

mod common;

use common::*;
use gbcli_audio::{
    PianoRoll, Takes, decode, note_spans, piano_roll, record_takes, spectrogram, wav,
};
use gbcli_core::{ApuChannel, AudioCapture, Budget, Core, Ticks};
use gbcli_sameboy::SameBoy;

/// The state the takes start from: the tune just before its first note.
fn start_state() -> Vec<u8> {
    tune_core().save()
}

fn takes() -> Takes {
    let start = start_state();
    record_takes(
        || {
            let mut core = tune_core();
            core.load(&start)?;
            Ok(core)
        },
        |core: &mut SameBoy| {
            core.run(&mut gbcli_core::RunRequest::new(Budget::frames(
                TUNE_FRAMES,
            )))
            .map(|_| ())
        },
    )
    .unwrap()
}

/// The PCM payload of a 16-bit WAV this crate wrote, as samples.
fn pcm(bytes: &[u8]) -> Vec<i16> {
    assert_eq!(&bytes[36..40], b"data");
    bytes[44..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&b| i16::from_le_bytes(b))
        .collect()
}

#[test]
fn a_replay_is_exactly_deterministic() {
    // Before trusting per-channel takes from replays: the same state and run,
    // replayed, give the same samples bit for bit, mixed and soloed.
    let (a, b) = (takes(), takes());
    assert_eq!(a, b);
    assert!(a.mixed.samples.len() > 15_000);
}

#[test]
fn per_channel_wavs_sum_to_the_mixed_wav_exactly() {
    // Criterion 0Jm. SameBoy mixes by adding the channels' band-limited
    // outputs, and muting zeroes a channel's input, so with the high-pass
    // filter off (SameBoy's default, which the core keeps) the sum is exact.
    let takes = takes();
    assert_eq!(takes.residual(), 0);
    let mixed = pcm(&wav(&takes.mixed.samples));
    let channels: Vec<Vec<i16>> = ApuChannel::ALL
        .iter()
        .map(|&c| pcm(&wav(&takes.channel(c).samples)))
        .collect();
    for (i, m) in mixed.iter().enumerate() {
        let sum: i32 = channels.iter().map(|c| i32::from(c[i])).sum();
        assert_eq!(sum, i32::from(*m), "PCM value {i}");
    }
    assert!(
        channels.iter().all(|c| c.iter().any(|&v| v != 0)),
        "every channel plays"
    );
}

#[test]
fn a_replay_that_diverges_is_refused() {
    let start = start_state();
    let mut runs = 0u64;
    let result = record_takes(
        || {
            let mut core = tune_core();
            core.load(&start)?;
            Ok(core)
        },
        |core: &mut SameBoy| {
            runs += 1;
            core.run(&mut gbcli_core::RunRequest::new(Budget::frames(runs)))
                .map(|_| ())
        },
    );
    assert!(result.is_err());
}

fn tune_spans() -> (Vec<gbcli_audio::NoteSpan>, Ticks, Ticks) {
    let mut core = tune_core();
    let snapshot = gbcli_audio::ApuSnapshot::from_core(&core).unwrap();
    core.set_apu_write_log(true).unwrap();
    let from = core.now();
    run(&mut core, Budget::frames(TUNE_FRAMES));
    let log = decode(&snapshot, &core.take_apu_writes().unwrap());
    (note_spans(&log, core.now()), from, core.now())
}

#[test]
fn the_tunes_notes_end_where_their_registers_say() {
    // Hand-checked from tune.asm: square 2's length of 32 is 32/256 s; the
    // noise envelope falls from 10 at one step per 1/64 s, so 10/64 s; square
    // 1's A4 is cut by the C5 trigger 15 frames later; the C5's upward sweep
    // (period += period >> 7 every 1/128 s) runs 1798, 1812, ... 2029, and
    // the 17th step writes 2044, whose next value would pass 2047, so the
    // channel stops there (Pan Docs, "Frequency sweep"): 16 steps after the
    // trigger, each its own span. The wave plays until its DAC is turned off.
    let (spans, _, _) = tune_spans();
    let of = |c: ApuChannel| spans.iter().filter(|s| s.channel == c).collect::<Vec<_>>();
    let secs = |t: Ticks| t.0 as f64 / Ticks::PER_SECOND as f64;
    let square2 = of(ApuChannel::Square2);
    assert_eq!(square2.len(), 1);
    assert_eq!(square2[0].note.to_string(), "E4");
    assert!((secs(square2[0].end - square2[0].start) - 32.0 / 256.0).abs() < 1e-9);
    let noise = of(ApuChannel::Noise);
    assert_eq!(noise.len(), 1);
    assert!((secs(noise[0].end - noise[0].start) - 10.0 / 64.0).abs() < 1e-9);
    let square1 = of(ApuChannel::Square1);
    assert_eq!(square1[0].note.to_string(), "A4");
    // NR13 $06 lands while A4 still plays, so for the two instructions before
    // the trigger the channel plays period $606 (C4): a span of its own.
    assert_eq!(square1[1].note.to_string(), "C4");
    assert!(square1[1].end - square1[1].start < Ticks(64));
    assert_eq!(square1[2].note.to_string(), "C5");
    assert_eq!(
        square1.len(),
        3 + 16,
        "A4, C4, then C5 and its 16 sweep steps"
    );
    assert_eq!(square1.last().unwrap().period, Some(2029));
    assert!((secs(square1[2].end - square1[2].start) - 1.0 / 128.0).abs() < 1e-9);
    assert_eq!(square1[0].end, square1[1].start);
    assert_eq!(square1[1].end, square1[2].start);
    let wave = of(ApuChannel::Wave);
    assert_eq!(
        (wave.len(), wave[0].note.to_string()),
        (1, "G3".to_string())
    );
    assert!(secs(wave[0].end - wave[0].start) > 29.0 / 59.7);
}

#[test]
fn the_piano_roll_is_pinned() {
    let (spans, from, to) = tune_spans();
    let image = piano_roll(&spans, from, to, &PianoRoll::default()).unwrap();
    assert_golden("tune_piano.png", &image.png().unwrap());
}

#[test]
fn the_mixed_and_square_1_spectrograms_are_pinned() {
    let takes = takes();
    let mixed = spectrogram(&takes.mixed, "mixed").unwrap();
    assert_golden("tune_spectrogram_mixed.png", &mixed.png().unwrap());
    let square1 = spectrogram(takes.channel(ApuChannel::Square1), "square 1").unwrap();
    assert_golden("tune_spectrogram_square1.png", &square1.png().unwrap());
}

#[test]
fn a_spectrogram_puts_a_tones_energy_in_its_row() {
    // The wave channel alone plays G3 (196.2 Hz) with a triangle: its
    // loudest row is the fundamental's, 32 Hz per row from the bottom.
    let takes = takes();
    let spectrum = gbcli_audio::spectrum(takes.channel(ApuChannel::Wave)).unwrap();
    let column = spectrum.columns / 4;
    let loudest = (0..gbcli_audio::ROWS)
        .max_by_key(|&row| (spectrum.level(column, row), std::cmp::Reverse(row)))
        .unwrap();
    let hz = loudest as f64 * f64::from(gbcli_audio::HZ_PER_ROW);
    assert!((hz - 196.2).abs() <= 32.0, "{hz} Hz");
}

#[test]
fn the_mixed_wav_is_pinned() {
    // Pinned as a hash-sized golden: the first 2048 samples are enough to
    // catch a change in the encoder or the renderer.
    let takes = takes();
    let head = &takes.mixed.samples[..2048];
    assert_golden("tune_mixed_head.wav", &wav(head));
    let _ = AudioCapture::SAMPLE_RATE;
}
