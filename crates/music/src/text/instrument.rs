//! An `instrument` block: its header, field lines and subpattern rows.

use super::fields::{self, length_max};
use super::lex::Token;
use super::spell::{self, DIRECTIONS, DUTIES, LEVELS, MODES, SWITCH};
use super::values::{args, at, byte, choice, int, number, row, string};
use crate::error::ParseError;
use crate::model::{BANK_SIZE, Bank, Instrument};

pub struct Block {
    pub index: usize,
    pub ins: Instrument,
    bank: Bank,
    seen: Vec<(String, u32)>,
    last_row: Option<usize>,
}

impl Block {
    /// `instrument BANK N "NAME"`.
    pub fn start(line: u32, tokens: &[Token]) -> Result<Block, ParseError> {
        let shape = "instrument square|wave|noise N \"NAME\" (N is 1-15)";
        let a = args(line, tokens, 3, shape)?;
        let bank = Bank::from_name(&a[0].text).ok_or_else(|| {
            at(
                line,
                &a[0],
                format!("`{}` is not an instrument bank", a[0].text),
                "the banks are square (channels 1 and 2), wave (3) and noise (4)",
            )
        })?;
        let slot = a[1]
            .text
            .parse::<usize>()
            .ok()
            .filter(|n| (1..=BANK_SIZE).contains(n))
            .ok_or_else(|| {
                at(
                    line,
                    &a[1],
                    format!("instrument `{}` is not in its bank", a[1].text),
                    "each bank's instruments are numbered 1-15",
                )
            })?;
        let index = bank as usize * BANK_SIZE + slot - 1;
        let mut ins = Instrument::default_for(index);
        ins.name = string(line, &a[2], "an instrument's name")?;
        Ok(Block {
            index,
            ins,
            bank,
            seen: Vec::new(),
            last_row: None,
        })
    }

    pub fn line(&mut self, line: u32, tokens: &[Token]) -> Result<(), ParseError> {
        let word = tokens[0].text.as_str();
        if word.bytes().all(|b| b.is_ascii_digit()) {
            return self.row(line, tokens);
        }
        if !fields::lines(self.bank).contains(&word) {
            return Err(at(
                line,
                &tokens[0],
                format!("`{word}` is not a {} instrument's field", self.bank.name()),
                format!(
                    "a {} instrument takes {} lines, then its subpattern rows",
                    self.bank.name(),
                    fields::lines(self.bank).join(", ")
                ),
            ));
        }
        if self.last_row.is_some() {
            return Err(at(
                line,
                &tokens[0],
                format!("`{word}` comes after the subpattern rows"),
                "put the field lines first, then the rows",
            ));
        }
        if let Some((_, first)) = self.seen.iter().find(|(w, _)| w == word) {
            return Err(at(
                line,
                &tokens[0],
                format!("`{word}` is given twice in this instrument (first at line {first})"),
                "keep one",
            ));
        }
        self.seen.push((word.to_string(), line));
        let ins = &mut self.ins;
        match word {
            "length" => length(line, tokens, self.bank, ins),
            "envelope" => {
                let a = args(line, tokens, 3, "envelope VOLUME up|down PACE (0-15, 0-7)")?;
                let v = number(line, &a[0], 0, 15, "the initial volume")?;
                ins.initial_volume = byte(line, &a[0], v, "the initial volume")?;
                let d = choice(line, &a[1], DIRECTIONS, "a direction")?;
                ins.volume_direction = int(line, &a[1], d, "the direction")?;
                let p = number(line, &a[2], 0, 7, "the envelope pace")?;
                ins.volume_pace = byte(line, &a[2], p, "the envelope pace")?;
                Ok(())
            }
            "sweep" => {
                let a = args(line, tokens, 3, "sweep TIME up|down SHIFT (0-7, 0-7)")?;
                let t = number(line, &a[0], 0, 7, "the sweep time")?;
                ins.sweep_time = int(line, &a[0], t, "the sweep time")?;
                let d = choice(line, &a[1], DIRECTIONS, "a direction")?;
                ins.sweep_direction = int(line, &a[1], d, "the direction")?;
                let s = number(line, &a[2], 0, 7, "the sweep shift")?;
                ins.sweep_shift = int(line, &a[2], s, "the sweep shift")?;
                Ok(())
            }
            "duty" => {
                let a = args(line, tokens, 1, "duty 12.5%|25%|50%|75%")?;
                let d = choice(line, &a[0], DUTIES, "a duty")?;
                ins.duty = byte(line, &a[0], d, "the duty")?;
                Ok(())
            }
            "level" => {
                let a = args(line, tokens, 1, "level 100%|50%|25%|0%")?;
                let l = choice(line, &a[0], LEVELS, "a wave level")?;
                ins.output_level = int(line, &a[0], l, "the level")?;
                Ok(())
            }
            "waveform" => {
                let a = args(line, tokens, 1, "waveform N (a wave, 0-15)")?;
                let w = number(line, &a[0], 0, 15, "the waveform")?;
                ins.waveform = int(line, &a[0], w, "the waveform")?;
                Ok(())
            }
            "mode" => {
                let a = args(line, tokens, 1, "mode 15-bit|7-bit")?;
                let m = choice(line, &a[0], MODES, "a noise mode")?;
                ins.counter_step = int(line, &a[0], m, "the noise mode")?;
                Ok(())
            }
            "subpattern" => {
                let a = args(line, tokens, 1, "subpattern on|off")?;
                let s = choice(line, &a[0], SWITCH, "on or off")?;
                ins.subpattern_enabled = byte(line, &a[0], s, "subpattern")?;
                Ok(())
            }
            _ => unused(line, tokens, self.bank, ins),
        }
    }

