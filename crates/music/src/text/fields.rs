//! The instrument fields each bank shows in readable form, and the stored
//! numbers behind them. A stored number the bank doesn't use is shown on an
//! `unused` line when it differs from the slot's default, so it survives.

use crate::model::{Bank, Instrument};

/// A stored instrument number, by the name an `unused` line gives it.
pub struct Raw {
    pub name: &'static str,
    pub get: fn(&Instrument) -> i64,
    /// `false` when the value doesn't fit the stored width.
    pub set: fn(&mut Instrument, i64) -> bool,
}

fn put_u8(slot: &mut u8, v: i64) -> bool {
    u8::try_from(v).map(|v| *slot = v).is_ok()
}

fn put_i32(slot: &mut i32, v: i64) -> bool {
    i32::try_from(v).map(|v| *slot = v).is_ok()
}

macro_rules! raw {
    ($name:ident, $put:ident) => {
        Raw {
            name: stringify!($name),
            get: |i| i64::from(i.$name),
            set: |i, v| $put(&mut i.$name, v),
        }
    };
}

pub const RAW: &[Raw] = &[
    Raw {
        name: "type",
        get: |i| i64::from(i.type_code),
        set: |i, v| put_i32(&mut i.type_code, v),
    },
    raw!(initial_volume, put_u8),
    raw!(volume_direction, put_i32),
    raw!(volume_pace, put_u8),
    raw!(sweep_time, put_i32),
    raw!(sweep_direction, put_i32),
    raw!(sweep_shift, put_i32),
    raw!(duty, put_u8),
    raw!(output_level, put_i32),
    raw!(waveform, put_i32),
    raw!(counter_step, put_i32),
];

/// The stored numbers `bank`'s readable fields show.
pub fn shown(bank: Bank) -> &'static [&'static str] {
    match bank {
        Bank::Square => &[
            "initial_volume",
            "volume_direction",
            "volume_pace",
            "sweep_time",
            "sweep_direction",
            "sweep_shift",
            "duty",
        ],
        Bank::Wave => &["output_level", "waveform"],
        Bank::Noise => &[
            "initial_volume",
            "volume_direction",
            "volume_pace",
            "counter_step",
        ],
    }
}

/// The readable field lines `bank` takes, in the order they're written.
pub fn lines(bank: Bank) -> &'static [&'static str] {
    match bank {
        Bank::Square => &[
            "length",
            "envelope",
            "sweep",
            "duty",
            "subpattern",
            "unused",
        ],
        Bank::Wave => &["length", "level", "waveform", "subpattern", "unused"],
        Bank::Noise => &["length", "envelope", "mode", "subpattern", "unused"],
    }
}

/// The stored numbers `bank` doesn't show, which an `unused` line may set.
pub fn unused(bank: Bank) -> impl Iterator<Item = &'static Raw> {
    RAW.iter().filter(move |r| !shown(bank).contains(&r.name))
}

/// Steps in the length counter: a note lasts `max - length` of them.
pub fn length_max(bank: Bank) -> i64 {
    if bank == Bank::Wave { 256 } else { 64 }
}
