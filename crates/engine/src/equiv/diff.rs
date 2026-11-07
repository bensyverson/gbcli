//! One frame's comparisons: pixels, RAM ranges, sound-register writes.

use gbcli_core::{ApuWrite, Core, CoreError, Pixels};
use gbcli_protocol::Location;

use super::{AudioCompare, Difference, EquivError, RamRange, Side};
use crate::addr::{self, RegionSizes};
use crate::machine::Machine;
use crate::mem::{self, Target};

/// The last complete frame's pixels, widened to one type.
fn frame_pixels(core: &dyn Core) -> (u32, Vec<u32>) {
    match core.last_complete_frame() {
        None => (0, Vec::new()),
        Some(f) => {
            let pixels = match f.pixels {
                Pixels::Shades(p) => p.iter().map(|&s| u32::from(s)).collect(),
                Pixels::Rgb(p) => p.to_vec(),
            };
            (f.width, pixels)
        }
    }
}

/// The first differing pixel of the two completed frames, and how many differ.
pub fn pixels(a: &dyn Core, b: &dyn Core) -> Option<Difference> {
    let ((width, pa), (_, pb)) = (frame_pixels(a), frame_pixels(b));
    let differs = |i: usize| pa.get(i) != pb.get(i);
    let len = pa.len().max(pb.len());
    let first = (0..len).find(|&i| differs(i))?;
    let width = width.max(1) as usize;
    Some(Difference::Pixels {
        x: (first % width) as u32,
        y: (first / width) as u32,
        a: pa.get(first).copied().unwrap_or(0),
        b: pb.get(first).copied().unwrap_or(0),
        count: (first..len).filter(|&i| differs(i)).count(),
    })
}

/// A range resolved on both sides.
pub struct Resolved {
    range: String,
    len: u16,
    a: Location,
    b: Location,
}

/// Resolves a range through each side's own symbols, and reads it once so a
/// range that can't be read is refused before the run.
pub fn resolve(range: &RamRange, a: &Machine, b: &Machine) -> Result<Resolved, EquivError> {
    let on = |side, m: &Machine| -> Result<Location, EquivError> {
        let core_error = |error| EquivError::Core { side, error };
        let sizes = RegionSizes::of(m.core()).map_err(core_error)?;
        let at =
            addr::parse_on(&range.at, m.symbols(), &sizes).map_err(|error| EquivError::Range {
                side,
                at: range.at.clone(),
                error,
            })?;
        for offset in 0..range.len {
            byte(m.core(), &at, offset).map_err(core_error)?;
        }
        Ok(at)
    };
    Ok(Resolved {
        range: range.at.clone(),
        len: range.len,
        a: on(Side::A, a)?,
        b: on(Side::B, b)?,
    })
}

/// The location of byte `offset` of a range starting at `at`.
fn nth(at: &Location, offset: u16) -> Location {
    Location {
        addr: at.addr.wrapping_add(offset),
        bank: at.bank,
        symbol: None,
    }
}

/// The byte at `offset` into a range, in its own bank if it names one, else
/// in the banks mapped now. Never a bus read.
fn byte(core: &dyn Core, at: &Location, offset: u16) -> Result<u8, CoreError> {
    let at = nth(at, offset);
    let target = match at.bank {
        Some(bank) => mem::target_in_bank(core, bank, at.addr)?,
        None => mem::target(core, at.addr)?,
    };
    match target {
        Target::Backing { region, offset } => mem::read_backing(core, region, offset),
        Target::Io(reg) => core.io_register(reg),
    }
}

/// The range's first differing byte, and how many differ.
pub fn ram(r: &Resolved, a: &dyn Core, b: &dyn Core) -> Result<Option<Difference>, EquivError> {
    let mut first = None;
    let mut count = 0;
    for offset in 0..r.len {
        let va = byte(a, &r.a, offset).map_err(|error| EquivError::Core {
            side: Side::A,
            error,
        })?;
        let vb = byte(b, &r.b, offset).map_err(|error| EquivError::Core {
            side: Side::B,
            error,
        })?;
        if va != vb {
            count += 1;
            first.get_or_insert((offset, va, vb));
        }
    }
    Ok(first.map(|(offset, a, b)| Difference::Ram {
        range: r.range.clone(),
        offset,
        a_at: nth(&r.a, offset),
        b_at: nth(&r.b, offset),
        a,
        b,
        count,
    }))
}

/// The first differing write of a frame's logs. `start` is each side's
/// starting tick, which `Timed` measures write times from.
pub fn apu(
    a: &[ApuWrite],
    b: &[ApuWrite],
    mode: AudioCompare,
    start: (u64, u64),
) -> Option<Difference> {
    let key = |w: &ApuWrite, start: u64| {
        let at = match mode {
            AudioCompare::Timed => Some(w.at.0 - start),
            _ => None,
        };
        (w.addr, w.value, w.effect, at)
    };
    let index = (0..a.len().max(b.len()))
        .find(|&i| a.get(i).map(|w| key(w, start.0)) != b.get(i).map(|w| key(w, start.1)))?;
    Some(Difference::Apu {
        index,
        a: a.get(index).copied(),
        b: b.get(index).copied(),
    })
}
