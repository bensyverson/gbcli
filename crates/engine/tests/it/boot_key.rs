//! `Machine::boot_key`: what the boot cache keys a machine's handoff on.

use gbcli_core::{CoreConfig, Model};
use gbcli_engine::{Backend, Machine};

use crate::common::{BUSY, config};

/// busy.gb with a byte past the header and its global checksum changed.
fn rebuilt() -> Vec<u8> {
    let mut rom = BUSY.to_vec();
    rom[0x14E] ^= 0xFF;
    rom[0x200] ^= 0xFF;
    rom
}

fn built_in(rom: &[u8]) -> Machine {
    let config = CoreConfig::new(Model::DmgB, rom.to_vec());
    Machine::new(Backend::sameboy(), config, None).expect("machine")
}

#[test]
fn with_the_built_in_boot_the_key_ignores_bytes_no_boot_reads() {
    let (a, b) = (built_in(BUSY), built_in(&rebuilt()));
    assert_ne!(a.rom_sha256(), b.rom_sha256());
    assert_eq!(a.boot_key(), b.boot_key());
    assert_ne!(a.boot_key(), a.rom_sha256(), "not the whole ROM's hash");
    let mut title = BUSY.to_vec();
    title[0x134] ^= 0xFF;
    assert_ne!(built_in(&title).boot_key(), a.boot_key());
}

#[test]
fn with_a_custom_boot_rom_the_key_is_the_whole_rom() {
    // A boot ROM of the caller's may read anything.
    let a = Machine::new(Backend::sameboy(), config(BUSY), None).expect("machine");
    let b = Machine::new(Backend::sameboy(), config(&rebuilt()), None).expect("machine");
    assert_eq!(a.boot_key(), a.rom_sha256());
    assert_ne!(a.boot_key(), b.boot_key());
}
