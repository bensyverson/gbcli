//! The piano roll: one lane per channel, time across, pitch up.

use gbcli_core::{ApuChannel, Ticks};
use gbcli_view::{Image, LabelKey};

use crate::AudioError;
use crate::notes::NoteSpan;
use crate::pitch::Note;
use crate::reg::channel_name;

/// Pixels per semitone; a note is drawn two pixels tall with a one-pixel gap.
const ROW_PX: u32 = 3;
/// A lane shows at least an octave.
const MIN_SEMITONES: i32 = 12;
/// The widest roll drawn, so a long span can't make a huge image.
pub const MAX_WIDTH: u32 = 8192;

const BACKGROUND: u32 = 0x181818;
/// The row of each C, so octaves can be counted.
const C_ROW: u32 = 0x262626;
const SEPARATOR: u32 = 0x505050;
/// Square 1, square 2, wave, noise.
const CHANNEL_COLORS: [u32; 4] = [0xF05050, 0xF0B040, 0x50A8F0, 0x90D860];

/// How the roll is laid out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PianoRoll {
    /// Emulated time per pixel column.
    pub ticks_per_px: u64,
}

impl Default for PianoRoll {
    /// Four columns per frame.
    fn default() -> Self {
        PianoRoll {
            ticks_per_px: Ticks::PER_FRAME / 4,
        }
    }
}

/// `spans` from `from` to `to` as a piano roll: lanes for square 1, square 2,
/// wave and noise, top to bottom, each spanning its notes' range (at least an
/// octave), one row per semitone, with a darker line on each C. A note's
/// color is its channel's, darker the quieter it starts. The noise lane plots
/// the LFSR clock as if it were a pitch. The labels give each lane's range and
/// the time scale.
pub fn piano_roll(
    spans: &[NoteSpan],
    from: Ticks,
    to: Ticks,
    layout: &PianoRoll,
) -> Result<Image, AudioError> {
    if to <= from || layout.ticks_per_px == 0 {
        return Err(AudioError::EmptySpan);
    }
    let width = (to - from)
        .0
        .div_ceil(layout.ticks_per_px)
        .min(u64::from(MAX_WIDTH)) as u32;
    let lanes = ApuChannel::ALL.map(|c| Lane::of(c, spans));
    let height = lanes.iter().map(|l| l.height()).sum::<u32>() + 3;
    let mut image = Image::filled(width, height, BACKGROUND);
    let mut top = 0;
    let mut details = Vec::new();
    for lane in &lanes {
        for midi in lane.low..=lane.high {
            if midi.rem_euclid(12) == 0 {
                let y = top + lane.row(midi) + 1;
                fill(&mut image, 0, y, width, 1, C_ROW);
            }
        }
        for span in spans.iter().filter(|s| s.channel == lane.channel) {
            if span.end <= from || span.start >= to {
                continue;
            }
            let x0 = (span.start.max(from) - from).0 / layout.ticks_per_px;
            let x1 = (span.end.min(to) - from).0.div_ceil(layout.ticks_per_px);
            let x1 = x1.max(x0 + 1);
            let y = top + lane.row(span.note.midi());
            let color = shade(CHANNEL_COLORS[lane.channel as usize], span.volume);
            fill(
                &mut image,
                x0 as u32,
                y,
                (x1 - x0) as u32,
                ROW_PX - 1,
                color,
            );
        }
        details.push(format!(
            "{} {}-{}",
            channel_name(lane.channel),
            Note::from_midi(lane.low),
            Note::from_midi(lane.high)
        ));
        top += lane.height();
        if lane.channel != ApuChannel::Noise {
            fill(&mut image, 0, top, width, 1, SEPARATOR);
            top += 1;
        }
    }
    let detail = format!(
        "lanes top to bottom: {}; {ROW_PX} px per semitone; {} ticks per px from tick {} to {}; \
         note ends from length, envelope and sweep are computed from the registers",
        details.join(", "),
        layout.ticks_per_px,
        from.0,
        to.0
    );
    image
        .labels
        .extend([(LabelKey::View, "piano".into()), (LabelKey::Detail, detail)]);
    Ok(image)
}

/// One channel's lane: the semitones it shows, `high` at the top.
struct Lane {
    channel: ApuChannel,
    low: i32,
    high: i32,
}

impl Lane {
    fn of(channel: ApuChannel, spans: &[NoteSpan]) -> Lane {
        let notes = spans
            .iter()
            .filter(|s| s.channel == channel)
            .map(|s| s.note.midi());
        let (low, high) = notes.fold((i32::MAX, i32::MIN), |(lo, hi), m| (lo.min(m), hi.max(m)));
        let (mut low, mut high) = if low > high {
            (60, 60) // no notes: an empty octave from C4
        } else {
            (low - 1, high + 1)
        };
        while high - low + 1 < MIN_SEMITONES {
            high += 1;
            if high - low + 1 < MIN_SEMITONES {
                low -= 1;
            }
        }
        Lane { channel, low, high }
    }

    fn height(&self) -> u32 {
        (self.high - self.low + 1) as u32 * ROW_PX
    }

    /// The top pixel row of `midi` within the lane.
    fn row(&self, midi: i32) -> u32 {
        (self.high - midi.clamp(self.low, self.high)) as u32 * ROW_PX
    }
}

fn fill(image: &mut Image, x: u32, y: u32, w: u32, h: u32, color: u32) {
    for yy in y..(y + h).min(image.height) {
        for xx in x..(x + w).min(image.width) {
            image.pixels[(yy * image.width + xx) as usize] = color;
        }
    }
}

/// `color` dimmed for a volume of 0-15: full at 15, a quarter at 0.
fn shade(color: u32, volume: u8) -> u32 {
    let scale = 5 + u32::from(volume.min(15));
    let part = |shift: u32| ((color >> shift) & 0xFF) * scale / 20;
    (part(16) << 16) | (part(8) << 8) | part(0)
}
