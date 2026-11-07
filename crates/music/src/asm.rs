//! A song as RGBDS assembly for hUGEDriver: a port of hUGETracker's
//! `RenderSongToRGBDSAsm` (codegen.pas at 3fedced3, lines 371–639) and what
//! it calls — `OptimizeSong` (song.pas), `FindUsedStuff`, `InstrumentToBytes`
//! (instruments.pas) and the cell renderers — so the bytes hUGEDriver reads
//! are hUGETracker's, with no Free Pascal needed.
//!
//! The port keeps hUGETracker's layout and its quirks, because a driver
//! that reads past a table (an instrument or wave number above the highest
//! the export counted) reads whatever the layout puts there:
//!
//! - an order names the first pattern, in storage order, whose 64 cells
//!   (the unused volume included) equal its own;
//! - instruments are written up to the highest each bank plays, counted
//!   per pattern for the first channel whose orders name it;
//! - waves up to the highest any wave instrument, a wave-channel 9xx or a
//!   wave instrument's subpattern 9xx names;
//! - subpatterns are their first 32 rows, the last jumping to row 1 when it
//!   jumps nowhere.
//!
//! Every label but the descriptor is scoped to it (`song_theme.P0`, where
//! hUGETracker writes `P0`), so the `.sym` has no name twice: hUGETracker's
//! `order1` and `waves` are also hUGEDriver's own WRAM labels, and two songs
//! would share every name. Names never reach the bytes.
//!
//! One addition: `<descriptor>_TIMER` and `<descriptor>_TMA`, exported
//! constants saying how the song wants `hUGE_dosound` called (hUGETracker's
//! export leaves that to the player), so a player can set the timer from
//! the song it links. They take no ROM space.

use std::collections::HashSet;
use std::fmt::Write as _;

use crate::model::{Bank, Cell, Instrument, NO_NOTE, PLAYED_SUBPATTERN_ROWS, Pattern, Song};
use crate::notes::note_name;

/// The song as an RGBDS source file declaring `descriptor::`, the label to
/// pass `hUGE_init` in `hl`. It includes `hUGE.inc` (hUGEDriver's macros and
/// note names). Bytes, not text, because routines are a song's own code,
/// stored as bytes.
pub fn rgbds_asm(song: &Song, descriptor: &str) -> Vec<u8> {
    let mut song = song.clone();
    song.normalize();
    optimize(&mut song);
    let used = Used::of(&song);

    let mut out = Vec::new();
    let line = |out: &mut Vec<u8>, s: &str| {
        out.extend_from_slice(s.as_bytes());
        out.push(b'\n');
    };
    line(&mut out, "include \"hUGE.inc\"");
    line(&mut out, "");
    line(
        &mut out,
        &format!("SECTION \"{descriptor} Song Data\", ROMX"),
    );
    line(&mut out, "");
    line(&mut out, &format!("{descriptor}::"));
    line(&mut out, &format!("db {}", song.ticks_per_row));
    let d = descriptor;
    for l in [
        format!("dw {d}.order_cnt"),
        format!("dw {d}.order1, {d}.order2, {d}.order3, {d}.order4"),
        format!("dw {d}.duty_instruments, {d}.wave_instruments, {d}.noise_instruments"),
        format!("dw {d}.routines"),
        format!("dw {d}.waves"),
        String::new(),
    ] {
        line(&mut out, &l);
    }

    // hUGETracker adds each block's text, itself ending in a newline, as
    // one more line, so a blank line follows each.
    let orders = song.orders[0].len();
    let mut block = format!("{d}.order_cnt: db {}\n", orders * 2);
    for (c, order) in song.orders.iter().enumerate() {
        let names: Vec<String> = order.iter().map(|k| format!("{d}.P{k}")).collect();
        let _ = writeln!(block, "{d}.order{}: dw {}", c + 1, names.join(","));
    }
    line(&mut out, &block);

    for p in song
        .patterns
        .iter()
        .filter(|p| used.patterns.contains(&p.key))
    {
        line(&mut out, &pattern(d, p));
    }
    for (i, ins) in song.instruments.iter().enumerate() {
        let n = slot_number(i);
        if ins.subpattern_enabled != 0 && used.instrument(n, bank(ins, i)) {
            line(
                &mut out,
                &subpattern(&format!("{d}.{}SP{n}", prefix(ins, i)), ins),
            );
        }
    }

    for (label, b) in [
        ("duty_instruments", Bank::Square),
        ("wave_instruments", Bank::Wave),
        ("noise_instruments", Bank::Noise),
    ] {
        line(&mut out, &format!("{d}.{label}:"));
        line(&mut out, &instruments(d, &song, b, used.highest(b)));
        line(&mut out, "");
    }

    line(&mut out, &format!("{d}.routines:"));
    for (i, routine) in song.routines.iter().enumerate() {
        line(&mut out, &format!("{d}.__hUGE_Routine_{i}:"));
        out.extend_from_slice(routine);
        out.push(b'\n');
        line(&mut out, &format!("{d}.__end_hUGE_Routine_{i}:"));
        line(&mut out, "ret");
        line(&mut out, "");
    }

    line(&mut out, &format!("{d}.waves:"));
    let mut waves = String::new();
    for (i, wave) in song.waves.iter().enumerate() {
        if i as i64 > used.highest_wave {
            break;
        }
        let bytes: Vec<String> = wave
            .chunks(2)
            .map(|p| ((u32::from(p[0]) << 4) | u32::from(p[1])).to_string())
            .collect();
        let _ = writeln!(waves, "{d}.wave{i}: db {}", bytes.join(","));
    }
    line(&mut out, &waves);

    let timer = u8::from(song.timer_enabled != 0);
    line(
        &mut out,
        "; gbcli: how the song wants hUGE_dosound called (not in hUGETracker's export)",
    );
    line(&mut out, &format!("DEF {descriptor}_TIMER EQU {timer}"));
    line(
        &mut out,
        &format!("DEF {descriptor}_TMA EQU {}", song.timer_divider & 0xFF),
    );
    line(
        &mut out,
        &format!("EXPORT {descriptor}_TIMER, {descriptor}_TMA"),
    );
    out
}

