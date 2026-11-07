//! The canonical text of a song: one spelling for everything, so text →
//! song → text is a fixpoint.

use super::FORMAT_LINE;
use super::fields::{self, length_max};
use super::spell::{self, DIRECTIONS, DUTIES, LEVELS, MODES, SWITCH, choice, number};
use crate::model::{BLANK_ROWS, Bank, Instrument, ROWS, Rows, Song, WAVE_LEN};

/// `song`'s canonical text, of the song [`Song::normalize`]d: the text has
/// no spelling for uneven orders or an order naming a missing pattern.
pub fn to_text(song: &Song) -> String {
    let mut normal = song.clone();
    normal.normalize();
    let song = &normal;
    let mut out = String::new();
    let o = &mut out;
    line(o, FORMAT_LINE);
    line(
        o,
        "# A hUGETracker song. `gbcli help music` explains every line.",
    );
    line(o, &format!("name {}", spell::quote(song.name.bytes())));
    line(o, &format!("artist {}", spell::quote(song.artist.bytes())));
    line(
        o,
        &format!("comment {}", spell::quote(song.comment.bytes())),
    );
    line(
        o,
        &format!("ticks_per_row {}", number(song.ticks_per_row, 1, 255)),
    );
    line(o, &timer(song));
    line(o, "");
    for (i, wave) in song.waves.iter().enumerate() {
        line(o, &format!("wave {i} {}", wave_samples(wave)));
    }
    for (index, ins) in song.instruments.iter().enumerate() {
        if *ins != Instrument::default_for(index) {
            line(o, "");
            instrument(o, index, ins);
        }
    }
    for (i, routine) in song.routines.iter().enumerate() {
        if !routine.is_empty() {
            line(o, "");
            self::routine(o, i, routine);
        }
    }
    orders(o, song);
    patterns(o, song);
    out
}

fn line(out: &mut String, text: &str) {
    out.push_str(text);
    out.push('\n');
}

fn timer(song: &Song) -> String {
    let divider = number(song.timer_divider, 0, 255);
    match (song.timer_enabled, song.timer_divider) {
        (0, 0) => "timer off".into(),
        (0, _) => format!("timer off {divider}"),
        (1, _) => format!("timer on {divider}"),
        (e, _) => format!("timer {} {divider}", spell::bang(e)),
    }
}

/// 32 hex nibbles, or 32 hex bytes when a sample doesn't fit a nibble.
fn wave_samples(wave: &[u8; WAVE_LEN]) -> String {
    if wave.iter().all(|s| *s < 16) {
        wave.iter().map(|s| format!("{s:X}")).collect()
    } else {
        let bytes: Vec<String> = wave.iter().map(|s| format!("{s:02X}")).collect();
        bytes.join(" ")
    }
}

fn instrument(out: &mut String, index: usize, ins: &Instrument) {
    let bank = Bank::of_slot(index);
    let slot = index % crate::model::BANK_SIZE + 1;
    line(
        out,
        &format!(
            "instrument {} {slot} {}",
            bank.name(),
            spell::quote(ins.name.bytes())
        ),
    );
    let field = |out: &mut String, text: String| line(out, &format!("  {text}"));
    field(out, length(bank, ins));
    let envelope = format!(
        "envelope {} {} {}",
        number(ins.initial_volume, 0, 15),
        choice(ins.volume_direction, DIRECTIONS),
        number(ins.volume_pace, 0, 7)
    );
    match bank {
        Bank::Square => {
            field(out, envelope);
            field(
                out,
                format!(
                    "sweep {} {} {}",
                    number(ins.sweep_time, 0, 7),
                    choice(ins.sweep_direction, DIRECTIONS),
                    number(ins.sweep_shift, 0, 7)
                ),
            );
            field(out, format!("duty {}", choice(ins.duty, DUTIES)));
        }
        Bank::Wave => {
            field(out, format!("level {}", choice(ins.output_level, LEVELS)));
            field(out, format!("waveform {}", number(ins.waveform, 0, 15)));
        }
        Bank::Noise => {
            field(out, envelope);
            field(out, format!("mode {}", choice(ins.counter_step, MODES)));
        }
    }
    field(
        out,
        format!("subpattern {}", choice(ins.subpattern_enabled, SWITCH)),
    );
    let default = Instrument::default_for(index);
    let unused: Vec<String> = fields::unused(bank)
        .filter(|r| (r.get)(ins) != (r.get)(&default))
        .map(|r| format!("{}={}", r.name, (r.get)(ins)))
        .collect();
    if !unused.is_empty() {
        field(out, format!("unused {}", unused.join(" ")));
    }
    if ins.subpattern.iter().any(|c| !c.is_blank()) {
        for (row, cell) in ins.subpattern.iter().enumerate() {
            field(out, format!("{row:02} {}", spell::sub_cell(cell).join(" ")));
        }
    }
}

