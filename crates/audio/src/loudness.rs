//! How loud each take sounded, for `--audio-summary`: the median RMS of the
//! short windows it sounded in, with the windows that carry a pop left out.
//!
//! A DMG DAC outputs its full positive level for a digital 0 and SameBoy's
//! capture keeps the DC offset (no high-pass filter), so a peak or a plain
//! RMS of a WAV measures DAC state, not volume: switching a DAC on or off
//! steps the output by nearly full scale whatever the volume, and a capture
//! starts from silence, so a channel already sounding opens with a step too.
//! Each window's own mean is removed (its RMS is the AC part), the windows a
//! step falls in are left out (the first, and any around a DAC switch the
//! write log names), and the median of the rest ignores the odd envelope or
//! panning step that remains.

use gbcli_core::{ApuChannel, ApuWriteEffect, AudioCapture, AudioSamples, StereoSample, Ticks};
use gbcli_protocol::{Level, Loudness, Real, WavTake};

use crate::decode::{ApuSnapshot, Change, DecodedWrite};
use crate::replay::Takes;

/// Samples a window holds: 1/64 s at 32,768 Hz, long enough for one period
/// of the lowest tone a channel plays (64 Hz).
pub const WINDOW: usize = 512;

/// Below this a window is silence, in dBFS: a quieter tone than any channel
/// can play (volume 1, master volume 0, 12.5% duty is near -63).
pub const FLOOR_DBFS: f64 = -72.0;

/// [`FLOOR_DBFS`] as an RMS in 16-bit sample units, 32,768 x 10^(-72/20),
/// so the comparison needs no platform math.
const FLOOR_RMS: f64 = 8.23094945877059;

/// Samples either side of a DAC switch that its step may reach: SameBoy's
/// band-limited step spans 64 samples and a DMG delays the DAC by 32 more.
const GUARD: u64 = 128;

/// The ticks at which each channel's DAC switched on or off, in
/// [`ApuChannel::ALL`] order, from `log` decoded from `start`.
pub fn dac_switches(start: &ApuSnapshot, log: &[DecodedWrite]) -> [Vec<Ticks>; 4] {
    let mut on = ApuChannel::ALL.map(|c| start.dac_on(c));
    let mut switches: [Vec<Ticks>; 4] = Default::default();
    let mut set = |channel: ApuChannel, now: bool, at: Ticks| {
        let was = &mut on[channel as usize];
        if *was != now {
            *was = now;
            switches[channel as usize].push(at);
        }
    };
    for w in log {
        if w.write.effect != ApuWriteEffect::Applied {
            continue;
        }
        let at = w.write.at;
        match w.change {
            // Power off clears every register, so every DAC goes off.
            Change::Power { on: false } => {
                for channel in ApuChannel::ALL {
                    set(channel, false, at);
                }
            }
            Change::DacOff { channel } => set(channel, false, at),
            Change::Envelope { channel, .. } => set(channel, true, at),
            Change::WaveDac { on } => set(ApuChannel::Wave, on, at),
            _ => {}
        }
    }
    switches
}

/// `audio`'s loudness, leaving out the windows around `switches`.
pub fn loudness_of(take: WavTake, audio: &AudioSamples, switches: &[Ticks]) -> Loudness {
    let windows = audio.samples.len() / WINDOW;
    let mut skip = vec![false; windows];
    if let Some(first) = skip.first_mut() {
        *first = true;
    }
    if let Some(first) = audio.first {
        for at in switches {
            let at = at.0.saturating_sub(first.0) / AudioCapture::TICKS_PER_SAMPLE;
            let from = at.saturating_sub(GUARD) as usize / WINDOW;
            let to = (at + GUARD) as usize / WINDOW;
            for s in skip.iter_mut().take(to + 1).skip(from) {
                *s = true;
            }
        }
    }
    let measured = skip.iter().filter(|s| !**s).count();
    let mut sounding: Vec<f64> = audio
        .samples
        .as_chunks::<WINDOW>()
        .0
        .iter()
        .zip(&skip)
        .filter(|(_, skip)| !**skip)
        .map(|(window, _)| ac_rms(window))
        .filter(|&rms| rms >= FLOOR_RMS)
        .collect();
    sounding.sort_by(f64::total_cmp);
    let rms_dbfs = match sounding.get(sounding.len() / 2) {
        Some(&rms) => {
            let db = 20.0 * (rms / 32768.0).log10();
            Level::Dbfs(Real((db * 10.0).round() / 10.0))
        }
        None if measured == 0 => Level::Unmeasured,
        None => Level::Silent,
    };
    Loudness {
        take,
        rms_dbfs,
        sounding_s: Real(
            sounding.len() as f64 * WINDOW as f64 / f64::from(AudioCapture::SAMPLE_RATE),
        ),
    }
}

