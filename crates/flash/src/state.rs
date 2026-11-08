//! The chip's state as bytes: for a checkpoint (everything, busy included)
//! and for a power cycle (only what the array holds). Little-endian:
//!
//! ```text
//! "GBFL" | u8 version | u8 kind | u32 name_len | name
//!        | mode (u8 tag; busy: u8 op, u64 until, u8 data) | u8 dq6
//!        | u32 sectors | { u32 start | u32 len | bytes }*
//! ```

use std::fmt;

use crate::chip::{FlashChip, Mode};
use crate::note::{NoteKind, Operation};

const MAGIC: &[u8; 4] = b"GBFL";
const VERSION: u8 = 1;

/// Which of the two layouts a byte string holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Checkpoint = 0,
    PowerOff = 1,
}

/// Why a saved chip state was refused. The chip is unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlashStateError(pub String);

impl fmt::Display for FlashStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "flash state: {}", self.0)
    }
}

impl std::error::Error for FlashStateError {}

/// A checkpoint state [`FlashChip::check_load`] found whole, ready for
/// [`FlashChip::finish_load`].
#[derive(Debug)]
pub struct CheckedLoad(Decoded);

/// A decoded state.
#[derive(Debug)]
struct Decoded {
    mode: Mode,
    toggle: bool,
    sectors: Vec<(u32, Vec<u8>)>,
}

impl FlashChip {
    /// Everything a checkpoint needs: the command state (busy and DQ6
    /// included) and the bytes of every sector the chip has changed.
    pub fn save(&self, rom: &[u8]) -> Vec<u8> {
        self.encode(Kind::Checkpoint, self.mode, rom)
    }

    /// Takes over a [`save`](FlashChip::save) of a chip on the same board
    /// and ROM. Sectors this chip changed that the state left alone get their
    /// original bytes back. Notes are dropped: they belong to the timeline
    /// the load replaced.
    pub fn load(&mut self, state: &[u8], rom: &mut [u8]) -> Result<(), FlashStateError> {
        let checked = self.check_load(state, rom.len())?;
        self.finish_load(checked, rom);
        Ok(())
    }

    /// The first half of [`load`](FlashChip::load): checks a state whole,
    /// changing nothing, for a caller that has more of its own state to
    /// check before it commits (a core whose emulator may still refuse).
    pub fn check_load(&self, state: &[u8], rom_len: usize) -> Result<CheckedLoad, FlashStateError> {
        self.decode(state, Kind::Checkpoint, rom_len)
            .map(CheckedLoad)
    }

    /// The second half of [`load`](FlashChip::load): takes over a checked
    /// state. `rom` is the array the check was made against.
    pub fn finish_load(&mut self, checked: CheckedLoad, rom: &mut [u8]) {
        let decoded = checked.0;
        self.adopt(&decoded.sectors, rom);
        self.mode = decoded.mode;
        self.toggle = decoded.toggle;
        self.take_notes();
    }

    /// What survives the power going off at `now`: the changed sectors'
    /// bytes, and the operation still running, which
    /// [`power_on`](FlashChip::power_on) notes as cut off.
    pub fn contents(&self, now: u64, rom: &[u8]) -> Vec<u8> {
        let mode = if self.busy(now) {
            self.mode
        } else {
            Mode::Read
        };
        self.encode(Kind::PowerOff, mode, rom)
    }

    /// A chip at power-on taking over [`contents`](FlashChip::contents)
    /// from the same board and ROM.
    pub fn power_on(&mut self, contents: &[u8], rom: &mut [u8]) -> Result<(), FlashStateError> {
        let decoded = self.decode(contents, Kind::PowerOff, rom.len())?;
        self.adopt(&decoded.sectors, rom);
        self.mode = Mode::Read;
        self.toggle = false;
        if let Mode::Busy { op, .. } = decoded.mode {
            self.note(0, 0, 0, NoteKind::CutOff { op });
        }
        Ok(())
    }

    /// A chip at power-on taking over a whole image of a chip on this board
    /// (`rom`'s size, as a flasher's ROM backup holds it): every sector where
    /// `image` differs from the array as built becomes the image's, and the
    /// rest reads as built. Returns those sectors. A wrong size is refused
    /// and changes nothing.
    pub fn power_on_image(
        &mut self,
        image: &[u8],
        rom: &mut [u8],
    ) -> Result<Vec<std::ops::Range<u32>>, FlashStateError> {
        if image.len() != rom.len() {
            return Err(FlashStateError(format!(
                "the flash image is {:#x} bytes, and the {} board's chip holds {:#x}",
                image.len(),
                self.profile.name,
                rom.len()
            )));
        }
        let size = rom.len() as u32;
        let (mut ranges, mut sectors) = (Vec::new(), Vec::new());
        let mut start = 0;
        while start < size {
            let sector = self.profile.sector_of(start, size);
            let span = sector.start as usize..sector.end as usize;
            let built = match self.pristine.get(&sector.start) {
                Some(original) => original.as_slice(),
                None => &rom[span.clone()],
            };
            if image[span.clone()] != *built {
                sectors.push((sector.start, image[span].to_vec()));
                ranges.push(sector.clone());
            }
            start = sector.end;
        }
        self.adopt(&sectors, rom);
        self.mode = Mode::Read;
        self.toggle = false;
        Ok(ranges)
    }

