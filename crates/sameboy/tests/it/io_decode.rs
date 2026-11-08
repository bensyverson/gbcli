//! Decoded IO registers equal what a CPU read returns, wherever the core
//! decodes them; the bus read that checks it runs on a throwaway copy.

use crate::common;

use common::*;
use gbcli_core::{Budget, Core, CoreConfig, CoreError, Feature, IoReg, Model, Region};
use gbcli_sameboy::SameBoy;

/// Compares every decodable register in FF00-FF7F with a bus read, restoring
/// the state after each read. Returns which registers decoded.
fn assert_decodes_match_bus_reads(core: &mut dyn Core, at: &str) -> [bool; 0x80] {
    let mut decoded = [false; 0x80];
    for reg in 0x00..0x80u8 {
        match core.io_register(IoReg(reg)) {
            Ok(value) => {
                let state = core.save();
                let read = core.bus_read(0xFF00 | u16::from(reg)).unwrap();
                core.load(&state).unwrap();
                assert_eq!(value, read, "io {reg:#04x} ({at})");
                decoded[usize::from(reg)] = true;
            }
            Err(CoreError::Unsupported {
                feature: Feature::SideEffectFreeIo,
                ..
            }) => {}
            Err(err) => panic!("io {reg:#04x} ({at}): {err:?}"),
        }
    }
    decoded
}

fn sweep(core: &mut dyn Core, seed: u64, points: usize) -> [bool; 0x80] {
    let mut rng = Rng::new(seed);
    let mut ever = [false; 0x80];
    for i in 0..points {
        run(core, Budget::instructions(1 + rng.below(2_000)));
        let now = assert_decodes_match_bus_reads(core, &format!("point {i}"));
        for (e, n) in ever.iter_mut().zip(now) {
            *e |= n;
        }
    }
    ever
}

#[test]
fn decoded_registers_match_bus_reads_on_a_dmg() {
    let mut core = quick_dmg(BUSY);
    core.bus_write(0xFF26, 0x80).unwrap(); // APU on, so NRxx hold something
    let ever = sweep(&mut core, 0xD3C0, 40);
    let never: Vec<String> = (0..0x80)
        .filter(|&r| !ever[r])
        .map(|r| format!("{r:#04x}"))
        .collect();
    // NR52, PCM and wave RAM only decode while the APU is synced, which a
    // random boundary rarely is; the APU test below covers them.
    assert!(
        never.iter().all(|r| r == "0x26"),
        "never decoded on DMG: {never:?}"
    );
}

#[test]
fn decoded_registers_match_bus_reads_on_a_cgb() {
    let mut core = SameBoy::new(CoreConfig::new(Model::CgbE, BUSY.to_vec())).unwrap();
    sweep(&mut core, 0xC6B0, 20);
}

#[test]
fn apu_registers_decode_while_the_apu_is_synced() {
    let mut core = quick_dmg(BUSY);
    core.bus_write(0xFF26, 0x80).unwrap(); // NR52: on
    core.bus_write(0xFF12, 0xF0).unwrap(); // NR12
    core.bus_write(0xFF14, 0x80).unwrap(); // NR14: trigger square 1 (syncs the APU)
    let decoded = assert_decodes_match_bus_reads(&mut core, "after an APU write");
    for (reg, ok) in decoded.iter().enumerate().take(0x40).skip(0x10) {
        assert!(ok, "io {reg:#04x} didn't decode with the APU synced");
    }
    assert_eq!(core.io_register(IoReg::NR52).unwrap() & 0x81, 0x81);
}

#[test]
fn unmapped_io_addresses_read_ff() {
    let core = quick_dmg(BUSY);
    for reg in [0x03, 0x08, 0x15, 0x1F, 0x27, 0x4C, 0x7F] {
        assert_eq!(core.io_register(IoReg(reg)), Ok(0xFF), "io {reg:#04x}");
    }
}

#[test]
fn tima_reads_zero_during_reload_while_storage_holds_tma() {
    let mut core = quick_dmg(BUSY);
    run(&mut core, Budget::instructions(30)); // past busy.asm's own timer setup
    core.bus_write(0xFF06, 0xAB).unwrap(); // TMA
    core.bus_write(0xFF07, 0x05).unwrap(); // TAC: on, every 16 cycles
    let mut seen = false;
    for _ in 0..20_000 {
        run(&mut core, Budget::instructions(1));
        let raw = core.region(Region::Io).unwrap().bytes[0x05];
        let decoded = core.io_register(IoReg::TIMA).unwrap();
        if raw == 0xAB && decoded == 0 {
            let state = core.save();
            assert_eq!(core.bus_read(0xFF05).unwrap(), 0);
            core.load(&state).unwrap();
            seen = true;
            break;
        }
    }
    assert!(seen, "never stopped inside a TIMA reload");
}
