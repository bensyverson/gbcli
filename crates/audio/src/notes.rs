//! Note spans from the decoded log: when each channel sounded which pitch.
//!
//! A note starts at a trigger and ends at the next trigger, a pitch change
//! (which starts a new span), its DAC or the APU turning off, or the end of the
//! log. Ends the log can't show are computed from the registers: length
//! expiry, an envelope falling to zero, and square 1's sweep (each step is a
//! span; overflow stops the channel). Those are timed from the trigger; the
//! frame sequencer's phase isn't known, so they can be early or late by up to
//! one of its steps (1/256 s for length, 1/128 s for the sweep, 1/64 s for the
//! envelope). Each channel's settings (DAC, envelope, wave level, noise,
//! period, length, NR10) start from the snapshot the log began at, so a log
//! that starts after the instruments were set still shows their notes. Notes
//! triggered before the log began aren't shown.

use gbcli_core::{ApuChannel, ApuWrite, ApuWriteEffect, Ticks};

use crate::decode::{ApuSnapshot, Change, DecodedWrite, decode};
use crate::pitch::{Note, Pitch, nearest_note};
use crate::reg::{Direction, Envelope, Sweep, WaveLevel};

const LENGTH_STEP: u64 = Ticks::PER_SECOND / 256;
const SWEEP_STEP: u64 = Ticks::PER_SECOND / 128;
const ENVELOPE_STEP: u64 = Ticks::PER_SECOND / 64;

/// One channel sounding one pitch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoteSpan {
    pub channel: ApuChannel,
    pub start: Ticks,
    pub end: Ticks,
    /// The nearest note; for the noise channel, the nearest to its LFSR clock.
    pub note: Note,
    pub hz: f64,
    /// The period, for the tone channels.
    pub period: Option<u16>,
    /// The volume at the start, 0-15 (the wave channel's level as 15, 8 or 4).
    pub volume: u8,
}

/// What the registers say about one channel, as the log has shown them.
#[derive(Clone, Copy, Debug, Default)]
struct Channel {
    dac: bool,
    envelope: Option<Envelope>,
    wave_level: Option<WaveLevel>,
    length: u16,
    length_enabled: bool,
    period: Option<u16>,
    noise_hz: Option<f64>,
    /// The note sounding now: its pitch source and the time it must end by.
    playing: Option<Playing>,
}

#[derive(Clone, Copy, Debug)]
struct Playing {
    start: Ticks,
    /// When length, envelope or sweep stops it, if any does.
    stop: Option<Ticks>,
    volume: u8,
    /// NR10 as of the trigger (square 1 only); later NR10 writes are not
    /// followed.
    sweep: Sweep,
}

/// The note spans in `log`, which `decode` read from `start`, closing any
/// still sounding at `end`.
pub fn note_spans(start: &ApuSnapshot, log: &[DecodedWrite], end: Ticks) -> Vec<NoteSpan> {
    let mut channels = [Channel::default(); 4];
    let mut sweep = Sweep::from_bits(0);
    let mut spans = Vec::new();
    // The snapshot's settings, as writes of what it holds with no trigger,
    // read by the same decoder and the same rules as the log; they can't
    // start a note, so they add no span.
    let at = log.first().map_or(end, |w| w.write.at);
    let seed = seeded(start, at);
    for w in seed.iter().chain(log) {
        if w.write.effect != ApuWriteEffect::Applied {
            continue;
        }
        let at = w.write.at;
        let c = |ch: ApuChannel| ch as usize;
        match w.change {
            Change::Power { on: false } => {
                for (i, ch) in channels.iter_mut().enumerate() {
                    close(&mut spans, ApuChannel::ALL[i], ch, at);
                    *ch = Channel::default();
                }
            }
            Change::Sweep(s) => sweep = s,
            Change::DutyLength {
                channel, length, ..
            }
            | Change::Length { channel, length } => channels[c(channel)].length = length,
            Change::Envelope { channel, envelope } => {
                let ch = &mut channels[c(channel)];
                ch.dac = true;
                ch.envelope = Some(envelope);
            }
            Change::DacOff { channel } => {
                let ch = &mut channels[c(channel)];
                close(&mut spans, channel, ch, at);
                ch.dac = false;
            }
            Change::WaveDac { on } => {
                let ch = &mut channels[c(ApuChannel::Wave)];
                if !on {
                    close(&mut spans, ApuChannel::Wave, ch, at);
                }
                ch.dac = on;
            }
            Change::WaveLevel(level) => channels[c(ApuChannel::Wave)].wave_level = Some(level),
            Change::Noise { hz, .. } => channels[c(ApuChannel::Noise)].noise_hz = Some(hz),
            Change::Period { channel, pitch } => {
                let ch = &mut channels[c(channel)];
                if ch.playing.is_some() && ch.period != Some(pitch.period) {
                    // A new pitch mid-note: the old one ends, the new one
                    // carries on to the same stop.
                    let playing = ch.playing.expect("checked");
                    close(&mut spans, channel, ch, at);
                    ch.playing = Some(Playing {
                        start: at,
                        ..playing
                    });
                }
                ch.period = Some(pitch.period);
            }
            Change::Control {
                channel,
                trigger,
                length_enabled,
                pitch,
            } => {
                let ch = &mut channels[c(channel)];
                if trigger {
                    close(&mut spans, channel, ch, at);
                }
                ch.length_enabled = length_enabled;
                if let Some(pitch) = pitch {
                    ch.period = Some(pitch.period);
                }
                if trigger && ch.dac {
                    let volume = match channel {
                        ApuChannel::Wave => ch.wave_level.map_or(0, WaveLevel::volume),
                        _ => ch.envelope.map_or(0, |e| e.initial),
                    };
                    let stop = stop_time(channel, ch, at);
                    ch.playing = (volume > 0).then_some(Playing {
                        start: at,
                        stop,
                        volume,
                        sweep,
                    });
                }
            }
            Change::Volume { .. } | Change::Panning { .. } | Change::WaveRam { .. } => {}
            Change::Power { on: true } => {}
        }
    }
    for (i, ch) in channels.iter_mut().enumerate() {
        close(&mut spans, ApuChannel::ALL[i], ch, end);
    }
    spans.sort_by_key(|s| (s.start, s.channel as usize));
    spans
}