    /// `NN OFFSET INSTRUMENT JUMP EFFECT`.
    fn row(&mut self, line: u32, tokens: &[Token]) -> Result<(), ParseError> {
        let n = row(line, &tokens[0])?;
        if self.last_row.is_some_and(|last| n <= last) {
            return Err(at(
                line,
                &tokens[0],
                format!(
                    "row {:02} comes after row {:02}",
                    n,
                    self.last_row.unwrap_or(0)
                ),
                "rows go in ascending order, 00-63; a row left out is blank",
            ));
        }
        if tokens.len() != 5 {
            return Err(at(
                line,
                &tokens[0],
                format!(
                    "this subpattern row has {} fields after its number",
                    tokens.len() - 1
                ),
                "a subpattern row is the row, then a note offset (+00, -05, ...), an \
                 instrument (..), a jump (J05 or ...) and an effect (C08 or ...)",
            ));
        }
        self.last_row = Some(n);
        let cell = &mut self.ins.subpattern[n];
        let bad = |t: &Token, what: &str, remedy: &str| {
            at(line, t, format!("`{}` is not {what}", t.text), remedy)
        };
        cell.note = spell::read_offset(&tokens[1].text).ok_or_else(|| {
            bad(
                &tokens[1],
                "a note offset",
                "a subpattern note is an offset from the played note, -36 to +35 (+00, -05), \
                 or ... for none",
            )
        })?;
        cell.instrument = spell::read_instrument(&tokens[2].text).ok_or_else(|| {
            bad(
                &tokens[2],
                "an instrument",
                "write .. (subpatterns don't use it)",
            )
        })?;
        cell.volume = spell::read_jump(&tokens[3].text).ok_or_else(|| {
            bad(
                &tokens[3],
                "a jump",
                "a jump is J01-J64 (the row to go to next, counting from 1), or ... for none",
            )
        })?;
        let (code, param) = spell::read_effect(&tokens[4].text).ok_or_else(|| {
            bad(
                &tokens[4],
                "an effect",
                "an effect is three hex digits, the code then the parameter (C08), or ... \
                 for none; `gbcli help music` lists the codes",
            )
        })?;
        cell.effect_code = code;
        cell.effect_param = param;
        Ok(())
    }
}

fn length(line: u32, tokens: &[Token], bank: Bank, ins: &mut Instrument) -> Result<(), ParseError> {
    let max = length_max(bank);
    let shape = format!(
        "length off, length N or length off N (N steps of 1/256 s, 1-{max}), or length raw \
         ENABLED VALUE"
    );
    let a = &tokens[1..];
    let steps = |t: &Token| number(line, t, 1, max, "the length").map(|n| max - n);
    let (enabled, raw) = match a.iter().map(|t| t.text.as_str()).collect::<Vec<_>>()[..] {
        ["off"] => (0, 0),
        ["off", _] => (0, steps(&a[1])?),
        // Stored numbers outside the readable form, kept as they are.
        ["raw", _, _] => (
            number(line, &a[1], i64::MIN, i64::MAX, "the length switch")?,
            number(line, &a[2], i64::MIN, i64::MAX, "the length register")?,
        ),
        [_] => (1, steps(&a[0])?),
        _ => {
            let t = a.first().unwrap_or(&tokens[0]);
            return Err(at(line, t, "this length line has the wrong shape", shape));
        }
    };
    let t = a.last().unwrap_or(&tokens[0]);
    ins.length_enabled = byte(line, t, enabled, "the length switch")?;
    ins.length = int(line, t, raw, "the length")?;
    Ok(())
}

fn unused(line: u32, tokens: &[Token], bank: Bank, ins: &mut Instrument) -> Result<(), ParseError> {
    let names: Vec<&str> = fields::unused(bank).map(|r| r.name).collect();
    let remedy = format!(
        "a {} instrument's unused line sets stored numbers its fields don't show: {}",
        bank.name(),
        names.join(", ")
    );
    for t in &tokens[1..] {
        let Some((key, value)) = t.text.split_once('=') else {
            return Err(at(
                line,
                t,
                format!("`{}` is not KEY=VALUE", t.text),
                remedy,
            ));
        };
        let Some(raw) = fields::unused(bank).find(|r| r.name == key) else {
            return Err(at(
                line,
                t,
                format!("`{key}` is not an unused field here"),
                remedy,
            ));
        };
        let fits = value.parse::<i64>().ok().is_some_and(|v| (raw.set)(ins, v));
        if !fits {
            return Err(at(
                line,
                t,
                format!("`{value}` is not a stored value for {key}"),
                "write the stored number in decimal",
            ));
        }
    }
    Ok(())
}
