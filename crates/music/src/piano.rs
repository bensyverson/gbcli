//! A piano roll of a song, drawn from the file without building or playing
//! it: the orders are walked once, row by row, and each note lasts until
//! its channel's next note or a note cut. Speed (F), position jump (B),
//! pattern break (D), note delay (7), note cut (E) and set volume (C) are
//! followed; instrument length, envelope, subpatterns and pitch effects are
//! not. Tone pitches come from hUGEDriver's period table, so the roll
//! agrees with `run --piano` on the same notes.

use gbcli_audio::{AudioError, MAX_WIDTH, Note, NoteSpan, PianoRoll, Pitch, SpanSource};
use gbcli_core::{ApuChannel, Ticks};
use gbcli_view::Image;

use crate::model::{BANK_SIZE, Bank, Cell, HIGHEST_NOTE, NO_NOTE, ROWS, Song};
use crate::notes::period_of;

/// A drawn piano roll and what it covers.
#[derive(Clone, Debug)]
pub struct PianoMade {
    pub image: Image,
    /// Notes drawn.
    pub notes: usize,
    /// Rows played, through every order once.
    pub rows: u32,
}

/// `song` as a piano roll: [`gbcli_audio::piano_roll`]'s lanes and colors,
/// scaled so the whole song fits [`MAX_WIDTH`] (one column per driver tick
/// when it already does).
pub fn piano_roll(song: &Song) -> Result<PianoMade, AudioError> {
    let played = play(song);
    let tick = tick_length(song);
    let to = Ticks(played.ticks * tick);
    let ticks_per_px = tick.max(to.0.div_ceil(u64::from(MAX_WIDTH)));
    let layout = PianoRoll {
        ticks_per_px,
        source: SpanSource::Song,
    };
    let spans: Vec<NoteSpan> = played
        .notes
        .iter()
        .filter(|n| n.end > n.start)
        .map(|n| span(n, tick))
        .collect();
    let image = gbcli_audio::piano_roll(&spans, Ticks(0), to, &layout)?;
    Ok(PianoMade {
        image,
        notes: spans.len(),
        rows: played.rows,
    })
}

/// Emulated time per driver tick: a frame, or a timer period when the
/// song runs from the timer (4096 / (256 − divider) Hz).
fn tick_length(song: &Song) -> u64 {
    if song.timer_enabled == 1 {
        let divider = song.timer_divider.clamp(0, 255) as u64;
        Ticks::PER_SECOND * (256 - divider) / 4096
    } else {
        Ticks::PER_FRAME
    }
}

/// One note as the walk found it, in driver ticks.
struct Played {
    channel: usize,
    start: u64,
    end: u64,
    note: i32,
    volume: u8,
}

struct Walk {
    notes: Vec<Played>,
    /// Driver ticks.
    ticks: u64,
    rows: u32,
}

/// Each channel's note sounding now, as an index into the notes.
type Sounding = [Option<usize>; 4];

