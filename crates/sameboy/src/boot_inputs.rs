//! The bytes of a ROM that decide where its boot hands over, for a boot
//! cache keyed on them rather than on the whole ROM (design §9): two ROMs
//! that differ only elsewhere hand over in the same state, so a rebuild
//! whose header is unchanged reuses the boot it already ran.
//!
//! Two readers of the ROM run before the handoff. The boot ROM reads the
//! logo ($0104-$0133); the CGB's also the title, CGB flag and licensee
//! codes; the SGB's sends $0104-$014F to the SNES (SameBoy's
//! `BootROMs/*.asm`). SameBoy's cart setup (`GB_configure_cart`, from
//! `GB_load_rom_from_buffer`) reads the header's type and sizes, plus, past
//! the header: $0152-$0153 on a TPP1 cart, the last bank's header for
//! MMM01 and bank $10's logo for MBC1M wiring, both only on a ROM past
//! 32 KB. Those carts are keyed by the whole ROM.

use gbcli_core::Model;

/// The largest ROM whose cart setup reads nothing past the header.
const PLAIN_SIZE: usize = 0x8000;
/// The TPP1 cart type, whose setup reads $0152-$0153.
const TPP1: u8 = 0xBC;
const CART_TYPE: usize = 0x147;
/// The first header byte a boot reads (the logo).
const FIRST_READ: usize = 0x104;
/// One past the last header byte the DMG and CGB boots read: the header
/// checksum is read, the global checksum after it is not.
const DMG_CGB_END: usize = 0x14E;
/// One past the last byte the SGB boot sends: the whole header.
const SGB_END: usize = 0x150;

/// The bytes of `rom` that decide the machine's state at the boot handoff
/// on `model`: its length, then the header bytes the boot reads. `None`
/// when nothing short of the whole ROM does: a ROM without a whole header,
/// past 32 KB, or a TPP1 cart.
pub fn boot_inputs(rom: &[u8], model: Model) -> Option<Vec<u8>> {
    if rom.len() < SGB_END || rom.len() > PLAIN_SIZE || rom[CART_TYPE] == TPP1 {
        return None;
    }
    let end = match model {
        Model::Sgb | Model::Sgb2 => SGB_END,
        _ => DMG_CGB_END,
    };
    let mut inputs = (rom.len() as u64).to_le_bytes().to_vec();
    inputs.extend_from_slice(&rom[FIRST_READ..end]);
    Some(inputs)
}
