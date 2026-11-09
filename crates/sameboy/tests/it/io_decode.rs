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

/// Per line (50, 100): boundaries where the display was batched, PPU
/// registers decoded, and PPU registers refused.
type BatchedSweep = ([usize; 2], [usize; 2], [usize; 2]);

/// Every instruction boundary on lines 50 and 100 of two frames of
/// ppu_mode.asm, where the display is batched in modes 2 and 3: STAT, LY,
/// IF and the registers only the CPU writes decode wherever the core can
/// know them, and each decoded value is what a CPU read returns.
fn sweep_batched(core: &mut dyn Core) -> BatchedSweep {
    let regs = [
        IoReg::STAT,
        IoReg::LY,
        IoReg::IF,
        IoReg::LCDC,
        IoReg::SCX,
        IoReg::SCY,
        IoReg::LYC,
        IoReg::BGP,
        IoReg::WY,
        IoReg::WX,
    ];
    run(core, Budget::frames(2));
    let start = core.now();
    let (mut batched, mut decoded, mut refused) = ([0usize; 2], [0usize; 2], [0usize; 2]);
    while core.now().0 - start.0 < 2 * 70224 {
        run(core, Budget::instructions(1));
        let side = match core.ppu().ly {
            50 => 0,
            100 => 1,
            _ => continue,
        };
        // HDMA5 is refused exactly while the display is batched.
        batched[side] += usize::from(core.io_register(IoReg::HDMA5).is_err());
        for reg in regs {
            match core.io_register(reg) {
                Ok(value) => {
                    let state = core.save();
                    let read = core.bus_read(0xFF00 | u16::from(reg.0)).unwrap();
                    core.load(&state).unwrap();
                    assert_eq!(value, read, "io {:#04x}, side {side}", reg.0);
                    decoded[side] += 1;
                }
                Err(CoreError::Unsupported {
                    feature: Feature::SideEffectFreeIo,
                    ..
                }) => {
                    // Only STAT and IF depend on where a batched mode 3 ends.
                    assert!(reg == IoReg::STAT || reg == IoReg::IF, "io {:#04x}", reg.0);
                    refused[side] += 1;
                }
                Err(err) => panic!("io {:#04x}: {err:?}", reg.0),
            }
        }
    }
    (batched, decoded, refused)
}

fn assert_batched_sweep((batched, decoded, refused): BatchedSweep) {
    assert!(batched[0] > 0 && batched[1] > 0, "{batched:?}");
    assert!(decoded[0] > 0 && decoded[1] > 0);
    // Line 50 has no objects, so every batch there is exact.
    assert_eq!(refused[0], 0);
    // Line 100's late window: the mode 3 SameBoy batched by its bound.
    assert!(refused[1] > 0);
}

#[test]
fn ppu_registers_decode_while_the_display_is_batched_on_a_dmg() {
    assert_batched_sweep(sweep_batched(&mut quick_dmg(PPU_MODE)));
}

#[test]
fn ppu_registers_decode_while_the_display_is_batched_on_a_cgb() {
    let mut core = quick_boot(CoreConfig::new(Model::CgbE, PPU_MODE.to_vec()));
    assert_batched_sweep(sweep_batched(&mut core));
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