/// A window's RMS about its own mean, both sides together.
fn ac_rms(window: &[StereoSample]) -> f64 {
    let n = window.len() as f64;
    let side = |pick: fn(&StereoSample) -> i16| {
        let mean = window.iter().map(|s| f64::from(pick(s))).sum::<f64>() / n;
        window
            .iter()
            .map(|s| (f64::from(pick(s)) - mean).powi(2))
            .sum::<f64>()
    };
    ((side(|s| s.left) + side(|s| s.right)) / (2.0 * n)).sqrt()
}

/// The mix's loudness, then each of `channels`' (all four when empty),
/// loudest first and silent last. A channel's windows are gated around its
/// own DAC switches, the mix's around every channel's.
pub fn loudness(
    takes: &Takes,
    start: &ApuSnapshot,
    log: &[DecodedWrite],
    channels: &[ApuChannel],
) -> Vec<Loudness> {
    let switches = dac_switches(start, log);
    let mut every: Vec<Ticks> = switches.iter().flatten().copied().collect();
    every.sort();
    let mut rows: Vec<Loudness> = ApuChannel::ALL
        .into_iter()
        .filter(|c| channels.is_empty() || channels.contains(c))
        .map(|c| loudness_of(WavTake::solo(c), takes.channel(c), &switches[c as usize]))
        .collect();
    // Stable, so equal figures stay in channel order; `None` sorts last.
    rows.sort_by(|a, b| {
        let key = |l: &Loudness| l.rms_dbfs.dbfs().map(|r| r.0);
        match (key(a), key(b)) {
            (Some(x), Some(y)) => y.total_cmp(&x),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });
    rows.insert(0, loudness_of(WavTake::Mixed, &takes.mixed, &every));
    rows
}

#[cfg(test)]
mod tests {
    use gbcli_core::ApuWrite;

    use super::*;
    use crate::decode::decode;

    const FIRST: u64 = 1_000_000;

    /// Ticks at sample `i` of a capture whose first sample is at `FIRST`.
    fn tick_at(i: usize) -> Ticks {
        Ticks(FIRST + i as u64 * AudioCapture::TICKS_PER_SAMPLE)
    }

    fn capture(samples: Vec<i16>) -> AudioSamples {
        AudioSamples {
            first: Some(Ticks(FIRST)),
            samples: samples
                .into_iter()
                .map(|v| StereoSample { left: v, right: v })
                .collect(),
        }
    }

    /// A square of `amp` around `dc`, a period of 64 samples (512 Hz).
    fn square(dc: i16, amp: i16, n: usize) -> Vec<i16> {
        (0..n)
            .map(|i| if i % 64 < 32 { dc + amp } else { dc - amp })
            .collect()
    }

    fn dbfs(amp: f64) -> f64 {
        (20.0 * (amp / 32768.0).log10() * 10.0).round() / 10.0
    }

    #[test]
    fn a_steady_square_reads_as_its_ac_rms_whatever_its_dc_offset() {
        let l = loudness_of(
            WavTake::Square1,
            &capture(square(3000, 1000, WINDOW * 20)),
            &[],
        );
        assert_eq!(l.take, WavTake::Square1);
        assert_eq!(l.rms_dbfs, Level::Dbfs(Real(dbfs(1000.0))));
        // The first of the twenty windows is left out.
        assert_eq!(l.sounding_s, Real(19.0 / 64.0));
    }

    #[test]
    fn silence_a_dac_held_on_and_a_tone_under_the_floor_read_as_silent() {
        for level in [0, 3500] {
            let l = loudness_of(WavTake::Noise, &capture(vec![level; WINDOW * 8]), &[]);
            assert_eq!(
                (l.rms_dbfs, l.sounding_s),
                (Level::Silent, Real(0.0)),
                "{level}"
            );
        }
        // -72 dBFS is an amplitude of 8.2.
        let under = loudness_of(WavTake::Noise, &capture(square(3500, 8, WINDOW * 8)), &[]);
        assert_eq!(under.rms_dbfs, Level::Silent);
        let over = loudness_of(WavTake::Noise, &capture(square(3500, 9, WINDOW * 8)), &[]);
        assert_eq!(over.rms_dbfs, Level::Dbfs(Real(dbfs(9.0))));
    }

    #[test]
    fn the_linear_floor_is_floor_dbfs() {
        let floor = 32768.0 * 10f64.powf(FLOOR_DBFS / 20.0);
        assert!((FLOOR_RMS - floor).abs() < 1e-9, "{FLOOR_RMS} {floor}");
    }

    /// The first window is always left out, so a capture of under two
    /// windows (a one-frame run) has nothing to measure: unmeasured, not
    /// silent, though it sounded.
    #[test]
    fn a_capture_with_no_window_left_is_unmeasured_not_silent() {
        let empty = loudness_of(WavTake::Mixed, &AudioSamples::default(), &[]);
        assert_eq!(
            (empty.rms_dbfs, empty.sounding_s),
            (Level::Unmeasured, Real(0.0))
        );
        for n in [WINDOW, WINDOW * 2 - 1] {
            let short = loudness_of(WavTake::Square1, &capture(square(3000, 1000, n)), &[]);
            assert_eq!(short.rms_dbfs, Level::Unmeasured, "{n} samples");
        }
        let two = loudness_of(
            WavTake::Square1,
            &capture(square(3000, 1000, WINDOW * 2)),
            &[],
        );
        assert_eq!(two.rms_dbfs, Level::Dbfs(Real(dbfs(1000.0))));
        // A switch beside every window but the first leaves none either.
        let n = WINDOW * 3;
        let switches = [tick_at(WINDOW + 10), tick_at(WINDOW * 2 + 10)];
        let gated = loudness_of(WavTake::Square2, &capture(square(3000, 1000, n)), &switches);
        assert_eq!(gated.rms_dbfs, Level::Unmeasured);
        // Measured windows, none sounding: silent.
        let quiet = loudness_of(WavTake::Square2, &capture(vec![3500; n]), &switches[..1]);
        assert_eq!(quiet.rms_dbfs, Level::Silent);
    }

    #[test]
    fn the_opening_step_is_left_out() {
        // The capture starts from silence and steps to the DAC's level in
        // its first window, then plays a quiet tone for one window and
        // holds. Counted, the step's window would outvote the tone's.
        let mut samples = vec![0; 100];
        samples.extend(vec![3500; WINDOW - 100]);
        samples.extend(square(3500, 100, WINDOW));
        samples.extend(vec![3500; WINDOW * 6]);
        let l = loudness_of(WavTake::Wave, &capture(samples), &[]);
        assert_eq!(l.rms_dbfs, Level::Dbfs(Real(dbfs(100.0))));
        assert_eq!(l.sounding_s, Real(1.0 / 64.0));
    }

    /// A quiet tone whose DAC switches on and off, two windows each: each
    /// cycle has one clean window and two that pop.
    fn popping(cycles: usize) -> (Vec<i16>, Vec<Ticks>) {
        let mut samples = Vec::new();
        let mut switches = Vec::new();
        for _ in 0..cycles {
            // Mid-window, as a write rarely lands on a window's edge.
            switches.push(tick_at(samples.len() + 200));
            samples.extend(vec![0; 200]);
            samples.extend(square(3500, 250, WINDOW * 2));
            switches.push(tick_at(samples.len()));
            samples.extend(vec![0; WINDOW * 2 - 200]);
        }
        (samples, switches)
    }

    #[test]
    fn a_dac_switching_pops_but_its_windows_are_left_out() {
        let (samples, switches) = popping(8);
        let quiet = loudness_of(WavTake::Square2, &capture(samples.clone()), &switches);
        assert_eq!(quiet.rms_dbfs, Level::Dbfs(Real(dbfs(250.0))));
        let loud = loudness_of(
            WavTake::Square1,
            &capture(square(1500, 1000, samples.len())),
            &[],
        );
        assert!(
            loud.rms_dbfs.dbfs() > quiet.rms_dbfs.dbfs(),
            "{loud:?} {quiet:?}"
        );
        // Without the switches, the pops' windows outvote the tone's.
        let ungated = loudness_of(WavTake::Square2, &capture(samples), &[]);
        assert_ne!(ungated.rms_dbfs, quiet.rms_dbfs);
    }

    fn write(at: u64, addr: u16, value: u8) -> ApuWrite {
        ApuWrite {
            at: Ticks(at),
            frame: 0,
            line: None,
            pc: 0x200,
            boot_rom: false,
            addr,
            value,
            effect: ApuWriteEffect::Applied,
        }
    }

    #[test]
    fn the_log_names_each_dac_switch_and_only_switches() {
        // Square 1's DAC on, the rest off, when the log begins.
        let mut regs = [0; 0x17];
        regs[0x02] = 0xF0;
        let start = ApuSnapshot {
            regs,
            powered: true,
        };
        let writes = [
            write(10, 0xFF17, 0x10), // NR22: square 2 DAC on
            write(20, 0xFF17, 0x20), // NR22 again: already on
            write(30, 0xFF12, 0xA0), // NR12: square 1 already on
            write(40, 0xFF1A, 0x80), // NR30: wave DAC on
            write(50, 0xFF21, 0x00), // NR42: noise already off
            write(60, 0xFF17, 0x07), // NR22: bits 3-7 clear, off
            ApuWrite {
                effect: ApuWriteEffect::IgnoredApuOff,
                ..write(70, 0xFF21, 0xF0)
            },
            write(80, 0xFF26, 0x00), // NR52: power off, DACs with it
            write(90, 0xFF26, 0x80), // power on: every DAC still off
        ];
        let log = decode(&start, &writes);
        let t = |ts: &[u64]| ts.iter().map(|&t| Ticks(t)).collect::<Vec<_>>();
        assert_eq!(
            dac_switches(&start, &log),
            [t(&[80]), t(&[10, 60]), t(&[40, 80]), t(&[])]
        );
    }

    #[test]
    fn the_mix_comes_first_then_the_channels_loudest_first() {
        let n = WINDOW * 10;
        let mut channels: [AudioSamples; 4] = Default::default();
        channels[ApuChannel::Square1 as usize] = capture(square(0, 300, n));
        channels[ApuChannel::Square2 as usize] = capture(vec![0; n]);
        channels[ApuChannel::Wave as usize] = capture(square(0, 2000, n));
        channels[ApuChannel::Noise as usize] = capture(square(0, 900, n));
        let takes = Takes {
            mixed: capture(square(0, 3200, n)),
            channels,
        };
        let start = ApuSnapshot {
            regs: [0; 0x17],
            powered: true,
        };
        let order = |filter: &[ApuChannel]| -> Vec<WavTake> {
            loudness(&takes, &start, &[], filter)
                .iter()
                .map(|l| l.take)
                .collect()
        };
        assert_eq!(
            order(&[]),
            [
                WavTake::Mixed,
                WavTake::Wave,
                WavTake::Noise,
                WavTake::Square1,
                WavTake::Square2
            ]
        );
        assert_eq!(
            order(&[ApuChannel::Square2, ApuChannel::Square1]),
            [WavTake::Mixed, WavTake::Square1, WavTake::Square2]
        );
    }
}
