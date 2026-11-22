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
//! - instruments are written up to the highest each bank plays (but see
//!   the departures: a shared pattern counts for each channel kind);
//! - waves up to the highest any wave instrument, a wave-channel 9xx or a
//!   wave instrument's subpattern 9xx names;
//! - subpatterns are their first 32 rows, the last jumping to row 1 when it
//!   jumps nowhere.
//!
//! Departures from the export:
//!
//! - A pattern several channels play counts its instruments, waves and
//!   wave 9xx once per channel kind (square, wave, noise) that plays it.
//!   hUGETracker counts a pattern only for the first channel whose orders
//!   name it, so when a square and the wave (or noise) channel play equal
//!   cells, stored once, its export leaves out the wave's instruments and
//!   waves, and hUGEDriver plays that channel from past the tables
//!   (Ben's ruling, 2026-11-09). A song where no two channel kinds share a
//!   pattern exports as hUGETracker's does.
//! - Waves stop at wave 15. A 9xx above 15 makes hUGETracker's export
//!   write waves past its 16-entry array (Tempest's Kekri: waves 16-112,
//!   whatever memory follows); gbcli writes none of those (found by
//!   rev-bld, 2026-11-09; the whole-song register comparison is Same).
//! - Every data label is scoped to the descriptor (`song_theme.P0`, where
//!   hUGETracker writes `P0`), so the `.sym` has no name twice:
//!   hUGETracker's `order1` and `waves` are also hUGEDriver's own WRAM
//!   labels, and two songs would share every name. Each routine opens a
//!   global label of its own, `song_theme__hUGE_Routine_0` (hUGETracker's
//!   `__hUGE_Routine_0` behind the descriptor), so a routine's local labels
//!   are scoped to it as in the export.
//! - `routines:` is a table of the 16 routines' addresses, after the
//!   routines, as hUGETracker's preview builds it (hUGEDriver's `song.asm`):
//!   hUGEDriver's 6xx reads a pointer there. The RGBDS export writes the
//!   routines' code at `routines:`, so 6xx in an exported song jumps to an
//!   address made of code bytes. The table moves the waves 32 bytes later,
//!   which only a 6xx, or a read running past the routines' code, sees.
//! - `<descriptor>_TIMER` and `<descriptor>_TMA`, exported constants saying
//!   how the song wants `hUGE_dosound` called (hUGETracker's export leaves
//!   that to the player), so a player can set the timer from the song it
//!   links. They take no ROM space.

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

    // The routines, then `routines:` as the table of their addresses that
    // hUGEDriver's 6xx reads, as hUGETracker's preview lays them out
    // (hUGEDriver's song.asm). Its RGBDS export writes the code at
    // `routines:` instead, which 6xx jumps into as if it were the table.
    for (i, routine) in song.routines.iter().enumerate() {
        line(&mut out, &format!("{}:", routine_label(d, i)));
        out.extend_from_slice(routine);
        out.push(b'\n');
        line(&mut out, &format!("{d}__end_hUGE_Routine_{i}:"));
        line(&mut out, "ret");
        line(&mut out, "");
    }
    line(&mut out, &format!("{d}.routines:"));
    for half in (0..song.routines.len()).collect::<Vec<_>>().chunks(8) {
        let names: Vec<String> = half.iter().map(|i| routine_label(d, *i)).collect();
        line(&mut out, &format!("dw {}", names.join(", ")));
    }
    line(&mut out, "");

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

/// Why a song has no RGBDS source as text: what [`rgbds_text`] refuses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AsmRefusal {
    /// The descriptor is no RGBDS symbol the source's labels can hang
    /// from (see [`is_descriptor`]).
    Descriptor(String),
    /// Routine `routine` (0-15) holds bytes that aren't UTF-8, which a
    /// `.uge` can store and text can't carry.
    NotText { routine: usize },
}

impl AsmRefusal {
    pub fn message(&self) -> String {
        match self {
            AsmRefusal::Descriptor(d) => {
                format!("`{d}` can't be a song's descriptor: it is no RGBDS symbol of its own")
            }
            AsmRefusal::NotText { routine } => {
                format!("routine {routine} holds bytes that aren't UTF-8 text")
            }
        }
    }

    pub fn remedy(&self) -> &'static str {
        match self {
            AsmRefusal::Descriptor(_) => {
                "name it with letters, digits and _, not starting with a digit (song_theme)"
            }
            AsmRefusal::NotText { .. } => {
                "retype the routine in hUGETracker, or clear it if the song never calls it (6xx)"
            }
        }
    }
}

/// Whether `name` can be a descriptor: a global RGBDS symbol, letters,
/// digits and `_`, not starting with a digit. Not a local name (`a.b`),
/// since every other label is scoped under it.
pub fn is_descriptor(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// [`rgbds_asm`] as text, for an answer that carries it as a string: the
/// same bytes, refused when the descriptor is no symbol or a routine
/// isn't UTF-8.
pub fn rgbds_text(song: &Song, descriptor: &str) -> Result<String, AsmRefusal> {
    if !is_descriptor(descriptor) {
        return Err(AsmRefusal::Descriptor(descriptor.to_string()));
    }
    if let Some(routine) = song
        .routines
        .iter()
        .position(|r| std::str::from_utf8(r).is_err())
    {
        return Err(AsmRefusal::NotText { routine });
    }
    // Every byte the emitter writes but the routines' is ASCII.
    Ok(String::from_utf8(rgbds_asm(song, descriptor))
        .expect("the routines are UTF-8 and the rest is ASCII"))
}

/// Every global symbol [`rgbds_asm`] defines for `descriptor`.
pub fn global_symbols(descriptor: &str) -> Vec<String> {
    let d = descriptor;
    let mut names = vec![d.to_string(), format!("{d}_TIMER"), format!("{d}_TMA")];
    for i in 0..crate::model::ROUTINES {
        names.push(routine_label(d, i));
        names.push(format!("{d}__end_hUGE_Routine_{i}"));
    }
    names
}

/// Routine `i`'s label: global, so the routine's own local labels are
/// scoped to it, and unique to the song.
fn routine_label(descriptor: &str, i: usize) -> String {
    format!("{descriptor}__hUGE_Routine_{i}")
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
        // hUGETracker visits a pattern once, for the first channel that
        // plays it; counting it once per channel kind keeps a wave or noise
        // channel's instruments when a square plays the same cells first.
        let mut counted: HashSet<(i32, Bank)> = HashSet::new();
        for (channel, order) in song.orders.iter().enumerate() {
            let b = Bank::of_channel(channel);
            for key in order {
                used.patterns.insert(*key);
                if !counted.insert((*key, b)) {
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
