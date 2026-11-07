//! Writing `.uge` v6.

use super::{FORMAT_VERSION, SHORT_STRING};
use crate::model::{Cell, Rows, ShortString, Song};

/// The song as a `.uge` version 6 file, as hUGETracker's
/// `WriteSongToStream` lays it out. `ShortString` tails and each channel's
/// extra last order entry are written as zeros (see the module note).
pub fn write_uge(song: &Song) -> Vec<u8> {
    let mut w = Writer(Vec::with_capacity(
        64 * 1024 + song.patterns.len() * (4 + 64 * super::CELL_V6),
    ));
    w.u32(FORMAT_VERSION);
    w.short_string(&song.name);
    w.short_string(&song.artist);
    w.short_string(&song.comment);
    for ins in &song.instruments {
        w.i32(ins.type_code);
        w.short_string(&ins.name);
        w.i32(ins.length);
        w.0.push(ins.length_enabled);
        w.0.push(ins.initial_volume);
        w.i32(ins.volume_direction);
        w.0.push(ins.volume_pace);
        w.i32(ins.sweep_time);
        w.i32(ins.sweep_direction);
        w.i32(ins.sweep_shift);
        w.0.push(ins.duty);
        w.i32(ins.output_level);
        w.i32(ins.waveform);
        w.i32(ins.counter_step);
        w.0.push(ins.subpattern_enabled);
        w.rows(&ins.subpattern);
    }
    for wave in &song.waves {
        w.0.extend(wave);
    }
    w.i32(song.ticks_per_row);
    w.0.push(song.timer_enabled);
    w.i32(song.timer_divider);
    w.len(song.patterns.len());
    for pattern in &song.patterns {
        w.i32(pattern.key);
        w.rows(&pattern.rows);
    }
    for order in &song.orders {
        w.len(order.len() + 1);
        for key in order {
            w.i32(*key);
        }
        w.i32(0);
    }
    for routine in &song.routines {
        w.len(routine.len());
        w.0.extend(routine);
    }
    w.0
}

struct Writer(Vec<u8>);

impl Writer {
    fn u32(&mut self, v: u32) {
        self.0.extend(v.to_le_bytes());
    }

    fn i32(&mut self, v: i32) {
        self.0.extend(v.to_le_bytes());
    }

    fn len(&mut self, n: usize) {
        self.u32(u32::try_from(n).expect("a song's counts fit in 32 bits"));
    }

    fn short_string(&mut self, s: &ShortString) {
        let bytes = s.bytes();
        self.0.push(bytes.len() as u8);
        self.0.extend(bytes);
        self.0
            .resize(self.0.len() + SHORT_STRING - 1 - bytes.len(), 0);
    }

    fn cell(&mut self, c: &Cell) {
        self.i32(c.note);
        self.i32(c.instrument);
        self.i32(c.volume);
        self.i32(c.effect_code);
        self.0.push(c.effect_param);
    }

    fn rows(&mut self, rows: &Rows) {
        for c in rows {
            self.cell(c);
        }
    }
}
