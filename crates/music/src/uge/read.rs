//! Reading `.uge` v0-v6.

use super::upgrade::{OldInstrument, OldSong, upgrade};
use super::{CELL_V1, CELL_V6, SHORT_STRING, Uge};
use crate::error::UgeError;
use crate::model::{
    BLANK_ROWS, Cell, INSTRUMENTS, Instrument, Pattern, ROUTINES, ROWS, Rows, ShortString, Song,
    WAVE_LEN, WAVES,
};

const REMEDY_DAMAGED: &str = "the file is cut short or isn't a .uge; open it in hUGETracker, save \
                              it again, and retry";

/// Reads a `.uge` file of any version hUGETracker writes (0-6), upgraded to
/// version 6 the way hUGETracker's song.pas upgrades it.
pub fn read_uge(bytes: &[u8]) -> Result<Uge, UgeError> {
    let mut r = Reader { bytes, at: 0 };
    let version = r.i32("the version")?;
    let version = u32::try_from(version)
        .ok()
        .filter(|v| *v <= super::FORMAT_VERSION)
        .ok_or_else(|| UgeError {
            offset: 0,
            message: format!("version {version} is not a .uge version this gbcli reads (0-6)"),
            remedy: "check that the file is a hUGETracker .uge; a newer hUGETracker's file \
                     needs a newer gbcli"
                .into(),
        })?;
    let name = r.short_string("the song name")?;
    let artist = r.short_string("the artist")?;
    let comment = r.short_string("the comment")?;
    if version == 6 {
        let instruments = (0..INSTRUMENTS)
            .map(|_| r.instrument_v6())
            .collect::<Result<Vec<_>, _>>()?;
        let waves = r.waves(WAVE_LEN)?;
        let ticks_per_row = r.i32("the ticks per row")?;
        let timer_enabled = r.u8("the timer flag")?;
        let timer_divider = r.i32("the timer divider")?;
        let patterns = r.patterns(version)?;
        let orders = r.orders()?;
        let routines = r.routines()?;
        r.end()?;
        let song = Song {
            name,
            artist,
            comment,
            instruments,
            waves,
            ticks_per_row,
            timer_enabled,
            timer_divider,
            patterns,
            orders,
            routines,
        };
        return Ok(Uge { version, song });
    }
    let count = if version < 3 { 15 } else { INSTRUMENTS };
    let instruments = (0..count)
        .map(|_| r.instrument_old(version >= 4))
        .collect::<Result<Vec<_>, _>>()?;
    // Versions 1 and 2 stored 33 bytes per wave (`TWaveV1`).
    let waves = r.waves(if version < 3 { WAVE_LEN + 1 } else { WAVE_LEN })?;
    let ticks_per_row = r.i32("the ticks per row")?;
    let patterns = r.patterns(version)?;
    let orders = r.orders()?;
    let routines = if version >= 2 {
        r.routines()?
    } else {
        Default::default()
    };
    r.end()?;
    let song = upgrade(OldSong {
        version,
        name,
        artist,
        comment,
        instruments,
        waves,
        ticks_per_row,
        patterns,
        orders,
        routines,
    });
    Ok(Uge { version, song })
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize, what: &str) -> Result<&[u8], UgeError> {
        match self.bytes.get(self.at..self.at + n) {
            Some(slice) => {
                self.at += n;
                Ok(slice)
            }
            None => Err(UgeError {
                offset: self.bytes.len(),
                message: format!(
                    "the file ends at byte {}, inside {what} (which starts at byte {})",
                    self.bytes.len(),
                    self.at
                ),
                remedy: REMEDY_DAMAGED.into(),
            }),
        }
    }

    fn u8(&mut self, what: &str) -> Result<u8, UgeError> {
        Ok(self.take(1, what)?[0])
    }

    fn i32(&mut self, what: &str) -> Result<i32, UgeError> {
        let b = self.take(4, what)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// A count of `size`-byte items that must fit in the rest of the file.
    fn count(&mut self, size: usize, what: &str) -> Result<usize, UgeError> {
        let at = self.at;
        let n = self.i32(what)?;
        let left = self.bytes.len() - self.at;
        match usize::try_from(n) {
            Ok(n) if n.saturating_mul(size) <= left => Ok(n),
            _ => Err(UgeError {
                offset: at,
                message: format!(
                    "the file says it holds {n} {what}, more than its remaining {left} bytes can"
                ),
                remedy: REMEDY_DAMAGED.into(),
            }),
        }
    }

    /// A `ShortString`: the length byte and that many bytes of its 255; the
    /// rest is residue (see the module note).
    fn short_string(&mut self, what: &str) -> Result<ShortString, UgeError> {
        let slot = self.take(SHORT_STRING, what)?;
        let len = usize::from(slot[0]);
        Ok(ShortString::new(slot[1..=len].to_vec()).expect("a length byte is at most 255"))
    }

    fn cell(&mut self, with_volume: bool) -> Result<Cell, UgeError> {
        let what = "a pattern cell";
        let note = self.i32(what)?;
        let instrument = self.i32(what)?;
        let volume = if with_volume { self.i32(what)? } else { 0 };
        Ok(Cell {
            note,
            instrument,
            volume,
            effect_code: self.i32(what)?,
            effect_param: self.u8(what)?,
        })
    }

    fn rows(&mut self, with_volume: bool) -> Result<Rows, UgeError> {
        let mut rows = BLANK_ROWS;
        for row in &mut rows {
            *row = self.cell(with_volume)?;
        }
        Ok(rows)
    }

    fn instrument_v6(&mut self) -> Result<Instrument, UgeError> {
        let what = "an instrument";
        Ok(Instrument {
            type_code: self.i32(what)?,
            name: self.short_string("an instrument name")?,
            length: self.i32(what)?,
            length_enabled: self.u8(what)?,
            initial_volume: self.u8(what)?,
            volume_direction: self.i32(what)?,
            volume_pace: self.u8(what)?,
            sweep_time: self.i32(what)?,
            sweep_direction: self.i32(what)?,
            sweep_shift: self.i32(what)?,
            duty: self.u8(what)?,
            output_level: self.i32(what)?,
            waveform: self.i32(what)?,
            counter_step: self.i32(what)?,
            subpattern_enabled: self.u8(what)?,
            subpattern: self.rows(true)?,
        })
    }

    /// `TInstrumentV1`, or `TInstrumentV2` (with the noise macro) from v4.
    fn instrument_old(&mut self, with_macro: bool) -> Result<OldInstrument, UgeError> {
        let what = "an instrument";
        let type_code = self.i32(what)?;
        let name = self.short_string("an instrument name")?;
        let length = self.i32(what)?;
        let length_enabled = self.u8(what)?;
        let initial_volume = self.u8(what)?;
        let volume_direction = self.i32(what)?;
        let volume_pace = self.u8(what)?;
        let sweep_time = self.i32(what)?;
        let sweep_direction = self.i32(what)?;
        let sweep_shift = self.i32(what)?;
        let duty = self.u8(what)?;
        let output_level = self.i32(what)?;
        let waveform = self.i32(what)?;
        let _shift_clock_freq = self.i32(what)?;
        let counter_step = self.i32(what)?;
        let _dividing_ratio = self.i32(what)?;
        let mut noise_macro = [0i8; 6];
        if with_macro {
            for m in &mut noise_macro {
                *m = self.u8("a noise macro")? as i8;
            }
        }
        Ok(OldInstrument {
            v6: Instrument {
                type_code,
                name,
                length,
                length_enabled,
                initial_volume,
                volume_direction,
                volume_pace,
                sweep_time,
                sweep_direction,
                sweep_shift,
                duty,
                output_level,
                waveform,
                counter_step,
                subpattern_enabled: 0,
                subpattern: BLANK_ROWS,
            },
            noise_macro,
        })
    }

    fn waves(&mut self, stride: usize) -> Result<[[u8; WAVE_LEN]; WAVES], UgeError> {
        let mut waves = [[0; WAVE_LEN]; WAVES];
        for wave in &mut waves {
            wave.copy_from_slice(&self.take(stride, "a wave")?[..WAVE_LEN]);
        }
        Ok(waves)
    }

    fn patterns(&mut self, version: u32) -> Result<Vec<Pattern>, UgeError> {
        let keyed = version >= 5;
        let cell = if version >= 6 { CELL_V6 } else { CELL_V1 };
        let size = usize::from(keyed) * 4 + ROWS * cell;
        let count = self.count(size, "patterns")?;
        let mut patterns: Vec<Pattern> = Vec::with_capacity(count);
        for n in 0..count {
            let at = self.at;
            let mut key = if keyed {
                self.i32("a pattern key")?
            } else {
                n as i32
            };
            let taken = |key: i32, patterns: &[Pattern]| patterns.iter().any(|p| p.key == key);
            // GB Studio 3.0.2 and earlier wrote v5 files without unique keys;
            // GB Studio's loader numbers a repeat by its position.
            if version == 5 && taken(key, &patterns) {
                key = n as i32;
            }
            if taken(key, &patterns) {
                return Err(UgeError {
                    offset: at,
                    message: format!("pattern key {key} appears twice"),
                    remedy: "open the song in hUGETracker, save it again, and retry".into(),
                });
            }
            let rows = self.rows(version >= 6)?;
            patterns.push(Pattern { key, rows });
        }
        Ok(patterns)
    }

    /// Each channel's orders, without the unread last entry.
    fn orders(&mut self) -> Result<[Vec<i32>; 4], UgeError> {
        let mut orders: [Vec<i32>; 4] = Default::default();
        for order in &mut orders {
            let n = self.count(4, "order entries")?;
            let mut keys = (0..n)
                .map(|_| self.i32("an order entry"))
                .collect::<Result<Vec<_>, _>>()?;
            keys.pop();
            *order = keys;
        }
        Ok(orders)
    }

    fn routines(&mut self) -> Result<[Vec<u8>; ROUTINES], UgeError> {
        let mut routines: [Vec<u8>; ROUTINES] = Default::default();
        for routine in &mut routines {
            let n = self.count(1, "routine bytes")?;
            *routine = self.take(n, "a routine")?.to_vec();
        }
        Ok(routines)
    }

    fn end(&self) -> Result<(), UgeError> {
        let left = self.bytes.len() - self.at;
        if left == 0 {
            return Ok(());
        }
        Err(UgeError {
            offset: self.at,
            message: format!("{left} bytes follow the song, where the file should end"),
            remedy: REMEDY_DAMAGED.into(),
        })
    }
}