fn play(song: &Song) -> Walk {
    let mut normal = song.clone();
    normal.normalize();
    let song = &normal;
    let orders = song.orders[0].len();
    let mut walk = Walk {
        notes: Vec::new(),
        ticks: 0,
        rows: 0,
    };
    let mut sounding: Sounding = [None; 4];
    let mut instrument = [0i32; 4];
    let mut visited = vec![false; orders];
    let mut speed = u64::try_from(song.ticks_per_row).unwrap_or(1).max(1);
    let (mut pos, mut start_row) = (0, 0);
    while pos < orders && !visited[pos] {
        visited[pos] = true;
        let cells = |row: usize, c: usize| -> Cell {
            song.pattern(song.orders[c][pos])
                .map_or(Cell::BLANK, |p| p.rows[row])
        };
        let mut next = None;
        for row in start_row..ROWS {
            // hUGEDriver's row_break and next_order: both offset by one,
            // so 0 means "not set" (hUGEDriver.asm, fx_pos_jump and
            // fx_pattern_break, and the row-end code).
            let (mut row_break, mut next_order) = (0usize, 0usize);
            walk.rows += 1;
            for c in 0..4 {
                let cell = cells(row, c);
                if cell.effect_code == 0xF && cell.effect_param > 0 {
                    speed = u64::from(cell.effect_param);
                }
            }
            let t = walk.ticks;
            for c in 0..4 {
                let cell = cells(row, c);
                if cell.instrument != 0 {
                    instrument[c] = cell.instrument;
                }
                let param = u64::from(cell.effect_param);
                // hUGEDriver acts on a tick of the row, 0 to speed - 1:
                // note delay plays at tick xx and never at 0 (fx_note_delay),
                // and note cut fires at tick xx (fx_note_cut). A tick the row
                // never reaches never plays the note, or never cuts it.
                let within = param < speed;
                let plays = match cell.effect_code {
                    7 => within && param > 0,
                    _ => true,
                };
                let at = if cell.effect_code == 7 { t + param } else { t };
                if plays && cell.note != NO_NOTE && (0..=HIGHEST_NOTE).contains(&cell.note) {
                    stop(&mut walk.notes, &mut sounding, c, at);
                    let volume = match (cell.effect_code, cell.effect_param) {
                        (0xC, p) => p & 0xF,
                        _ => volume(song, c, instrument[c]),
                    };
                    sounding[c] = Some(walk.notes.len());
                    walk.notes.push(Played {
                        channel: c,
                        start: at,
                        end: u64::MAX,
                        note: cell.note,
                        volume,
                    });
                }
                match cell.effect_code {
                    0xE if within => stop(&mut walk.notes, &mut sounding, c, t + param),
                    0xB => {
                        row_break = row_break.max(1);
                        next_order = usize::from(cell.effect_param);
                    }
                    0xD => row_break = usize::from(cell.effect_param),
                    _ => {}
                }
            }
            walk.ticks += speed;
            if row_break != 0 {
                let order = if next_order == 0 {
                    pos + 1
                } else {
                    next_order - 1
                };
                next = Some((order, (row_break - 1).min(ROWS - 1)));
                break;
            }
        }
        (pos, start_row) = next.unwrap_or((pos + 1, 0));
    }
    for c in 0..4 {
        stop(&mut walk.notes, &mut sounding, c, walk.ticks);
    }
    walk
}

/// Ends channel `c`'s sounding note at `at`.
fn stop(notes: &mut [Played], sounding: &mut Sounding, c: usize, at: u64) {
    if let Some(i) = sounding[c].take() {
        notes[i].end = at.max(notes[i].start);
    }
}

/// The volume an instrument starts a note at: its envelope's initial
/// volume, or the wave level as 15, 8 or 4.
fn volume(song: &Song, channel: usize, instrument: i32) -> u8 {
    let bank = Bank::of_channel(channel);
    let Some(ins) = instrument
        .checked_sub(1)
        .and_then(|i| usize::try_from(i).ok())
        .filter(|i| *i < BANK_SIZE)
        .map(|i| &song.instruments[bank as usize * BANK_SIZE + i])
    else {
        return 15;
    };
    match bank {
        Bank::Wave => match ins.output_level {
            1 => 15,
            2 => 8,
            3 => 4,
            _ => 0,
        },
        _ => ins.initial_volume.min(15),
    }
}

fn span(n: &Played, tick: u64) -> NoteSpan {
    let channel = ApuChannel::ALL[n.channel];
    let pitch = period_of(n.note).and_then(|p| Pitch::of_period(channel, p));
    let (note, hz, period) = match pitch {
        Some(p) => (p.note, p.hz, Some(p.period)),
        // The noise channel: the written note (C-3 at MIDI 48).
        None => {
            let midi = n.note + 48;
            let hz = 440.0 * 2f64.powf(f64::from(midi - 69) / 12.0);
            (Note::from_midi(midi), hz, None)
        }
    };
    NoteSpan {
        channel,
        start: Ticks(n.start * tick),
        end: Ticks(n.end.min(u64::MAX / tick) * tick),
        note,
        hz,
        period,
        volume: n.volume,
    }
}
