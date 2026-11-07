//! Whether two machines start in equivalent states: same model, seed, boot
//! ROM and config, at the same tick with the same keys held. Their ROMs and
//! RAM may differ; that is what the check is for.

use std::fmt;

use gbcli_protocol::model_name;

use crate::checkpoint::SaveMeta;
use crate::machine::Machine;

/// A setting the two machines must share to start equivalently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartField {
    Model,
    Seed,
    BootRom,
    /// The render config, joypad bounce and write tracking.
    Config,
    /// Emulated time: frames are compared vblank by vblank, so both must be
    /// at the same point in the frame.
    Ticks,
    Keys,
}

/// The first setting that differs, with each side's value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartMismatch {
    pub field: StartField,
    pub a: String,
    pub b: String,
}

impl fmt::Display for StartMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the machines don't start equivalently: {:?} is {} on A and {} on B",
            self.field, self.a, self.b
        )
    }
}

/// Refuses two machines whose starting states aren't equivalent.
pub fn check(a: &Machine, b: &Machine) -> Result<(), StartMismatch> {
    let (ha, hb) = (
        a.capture(SaveMeta::default()).header,
        b.capture(SaveMeta::default()).header,
    );
    let same = |field, x: String, y: String| {
        if x == y {
            Ok(())
        } else {
            Err(StartMismatch { field, a: x, b: y })
        }
    };
    same(
        StartField::Model,
        model_name(ha.model).into(),
        model_name(hb.model).into(),
    )?;
    same(StartField::Seed, ha.seed.to_string(), hb.seed.to_string())?;
    same(
        StartField::BootRom,
        ha.boot_rom_sha256.to_string(),
        hb.boot_rom_sha256.to_string(),
    )?;
    same(
        StartField::Config,
        format!("{:?}", ha.config),
        format!("{:?}", hb.config),
    )?;
    same(
        StartField::Ticks,
        ha.ticks.to_string(),
        hb.ticks.to_string(),
    )?;
    same(
        StartField::Keys,
        format!("{:#04x}", ha.keys),
        format!("{:#04x}", hb.keys),
    )
}