fn length(bank: Bank, ins: &Instrument) -> String {
    let max = length_max(bank);
    let raw = i64::from(ins.length);
    match ins.length_enabled {
        0 if raw == 0 => "length off".into(),
        1 if (0..max).contains(&raw) => format!("length {}", max - raw),
        0 if (1..max).contains(&raw) => format!("length off {}", max - raw),
        e => format!("length raw {e} {raw}"),
    }
}

/// A routine as `|` lines when its line endings are all LF or all CRLF,
/// else as one escaped string.
fn routine(out: &mut String, i: usize, routine: &[u8]) {
    let text = std::str::from_utf8(routine).ok();
    let crlf = text.is_some_and(|t| t.contains("\r\n"));
    let clean = text.filter(|t| {
        if crlf {
            t.replace("\r\n", "\n").find('\r').is_none()
                && t.matches('\n').count() == t.matches("\r\n").count()
        } else {
            !t.contains('\r')
        }
    });
    let Some(text) = clean else {
        line(out, &format!("routine {i} {}", spell::quote(routine)));
        return;
    };
    line(
        out,
        &format!("routine {i}{}", if crlf { " crlf" } else { "" }),
    );
    let newline = if crlf { "\r\n" } else { "\n" };
    for l in text.split(newline) {
        if l.is_empty() {
            line(out, "|");
        } else {
            line(out, &format!("| {l}"));
        }
    }
    line(out, "end");
}

fn name(key: Option<&i32>) -> String {
    key.map_or_else(|| "-".into(), |k| format!("P{k}"))
}

fn orders(out: &mut String, song: &Song) {
    let len = song.orders.iter().map(Vec::len).max().unwrap_or(0);
    if len == 0 {
        return;
    }
    line(out, "");
    line(out, "orders");
    let width = (len - 1).to_string().len().max(2);
    for i in 0..len {
        let names: Vec<String> = song.orders.iter().map(|o| name(Some(&o[i]))).collect();
        line(out, &format!("{i:0width$} {}", names.join(" ")));
    }
    let keys: Vec<i32> = song.patterns.iter().map(|p| p.key).collect();
    if !keys.is_sorted() {
        let names: Vec<String> = keys.iter().map(|k| name(Some(k))).collect();
        line(out, &format!("storage {}", names.join(" ")));
    }
}

/// One block per distinct row of the orders, then the patterns no order
/// plays, four to a block.
fn patterns(out: &mut String, song: &Song) {
    let len = song.orders.iter().map(Vec::len).max().unwrap_or(0);
    let mut blocks: Vec<[Option<i32>; 4]> = Vec::new();
    for i in 0..len {
        let block = [0, 1, 2, 3].map(|c| song.orders[c].get(i).copied());
        if !blocks.contains(&block) {
            blocks.push(block);
        }
    }
    let shown =
        |key: i32, blocks: &[[Option<i32>; 4]]| blocks.iter().any(|b| b.contains(&Some(key)));
    let unplayed: Vec<i32> = song
        .patterns
        .iter()
        .map(|p| p.key)
        .filter(|k| !shown(*k, &blocks))
        .collect();
    for chunk in unplayed.chunks(4) {
        blocks.push([0, 1, 2, 3].map(|c| chunk.get(c).copied()));
    }
    if blocks.is_empty() {
        return;
    }
    line(out, "");
    line(
        out,
        "# Pattern rows: the row, then for each channel (square 1, square 2, wave, noise) a \
         note, instrument, volume (unused) and effect.",
    );
    for block in blocks {
        line(out, "");
        let names: Vec<String> = block.iter().map(|k| name(k.as_ref())).collect();
        line(out, &format!("pattern {}", names.join(" ")));
        let rows: Vec<&Rows> = block
            .iter()
            .map(|k| {
                k.and_then(|k| song.pattern(k))
                    .map_or(&BLANK_ROWS, |p| &p.rows)
            })
            .collect();
        for row in 0..ROWS {
            let cells: Vec<String> = rows
                .iter()
                .map(|r| spell::cell(&r[row]).join(" "))
                .collect();
            line(out, &format!("{row:02} {}", cells.join(" | ")));
        }
    }
}
