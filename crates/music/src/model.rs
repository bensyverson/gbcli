//! A hUGETracker song as `.uge` version 6 stores it: every field the file
//! holds, at the width the file holds it, so writing a song read from a v6
//! file gives back its bytes (except the parts named in [`crate::uge`]).

/// Rows in a pattern and in an instrument's subpattern.
pub const ROWS: usize = 64;
/// Instruments in each bank (square, wave, noise).
pub const BANK_SIZE: usize = 15;
/// Instruments in a song: three banks of [`BANK_SIZE`].
pub const INSTRUMENTS: usize = 3 * BANK_SIZE;
/// Waves in a song, each 32 four-bit samples.
pub const WAVES: usize = 16;
/// Samples in a wave.
pub const WAVE_LEN: usize = 32;
/// Routines (custom code called by effect 6) in a song.
pub const ROUTINES: usize = 16;
/// The note value of an empty cell.
pub const NO_NOTE: i32 = 90;
/// The highest note, B-8; the lowest, 0, is C-3.
pub const HIGHEST_NOTE: i32 = 71;
/// Subpattern rows hUGEDriver plays: hUGETracker exports rows 00-31 of the
/// 64 it stores (utils.pas `SubpatternToBytes`).
pub const PLAYED_SUBPATTERN_ROWS: usize = 32;
/// The highest subpattern jump hUGEDriver encodes: the jump is five bits
/// (utils.pas `DN`, hUGEDriver.asm `do_table`), so J32 and up never jump.
pub const HIGHEST_JUMP: i32 = 31;
/// The note value a subpattern treats as "no transposition" (hUGETracker's
/// `MIDDLE_NOTE`): a subpattern note is shown as its offset from this.
pub const MIDDLE_NOTE: i32 = 36;

/// A Pascal `ShortString`'s text: at most 255 bytes, encoding unchecked.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShortString(Vec<u8>);

impl ShortString {
    pub const MAX: usize = 255;

    /// `None` when `bytes` is longer than [`ShortString::MAX`].
    pub fn new(bytes: Vec<u8>) -> Option<ShortString> {
        (bytes.len() <= Self::MAX).then_some(ShortString(bytes))
    }

    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}

/// One pattern or subpattern cell, as stored: four 32-bit integers and a
/// parameter byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    /// 0 (C-3) to 71 (B-8), or [`NO_NOTE`]. In a subpattern, an offset from
    /// [`MIDDLE_NOTE`].
    pub note: i32,
    /// 1-15 within the channel's bank, or 0 for none. Unused in a subpattern.
    pub instrument: i32,
    /// In a pattern, unused (hUGETracker shows `...`). In a subpattern, the
    /// row to jump to after this one, 1-based, or 0 for the next row.
    pub volume: i32,
    /// 0-F: hUGETracker's effect codes.
    pub effect_code: i32,
    pub effect_param: u8,
}

impl Cell {
    pub const BLANK: Cell = Cell {
        note: NO_NOTE,
        instrument: 0,
        volume: 0,
        effect_code: 0,
        effect_param: 0,
    };

    pub fn is_blank(&self) -> bool {
        *self == Cell::BLANK
    }
}

/// 64 cells: one channel's pattern, or an instrument's subpattern.
pub type Rows = [Cell; ROWS];

/// A blank pattern.
pub const BLANK_ROWS: Rows = [Cell::BLANK; ROWS];

/// One channel's pattern, under the key the orders reference it by.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern {
    pub key: i32,
    pub rows: Rows,
}

/// An instrument bank, which is also the channel kind that plays it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Bank {
    Square,
    Wave,
    Noise,
}

impl Bank {
    pub const ALL: [Bank; 3] = [Bank::Square, Bank::Wave, Bank::Noise];

    /// The bank of instrument slot `index` (0-44).
    pub fn of_slot(index: usize) -> Bank {
        Bank::ALL[index / BANK_SIZE]
    }

    /// The `Type_` value hUGETracker stores for it.
    pub fn type_code(self) -> i32 {
        self as i32
    }

    pub fn name(self) -> &'static str {
        match self {
            Bank::Square => "square",
            Bank::Wave => "wave",
            Bank::Noise => "noise",
        }
    }

    pub fn from_name(name: &str) -> Option<Bank> {
        Bank::ALL.into_iter().find(|b| b.name() == name)
    }

    /// The bank channel `channel` (0-3) plays from.
    pub fn of_channel(channel: usize) -> Bank {
        match channel {
            0 | 1 => Bank::Square,
            2 => Bank::Wave,
            _ => Bank::Noise,
        }
    }
}

/// One instrument, every field the v6 record holds. Which fields a bank
/// uses: square — length, envelope, sweep, duty; wave — length, output
/// level, waveform; noise — length, envelope, counter step. All three have
/// a subpattern. The rest are stored and not used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instrument {
    /// `Type_`: 0 square, 1 wave, 2 noise; matches the bank in files
    /// hUGETracker writes.
    pub type_code: i32,
    pub name: ShortString,
    /// The length register's value: the note lasts 64 − length (256 −
    /// length on the wave channel) 1/256 s steps, when `length_enabled`.
    pub length: i32,
    pub length_enabled: u8,
    pub initial_volume: u8,
    /// 0 up, 1 down.
    pub volume_direction: i32,
    /// Envelope pace: 0 holds the volume, 1-7 steps it every n/64 s.
    pub volume_pace: u8,
    pub sweep_time: i32,
    /// 0 up, 1 down.
    pub sweep_direction: i32,
    pub sweep_shift: i32,
    /// 0-3: 12.5%, 25%, 50%, 75%.
    pub duty: u8,
    /// The wave channel's level: 0 mute, 1 100%, 2 50%, 3 25%.
    pub output_level: i32,
    /// The wave (0-15) a wave instrument plays.
    pub waveform: i32,
    /// The noise LFSR width: 0 fifteen bits, 1 seven bits.
    pub counter_step: i32,
    pub subpattern_enabled: u8,
    pub subpattern: Rows,
}

