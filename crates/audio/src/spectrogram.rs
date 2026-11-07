//! Spectrograms: time across, frequency up, loudness as color.

use gbcli_core::{AudioCapture, AudioSamples, Ticks};
use gbcli_view::{Image, LabelKey};

use crate::AudioError;
use crate::fft::{N, Twiddles, power_spectrum};

/// Samples between columns: half a window, 15.6 ms at the capture rate.
pub const HOP: usize = N / 2;
/// Frequency rows, from 0 Hz at the bottom; each is one FFT bin of
/// [`HZ_PER_ROW`], so the top row is 8160 Hz.
pub const ROWS: usize = 256;
pub const HZ_PER_ROW: u32 = AudioCapture::SAMPLE_RATE / N as u32;
/// Loudness levels, from below [`FLOOR_DB`] (0) to full scale (LEVELS - 1).
pub const LEVELS: usize = 64;
pub const FLOOR_DB: i32 = -90;

/// The power of a full-scale sine under the Hann window: amplitude 1 peaks
/// at N/4 (the window sums to N/2, and a real sine splits in two).
const FULL_SCALE_POWER: f64 = (N as f64 / 4.0) * (N as f64 / 4.0);
/// 10^(-90/10): the floor as a power ratio.
const FLOOR_RATIO: f64 = 1e-9;
/// 10^((90/63)/10): one level's step as a power ratio (Python's `math`).
const LEVEL_RATIO: f64 = 1.389_495_494_373_137_7;

/// A spectrogram as levels, before coloring.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spectrum {
    pub columns: usize,
    /// `levels[column * ROWS + row]`, row 0 being 0 Hz.
    pub levels: Vec<u8>,
}

impl Spectrum {
    pub fn level(&self, column: usize, row: usize) -> u8 {
        self.levels[column * ROWS + row]
    }
}

/// The levels of `audio`'s mono mix ((left + right) / 2): one column per
/// [`HOP`] samples, each a Hann-windowed 1024-sample FFT starting there
/// (zero past the end).
pub fn spectrum(audio: &AudioSamples) -> Result<Spectrum, AudioError> {
    let n = audio.samples.len();
    if n == 0 {
        return Err(AudioError::NoSamples);
    }
    let mono: Vec<f64> = audio
        .samples
        .iter()
        .map(|s| (f64::from(s.left) + f64::from(s.right)) / 65_536.0)
        .collect();
    let twiddles = Twiddles::new();
    let thresholds = thresholds();
    let columns = n.div_ceil(HOP);
    let mut levels = Vec::with_capacity(columns * ROWS);
    let mut window = vec![0.0; N];
    for column in 0..columns {
        let start = column * HOP;
        for (i, w) in window.iter_mut().enumerate() {
            *w = mono.get(start + i).map_or(0.0, |x| x * twiddles.hann(i));
        }
        let power = power_spectrum(&twiddles, &window);
        levels.extend(
            power[..ROWS]
                .iter()
                .map(|&p| thresholds.partition_point(|&t| t <= p) as u8),
        );
    }
    Ok(Spectrum { columns, levels })
}

/// The power at which each level from 1 up begins.
fn thresholds() -> Vec<f64> {
    let mut t = Vec::with_capacity(LEVELS - 1);
    let mut power = FULL_SCALE_POWER * FLOOR_RATIO;
    for _ in 1..LEVELS {
        t.push(power);
        power *= LEVEL_RATIO;
    }
    t
}

/// `audio`'s spectrogram as an image: [`spectrum`]'s columns left to right,
/// 0 Hz at the bottom, each level a color from black through blue, purple,
/// red and yellow to white. `title` names what was captured (a channel, the
/// mix) in the image's labels.
pub fn spectrogram(audio: &AudioSamples, title: &str) -> Result<Image, AudioError> {
    let spectrum = spectrum(audio)?;
    let palette = palette();
    let (width, height) = (spectrum.columns as u32, ROWS as u32);
    let mut pixels = Vec::with_capacity(spectrum.levels.len());
    for y in 0..ROWS {
        let row = ROWS - 1 - y;
        pixels.extend((0..spectrum.columns).map(|c| palette[usize::from(spectrum.level(c, row))]));
    }
    let first = audio.first.unwrap_or(Ticks(0)).0;
    Ok(Image {
        width,
        height,
        pixels,
        labels: vec![
            (LabelKey::View, "spectrogram".into()),
            (
                LabelKey::Detail,
                format!(
                    "{title}; {} Hz mono, {N}-sample Hann window every {HOP} samples per \
                     column, {HZ_PER_ROW} Hz per row from 0 Hz at the bottom, {LEVELS} levels \
                     from {FLOOR_DB} dBFS to full scale; first sample at tick {first}",
                    AudioCapture::SAMPLE_RATE
                ),
            ),
        ],
    })
}

/// The level colors, interpolated in integer steps between the stops.
fn palette() -> Vec<u32> {
    const STOPS: [u32; 6] = [0x000000, 0x1A1A80, 0x8020A0, 0xE03030, 0xF0C020, 0xFFFFFF];
    let segments = (STOPS.len() - 1) as u32;
    (0..LEVELS as u32)
        .map(|level| {
            let pos = level * segments * 256 / (LEVELS as u32 - 1);
            let (i, frac) = (
                (pos / 256).min(segments - 1) as usize,
                pos - 256 * (pos / 256).min(segments - 1),
            );
            let mix = |shift: u32| {
                let (a, b) = ((STOPS[i] >> shift) & 0xFF, (STOPS[i + 1] >> shift) & 0xFF);
                (a * (256 - frac) + b * frac) / 256
            };
            (mix(16) << 16) | (mix(8) << 8) | mix(0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gbcli_core::StereoSample;

    #[test]
    fn the_palette_runs_from_black_to_white() {
        let p = palette();
        assert_eq!((p[0], p[LEVELS - 1]), (0x000000, 0xFFFFFF));
    }

    #[test]
    fn silence_is_level_zero_and_a_full_scale_square_reaches_the_top() {
        let silent = AudioSamples {
            first: Some(Ticks(0)),
            samples: vec![StereoSample::default(); 2048],
        };
        assert!(spectrum(&silent).unwrap().levels.iter().all(|&l| l == 0));
        // 512 Hz (bin 16) at full scale: 64-sample periods.
        let square: Vec<StereoSample> = (0..2048)
            .map(|i| {
                let v = if i % 64 < 32 { i16::MAX } else { -i16::MAX };
                StereoSample { left: v, right: v }
            })
            .collect();
        let s = spectrum(&AudioSamples {
            first: Some(Ticks(0)),
            samples: square,
        })
        .unwrap();
        assert_eq!(s.columns, 4);
        assert_eq!(s.level(0, 16), (LEVELS - 1) as u8);
        assert!(
            s.level(0, 16) > s.level(0, 48),
            "the third harmonic is quieter"
        );
    }
}