/// `song_` and `stem` with every character an RGBDS symbol can't hold as
/// `_`: the descriptor `build` gives a song file.
pub fn descriptor_for(stem: &str) -> String {
    let body: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("song_{body}")
}

/// Points every order at the first stored pattern equal to the one it
/// names (song.pas `OptimizeSong`).
fn optimize(song: &mut Song) {
    let first: Vec<i32> = song
        .patterns
        .iter()
        .map(|p| {
            song.patterns
                .iter()
                .find(|q| q.rows == p.rows)
                .map_or(p.key, |q| q.key)
        })
        .collect();
    for order in &mut song.orders {
        for key in order.iter_mut() {
            if let Some(i) = song.patterns.iter().position(|p| p.key == *key) {
                *key = first[i];
            }
        }
    }
}

/// What the export writes (codegen.pas `FindUsedStuff`).
struct Used {
    /// The highest instrument each bank plays (square, wave, noise), or -1.
    highest: [i64; 3],
    highest_wave: i64,
    patterns: HashSet<i32>,
}

impl Used {
    fn of(song: &Song) -> Used {
        let mut used = Used {
            highest: [-1; 3],
            highest_wave: -1,
            patterns: HashSet::new(),
        };
        let wave_bank = Bank::Wave as usize * crate::model::BANK_SIZE;
        for ins in &song.instruments[wave_bank..wave_bank + crate::model::BANK_SIZE] {
            if ins.subpattern_enabled == 0 {
                continue;
            }
            for c in &ins.subpattern {
                if c.effect_code == 9 {
                    used.highest_wave = used.highest_wave.max(i64::from(c.effect_param));
                }
            }
        }
        for (channel, order) in song.orders.iter().enumerate() {
            let b = Bank::of_channel(channel);
            for key in order {
                if !used.patterns.insert(*key) {
                    continue;
                }
                let Some(p) = song.pattern(*key) else {
                    continue;
                };
                for c in &p.rows {
                    if c.effect_code == 9 && channel == 2 {
                        used.highest_wave = used.highest_wave.max(i64::from(c.effect_param));
                    }
                    if c.instrument == 0 {
                        continue;
                    }
                    let n = if (0..=15).contains(&c.instrument) {
                        c.instrument
                    } else {
                        0
                    };
                    let h = &mut used.highest[b as usize];
                    *h = (*h).max(i64::from(n));
                    if channel == 2 && (1..=15).contains(&c.instrument) {
                        let ins = &song.instruments[wave_bank + c.instrument as usize - 1];
                        used.highest_wave = used.highest_wave.max(i64::from(ins.waveform));
                    }
                }
            }
        }
        used
    }

    fn highest(&self, b: Bank) -> i64 {
        self.highest[b as usize]
    }

    /// codegen.pas `InstrumentIsUsed`: at or below the bank's highest.
    fn instrument(&self, n: i64, b: Bank) -> bool {
        n <= self.highest(b)
    }
}