    fn encode(&self, kind: Kind, mode: Mode, rom: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        out.push(kind as u8);
        let name = self.profile.name.as_bytes();
        out.extend_from_slice(&(name.len() as u32).to_le_bytes());
        out.extend_from_slice(name);
        match mode {
            Mode::Busy { op, until, data } => {
                out.push(mode_tag(mode));
                out.push(op_code(op));
                out.extend_from_slice(&until.to_le_bytes());
                out.push(data);
            }
            other => out.push(mode_tag(other)),
        }
        out.push(u8::from(self.toggle));
        out.extend_from_slice(&(self.pristine.len() as u32).to_le_bytes());
        for (&start, original) in &self.pristine {
            let bytes = &rom[start as usize..start as usize + original.len()];
            out.extend_from_slice(&start.to_le_bytes());
            out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(bytes);
        }
        out
    }

    fn decode(&self, bytes: &[u8], want: Kind, rom_len: usize) -> Result<Decoded, FlashStateError> {
        let bad = |why: &str| FlashStateError(why.to_string());
        let mut r = Reader(bytes);
        if r.take(4)? != MAGIC {
            return Err(bad("not a flash chip state"));
        }
        if r.u8()? != VERSION {
            return Err(bad("an unknown version of the flash chip state"));
        }
        if r.u8()? != want as u8 {
            return Err(bad(match want {
                Kind::Checkpoint => "a power-off state where a checkpoint's was expected",
                Kind::PowerOff => "a checkpoint's state where a power-off one was expected",
            }));
        }
        let n = r.u32()? as usize;
        let name = String::from_utf8_lossy(r.take(n)?).into_owned();
        if name != self.profile.name {
            return Err(FlashStateError(format!(
                "the state is from a {name} board, this cart is {}",
                self.profile.name
            )));
        }
        let mode = match r.u8()? {
            0 => Mode::Read,
            1 => Mode::Unlocked1,
            2 => Mode::Unlocked2,
            3 => Mode::Program,
            4 => Mode::Erase0,
            5 => Mode::Erase1,
            6 => Mode::Erase2,
            7 => Mode::Autoselect,
            8 => Mode::Busy {
                op: match r.u8()? {
                    0 => Operation::Program,
                    1 => Operation::SectorErase,
                    2 => Operation::ChipErase,
                    _ => return Err(bad("an unknown operation")),
                },
                until: r.u64()?,
                data: r.u8()?,
            },
            _ => return Err(bad("an unknown command state")),
        };
        let toggle = r.u8()? != 0;
        let count = r.u32()?;
        let mut sectors = Vec::new();
        for _ in 0..count {
            let start = r.u32()?;
            let len = r.u32()? as usize;
            let bytes = r.take(len)?.to_vec();
            if start as usize + len > rom_len {
                return Err(bad("a sector past the end of this ROM"));
            }
            sectors.push((start, bytes));
        }
        if !r.0.is_empty() {
            return Err(bad("trailing bytes"));
        }
        Ok(Decoded {
            mode,
            toggle,
            sectors,
        })
    }

    /// Makes the array hold `sectors` over the ROM it was built from.
    fn adopt(&mut self, sectors: &[(u32, Vec<u8>)], rom: &mut [u8]) {
        let keep: Vec<u32> = sectors.iter().map(|(start, _)| *start).collect();
        let reverted: Vec<u32> = self
            .pristine
            .keys()
            .copied()
            .filter(|start| !keep.contains(start))
            .collect();
        for start in reverted {
            let original = self.pristine.remove(&start).expect("listed above");
            rom[start as usize..start as usize + original.len()].copy_from_slice(&original);
        }
        for (start, bytes) in sectors {
            let range = *start..*start + bytes.len() as u32;
            self.touch(range, rom);
            rom[*start as usize..*start as usize + bytes.len()].copy_from_slice(bytes);
        }
    }
}

fn mode_tag(mode: Mode) -> u8 {
    match mode {
        Mode::Read => 0,
        Mode::Unlocked1 => 1,
        Mode::Unlocked2 => 2,
        Mode::Program => 3,
        Mode::Erase0 => 4,
        Mode::Erase1 => 5,
        Mode::Erase2 => 6,
        Mode::Autoselect => 7,
        Mode::Busy { .. } => 8,
    }
}

fn op_code(op: Operation) -> u8 {
    match op {
        Operation::Program => 0,
        Operation::SectorErase => 1,
        Operation::ChipErase => 2,
    }
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], FlashStateError> {
        if n > self.0.len() {
            return Err(FlashStateError("truncated".into()));
        }
        let (head, tail) = self.0.split_at(n);
        self.0 = tail;
        Ok(head)
    }
    fn u8(&mut self) -> Result<u8, FlashStateError> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, FlashStateError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, FlashStateError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
}