impl Instrument {
    /// The instrument hUGETracker gives slot `index` (0-44) of a new song
    /// (song.pas `InitializeSong`): what an instrument the text leaves out
    /// is.
    pub fn default_for(index: usize) -> Instrument {
        let blank = Instrument {
            type_code: Bank::of_slot(index).type_code(),
            name: ShortString::default(),
            length: 0,
            length_enabled: 0,
            initial_volume: 0,
            volume_direction: 0,
            volume_pace: 0,
            sweep_time: 0,
            sweep_direction: 0,
            sweep_shift: 0,
            duty: 0,
            output_level: 0,
            waveform: 0,
            counter_step: 0,
            subpattern_enabled: 0,
            subpattern: BLANK_ROWS,
        };
        match Bank::of_slot(index) {
            Bank::Square => Instrument {
                initial_volume: 15,
                volume_direction: 1,
                sweep_direction: 1,
                duty: 2,
                output_level: 1,
                ..blank
            },
            Bank::Wave => Instrument {
                output_level: 1,
                waveform: (index % BANK_SIZE) as i32,
                ..blank
            },
            Bank::Noise => Instrument {
                initial_volume: 15,
                volume_direction: 1,
                ..blank
            },
        }
    }
}

/// A song: the whole of a `.uge` v6 file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Song {
    pub name: ShortString,
    pub artist: ShortString,
    pub comment: ShortString,
    /// 45 slots: square 1-15, wave 1-15, noise 1-15.
    pub instruments: Vec<Instrument>,
    /// Each sample is a nibble in files hUGETracker writes.
    pub waves: [[u8; WAVE_LEN]; WAVES],
    pub ticks_per_row: i32,
    pub timer_enabled: u8,
    /// The timer modulo (TMA): with the timer enabled the driver ticks at
    /// 4096 / (256 − divider) Hz instead of once per frame.
    pub timer_divider: i32,
    /// In the order the file stores them.
    pub patterns: Vec<Pattern>,
    /// Per channel, the pattern keys in play order. The file stores one
    /// more entry than this; see [`crate::uge`].
    pub orders: [Vec<i32>; 4],
    pub routines: [Vec<u8>; ROUTINES],
}

/// hUGETracker's default waves 0-10 (hugedatatypes.pas `DefaultWaves`),
/// loaded into every new song.
pub const DEFAULT_WAVES: [[u8; WAVE_LEN]; 11] = [
    nibbles("0000ffffffffffffffffffffffffffff"),
    nibbles("00000000ffffffffffffffffffffffff"),
    nibbles("0000000000000000ffffffffffffffff"),
    nibbles("000000000000000000000000ffffffff"),
    nibbles("000112233445566778899aabbccddeef"),
    nibbles("fedcba9876543210123456789abcdeff"),
    nibbles("7acddb75211368bddc974101479cddb8"),
    nibbles("0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f"),
    nibbles("fefcfaf8f6f4f2f0f2f4f6f8fafcfeff"),
    nibbles("feddccbbaa9988778abdf124578abdee"),
    nibbles("841161ed57475aadcea31779dd200347"),
];

/// 32 lowercase hex digits as nibbles.
const fn nibbles(hex: &str) -> [u8; WAVE_LEN] {
    let bytes = hex.as_bytes();
    assert!(bytes.len() == WAVE_LEN);
    let mut out = [0u8; WAVE_LEN];
    let mut i = 0;
    while i < WAVE_LEN {
        let c = bytes[i];
        out[i] = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            _ => panic!("not a hex digit"),
        };
        i += 1;
    }
    out
}

impl Default for Song {
    /// An empty song as the text form starts one: hUGETracker's defaults
    /// (seven ticks per row, the vblank timer, default instruments, waves
    /// 0-10 from [`DEFAULT_WAVES`] and 11-15 silent), no patterns, no orders.
    fn default() -> Song {
        let mut waves = [[0; WAVE_LEN]; WAVES];
        waves[..DEFAULT_WAVES.len()].copy_from_slice(&DEFAULT_WAVES);
        Song {
            name: ShortString::default(),
            artist: ShortString::default(),
            comment: ShortString::default(),
            instruments: (0..INSTRUMENTS).map(Instrument::default_for).collect(),
            waves,
            ticks_per_row: 7,
            timer_enabled: 0,
            timer_divider: 0,
            patterns: Vec::new(),
            orders: Default::default(),
            routines: Default::default(),
        }
    }
}

impl Song {
    pub fn pattern(&self, key: i32) -> Option<&Pattern> {
        self.patterns.iter().find(|p| p.key == key)
    }

    /// The song as hUGETracker holds it once loaded: a channel with fewer
    /// orders than the longest plays pattern 0 for the rest (tracker.pas
    /// `CopyOrderMatrixToOrderGrid` shows them as 0, and saving writes them),
    /// and a pattern an order names but the file lacks is blank, added after
    /// the others (hugedatatypes.pas `TPatternMap.GetOrCreateNew`).
    pub fn normalize(&mut self) {
        let len = self.orders.iter().map(Vec::len).max().unwrap_or(0);
        for order in &mut self.orders {
            order.resize(len, 0);
        }
        for c in 0..self.orders.len() {
            for i in 0..len {
                let key = self.orders[c][i];
                if self.pattern(key).is_none() {
                    self.patterns.push(Pattern {
                        key,
                        rows: BLANK_ROWS,
                    });
                }
            }
        }
    }
}