/// 1-15 within its bank (utils.pas `ModInst`).
fn slot_number(slot: usize) -> i64 {
    (slot % crate::model::BANK_SIZE) as i64 + 1
}

/// The bank an instrument's stored type names, or its slot's when the type
/// is out of range.
fn bank(ins: &Instrument, slot: usize) -> Bank {
    match ins.type_code {
        0 => Bank::Square,
        1 => Bank::Wave,
        2 => Bank::Noise,
        _ => Bank::of_slot(slot),
    }
}

/// hUGETracker's label prefix: its instrument type's Pascal name.
fn prefix(ins: &Instrument, slot: usize) -> &'static str {
    match bank(ins, slot) {
        Bank::Square => "itSquare",
        Bank::Wave => "itWave",
        Bank::Noise => "itNoise",
    }
}

fn effect(c: &Cell) -> String {
    format!(
        "${:03X}",
        ((c.effect_code << 8) | i32::from(c.effect_param)) & 0xFFF
    )
}

fn pattern(scope: &str, p: &Pattern) -> String {
    let mut s = format!("{scope}.P{}:\n", p.key);
    for c in &p.rows {
        let note = note_name(c.note)
            .filter(|_| c.note != NO_NOTE)
            .map_or("___".to_string(), |n| n.replace('-', "_"));
        let instrument = if (0..=15).contains(&c.instrument) {
            c.instrument
        } else {
            0
        };
        let _ = writeln!(s, " dn {note},{instrument},{}", effect(c));
    }
    s
}

fn subpattern(label: &str, ins: &Instrument) -> String {
    let mut s = format!("{label}:\n");
    for (i, c) in ins.subpattern[..PLAYED_SUBPATTERN_ROWS].iter().enumerate() {
        let note = if c.note == NO_NOTE {
            "___".to_string()
        } else {
            c.note.to_string()
        };
        let jump = if i == PLAYED_SUBPATTERN_ROWS - 1 && c.volume == 0 {
            1
        } else {
            c.volume.clamp(0, 32)
        };
        let _ = writeln!(s, " dn {note},{jump},{}", effect(c));
    }
    s
}

/// A bank's instruments 1 to `limit` (codegen.pas `RenderInstruments`).
fn instruments(scope: &str, song: &Song, b: Bank, limit: i64) -> String {
    let start = b as usize * crate::model::BANK_SIZE;
    let mut s = String::new();
    for n in 1..=limit.max(0) {
        let slot = start + n as usize - 1;
        let ins = &song.instruments[slot];
        let bytes = instrument_bytes(ins, bank(ins, slot));
        let table = if ins.subpattern_enabled != 0 {
            format!("dw {scope}.{}SP{n}", prefix(ins, slot))
        } else {
            "dw 0".to_string()
        };
        let _ = writeln!(s, "{scope}.{}inst{n}:", prefix(ins, slot));
        if bank(ins, slot) == Bank::Noise {
            let mut mask = bytes[0];
            if ins.length_enabled != 0 {
                mask |= 0x40;
            }
            if ins.counter_step == 1 {
                mask |= 0x80;
            }
            let _ = writeln!(s, "db {}\n{table}\ndb {mask}\nds 2\n", bytes[1]);
        } else {
            let _ = writeln!(
                s,
                "db {}\ndb {}\ndb {}\n{table}\ndb {}\n",
                bytes[0], bytes[1], bytes[2], bytes[3]
            );
        }
    }
    s
}

/// instruments.pas `InstrumentToBytes`: the registers a note-on writes,
/// with frequency 0 and the trigger bit set.
fn instrument_bytes(ins: &Instrument, b: Bank) -> [u8; 4] {
    let bit = |v: bool| u8::from(v);
    let envelope = ((ins.initial_volume & 0xF) << 4)
        | (bit(ins.volume_direction == 0) << 3)
        | (ins.volume_pace & 7);
    let high = 0x80 | ((ins.length_enabled & 1) << 6);
    match b {
        Bank::Square => [
            (((ins.sweep_time & 7) as u8) << 4)
                | (bit(ins.sweep_direction == 1) << 3)
                | ((ins.sweep_shift & 7) as u8),
            ((ins.duty & 3) << 6) | ((ins.length & 0x3F) as u8),
            envelope,
            high,
        ],
        Bank::Wave => [
            (ins.length & 0xFF) as u8,
            ((ins.output_level & 3) as u8) << 5,
            (ins.waveform & 0xFF) as u8,
            high,
        ],
        Bank::Noise => [(ins.length & 0xFF) as u8, envelope, 0, high],
    }
}