/// The snapshot's NR10 to NR51 as writes at `at` to a powered APU, trigger
/// bits cleared; nothing when the APU is off, where every channel is off.
fn seeded(start: &ApuSnapshot, at: Ticks) -> Vec<DecodedWrite> {
    if !start.powered {
        return Vec::new();
    }
    let blank = ApuSnapshot {
        regs: [0; 0x17],
        powered: true,
    };
    let writes: Vec<ApuWrite> = (0xFF10u16..0xFF26)
        .map(|addr| {
            let value = start.regs[usize::from(addr - 0xFF10)];
            let trigger_reg = matches!(addr, 0xFF14 | 0xFF19 | 0xFF1E | 0xFF23);
            // Made up from the snapshot, so it fell in no frame or line.
            ApuWrite {
                at,
                frame: 0,
                line: None,
                pc: 0,
                addr,
                value: if trigger_reg { value & 0x7F } else { value },
                effect: ApuWriteEffect::Applied,
            }
        })
        .collect();
    decode(&blank, &writes)
}

/// When length or the envelope stops a note triggered at `at`, if either does.
fn stop_time(channel: ApuChannel, ch: &Channel, at: Ticks) -> Option<Ticks> {
    let length = ch
        .length_enabled
        .then(|| at + Ticks(u64::from(ch.length) * LENGTH_STEP));
    let envelope = match (channel, ch.envelope) {
        (ApuChannel::Wave, _) | (_, None) => None,
        (_, Some(e)) if e.direction == Direction::Down && e.pace > 0 => {
            Some(at + Ticks(u64::from(e.initial) * u64::from(e.pace) * ENVELOPE_STEP))
        }
        _ => None,
    };
    match (length, envelope) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

/// Ends the channel's note at `at` (or earlier, where it stopped by itself),
/// pushing its span, or its sweep steps' spans.
fn close(spans: &mut Vec<NoteSpan>, channel: ApuChannel, ch: &mut Channel, at: Ticks) {
    let Some(playing) = ch.playing.take() else {
        return;
    };
    let end = playing.stop.map_or(at, |s| s.min(at));
    let push = |spans: &mut Vec<NoteSpan>, start: Ticks, end: Ticks, period: Option<u16>| {
        if end <= start {
            return;
        }
        let hz = match (channel, period) {
            (ApuChannel::Noise, _) => ch.noise_hz.unwrap_or(0.0),
            (_, Some(p)) => Pitch::of_period(channel, p).map_or(0.0, |p| p.hz),
            (_, None) => 0.0,
        };
        if hz <= 0.0 {
            return;
        }
        spans.push(NoteSpan {
            channel,
            start,
            end,
            note: nearest_note(hz).0,
            hz,
            period,
            volume: playing.volume,
        });
    };
    let sweep = playing.sweep;
    let sweeping = channel == ApuChannel::Square1 && sweep.pace > 0 && sweep.shift > 0;
    let Some(mut period) = ch.period.filter(|_| sweeping) else {
        push(spans, playing.start, end, ch.period);
        return;
    };
    // Each sweep step is its own span; an upward step whose next value would
    // pass 2047 stops the channel as it is written.
    let step = Ticks(u64::from(sweep.pace) * SWEEP_STEP);
    let mut start = playing.start;
    loop {
        let next_start = start + step;
        if next_start >= end {
            push(spans, start, end, Some(period));
            break;
        }
        push(spans, start, next_start, Some(period));
        let delta = period >> sweep.shift;
        period = match sweep.direction {
            Direction::Up => period + delta,
            Direction::Down => period - delta,
        };
        if period > 2047
            || (sweep.direction == Direction::Up && period + (period >> sweep.shift) > 2047)
        {
            break;
        }
        start = next_start;
    }
    ch.period = Some(period.min(2047));
}
