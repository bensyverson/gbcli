//! Hardware registers by name (`rLY`, `rDMA`), decoded as the CPU would read
//! them, next to their raw storage (design §2.3, §7).

use gbcli_core::{Core, CoreError, IoReg, Region};

/// Every hardware register gbcli names, in address order.
pub const HW_REGISTERS: &[HwReg] = super::hwreg_table::HW_REGISTERS;

/// How a register's value relates to its raw storage byte (`io:0x44`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The CPU reads the stored byte, with unreadable bits reading 1.
    Stored,
    /// The value comes from internal state and can differ from the stored
    /// byte: DIV from the internal counter, TIMA during reload, JOYP's
    /// switching delay, NR52's channel bits.
    Derived,
    /// Reads give 0xFF whatever is stored.
    WriteOnly,
}

/// Which models have the register. Outside CGB mode, a CGB register reads 0xFF.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Models {
    All,
    Cgb,
}

/// A named group of bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field {
    pub name: &'static str,
    pub lsb: u8,
    pub width: u8,
}

/// A hardware register in the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HwReg {
    /// The hardware.inc name.
    pub name: &'static str,
    /// Other names hardware.inc defines for it.
    pub aliases: &'static [&'static str],
    pub addr: u16,
    pub source: Source,
    pub models: Models,
    /// Empty when the register is one value.
    pub fields: &'static [Field],
}

/// Finds a register by name or alias, ignoring case.
pub fn find(name: &str) -> Option<&'static HwReg> {
    HW_REGISTERS.iter().find(|r| {
        r.name.eq_ignore_ascii_case(name) || r.aliases.iter().any(|a| a.eq_ignore_ascii_case(name))
    })
}

pub fn by_addr(addr: u16) -> Option<&'static HwReg> {
    HW_REGISTERS.iter().find(|r| r.addr == addr)
}

/// One field's bits of a decoded value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldValue {
    pub name: &'static str,
    pub lsb: u8,
    pub width: u8,
    pub value: u8,
}

/// A register read: the decoded value, or why the core can't decode it
/// without a bus read, beside the raw storage byte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HwRegRead {
    pub reg: &'static HwReg,
    /// The stored byte (for a [`Source::Derived`] register, not the value);
    /// `None` if the core exposes no storage for it.
    pub raw: Option<u8>,
    pub value: Result<u8, CoreError>,
}

impl HwRegRead {
    /// The decoded value split into the register's fields; `None` when the
    /// value didn't decode.
    pub fn fields(&self) -> Option<Vec<FieldValue>> {
        let value = *self.value.as_ref().ok()?;
        Some(
            self.reg
                .fields
                .iter()
                .map(|f| FieldValue {
                    name: f.name,
                    lsb: f.lsb,
                    width: f.width,
                    value: (value >> f.lsb) & (((1u16 << f.width) - 1) as u8),
                })
                .collect(),
        )
    }
}

/// Reads a register through the core's decoder; IE, which is storage only,
/// comes from its own region.
pub fn read_register(core: &dyn Core, reg: &'static HwReg) -> HwRegRead {
    let (region, index) = match reg.addr {
        0xFFFF => (Region::Ie, 0),
        addr => (Region::Io, usize::from(addr as u8)),
    };
    let stored = core.region(region).map(|v| v.bytes.get(index).copied());
    let raw = stored.clone().ok().flatten();
    let value = match region {
        Region::Ie => stored.and_then(|b| {
            b.ok_or(CoreError::InvalidState {
                reason: "this core exposes no IE storage".into(),
            })
        }),
        _ => core.io_register(IoReg(reg.addr as u8)),
    };
    HwRegRead { reg, raw, value }
}

/// Reads every register in the table.
pub fn read_all(core: &dyn Core) -> Vec<HwRegRead> {
    HW_REGISTERS
        .iter()
        .map(|r| read_register(core, r))
        .collect()
}
