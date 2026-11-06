//! The decoded hardware-register table: names, fields, raw vs decoded, and a
//! decoder check of every register against a CPU read.

mod common;

use std::collections::HashSet;

use common::*;
use gbcli_core::{Budget, Core, CoreError, Feature, Region};
use gbcli_engine::inspect::hwreg::{HW_REGISTERS, Source, by_addr, find, read_all, read_register};

/// Every register hardware.inc 4.6 defines in FF00-FFFF (as vendored with
/// SameBoy at crates/sameboy-sys/SameBoy/BootROMs/hardware.inc), by name.
const HARDWARE_INC: [(&str, u16); 61] = [
    ("rP1", 0xFF00),
    ("rSB", 0xFF01),
    ("rSC", 0xFF02),
    ("rDIV", 0xFF04),
    ("rTIMA", 0xFF05),
    ("rTMA", 0xFF06),
    ("rTAC", 0xFF07),
    ("rIF", 0xFF0F),
    ("rNR10", 0xFF10),
    ("rNR11", 0xFF11),
    ("rNR12", 0xFF12),
    ("rNR13", 0xFF13),
    ("rNR14", 0xFF14),
    ("rNR21", 0xFF16),
    ("rNR22", 0xFF17),
    ("rNR23", 0xFF18),
    ("rNR24", 0xFF19),
    ("rNR30", 0xFF1A),
    ("rNR31", 0xFF1B),
    ("rNR32", 0xFF1C),
    ("rNR33", 0xFF1D),
    ("rNR34", 0xFF1E),
    ("rNR41", 0xFF20),
    ("rNR42", 0xFF21),
    ("rNR43", 0xFF22),
    ("rNR44", 0xFF23),
    ("rNR50", 0xFF24),
    ("rNR51", 0xFF25),
    ("rNR52", 0xFF26),
    ("rLCDC", 0xFF40),
    ("rSTAT", 0xFF41),
    ("rSCY", 0xFF42),
    ("rSCX", 0xFF43),
    ("rLY", 0xFF44),
    ("rLYC", 0xFF45),
    ("rDMA", 0xFF46),
    ("rBGP", 0xFF47),
    ("rOBP0", 0xFF48),
    ("rOBP1", 0xFF49),
    ("rWY", 0xFF4A),
    ("rWX", 0xFF4B),
    ("rKEY1", 0xFF4D),
    ("rVBK", 0xFF4F),
    ("rHDMA1", 0xFF51),
    ("rHDMA2", 0xFF52),
    ("rHDMA3", 0xFF53),
    ("rHDMA4", 0xFF54),
    ("rHDMA5", 0xFF55),
    ("rRP", 0xFF56),
    ("rBCPS", 0xFF68),
    ("rBCPD", 0xFF69),
    ("rOCPS", 0xFF6A),
    ("rOCPD", 0xFF6B),
    ("rOPRI", 0xFF6C),
    ("rSVBK", 0xFF70),
    ("rPCM12", 0xFF76),
    ("rPCM34", 0xFF77),
    ("rIE", 0xFFFF),
    // Aliases hardware.inc also defines.
    ("rAUDENA", 0xFF26),
    ("rSPD", 0xFF4D),
    ("rBGPI", 0xFF68),
];

#[test]
fn every_hardware_inc_register_is_in_the_table_by_name_and_address() {
    for (name, addr) in HARDWARE_INC {
        let reg = find(name).unwrap_or_else(|| panic!("{name} missing"));
        assert_eq!(reg.addr, addr, "{name}");
        assert_eq!(by_addr(addr).map(|r| r.addr), Some(addr), "{name}");
    }
    assert_eq!(
        find("rly").map(|r| r.addr),
        Some(0xFF44),
        "names match case-insensitively"
    );
    assert!(find("rNOPE").is_none());
}

#[test]
fn table_addresses_are_unique_and_fields_fit_in_a_byte() {
    let mut seen = HashSet::new();
    for reg in HW_REGISTERS {
        assert!(
            seen.insert(reg.addr),
            "{} repeats {:#06x}",
            reg.name,
            reg.addr
        );
        let mut bits = 0u16;
        for field in reg.fields {
            assert!(field.lsb + field.width <= 8, "{}.{}", reg.name, field.name);
            let mask = ((1u16 << field.width) - 1) << field.lsb;
            assert_eq!(bits & mask, 0, "{}.{} overlaps", reg.name, field.name);
            bits |= mask;
        }
    }
}

/// Reads every table register through the engine and compares the decoded
/// value with a CPU read on a copy. Returns the addresses that decoded.
fn check_all(core: &mut dyn Core, at: &str) -> HashSet<u16> {
    let mut decoded = HashSet::new();
    for read in read_all(core) {
        let reg = read.reg;
        match read.value {
            Ok(value) => {
                let cpu = on_a_copy(core, |core| core.bus_read(reg.addr).unwrap());
                assert_eq!(value, cpu, "{} ({at})", reg.name);
                decoded.insert(reg.addr);
            }
            Err(CoreError::Unsupported {
                feature: Feature::SideEffectFreeIo,
                ..
            }) => {}
            Err(err) => panic!("{} ({at}): {err:?}", reg.name),
        }
    }
    decoded
}

fn sweep(core: &mut dyn Core, seed: u64, points: usize) -> HashSet<u16> {
    let mut rng = Rng::new(seed);
    let mut ever = HashSet::new();
    for i in 0..points {
        run(core, Budget::instructions(1 + rng.below(2_000)));
        ever.extend(check_all(core, &format!("point {i}")));
    }
    // Right after an APU write the APU is synced, so its derived registers decode.
    core.bus_write(0xFF24, 0x77).unwrap();
    ever.extend(check_all(core, "after an APU write"));
    ever
}

#[test]
fn every_register_decodes_as_the_cpu_reads_it_on_a_dmg_and_a_cgb() {
    let mut dmg = quick_dmg(BUSY);
    dmg.bus_write(0xFF26, 0x80).unwrap();
    let mut ever = sweep(&mut dmg, 0x7AB1, 15);
    let mut cgb = booted_cgb(BUSY);
    ever.extend(sweep(&mut cgb, 0x7AB2, 15));
    let never: Vec<&str> = HW_REGISTERS
        .iter()
        .filter(|r| !ever.contains(&r.addr))
        .map(|r| r.name)
        .collect();
    assert!(never.is_empty(), "never decoded: {never:?}");
}

#[test]
fn div_is_derived_from_the_internal_counter() {
    let mut core = quick_dmg(BUSY);
    run(&mut core, Budget::frames(2));
    let div = read_register(&core, find("rDIV").unwrap());
    assert_eq!(div.reg.source, Source::Derived);
    let value = div.value.unwrap();
    assert_eq!(value, on_a_copy(&mut core, |c| c.bus_read(0xFF04).unwrap()));
    assert_eq!(div.raw, Some(core.region(Region::Io).unwrap().bytes[0x04]));
}

#[test]
fn tima_during_reload_reads_zero_while_its_storage_holds_tma() {
    let mut core = quick_dmg(BUSY);
    run(&mut core, Budget::instructions(30)); // past busy.asm's own timer setup
    core.bus_write(0xFF06, 0xAB).unwrap(); // TMA
    core.bus_write(0xFF07, 0x05).unwrap(); // TAC: on, every 16 cycles
    let tima = find("rTIMA").unwrap();
    assert_eq!(tima.source, Source::Derived);
    for _ in 0..20_000 {
        run(&mut core, Budget::instructions(1));
        let read = read_register(&core, tima);
        if read.raw == Some(0xAB) && read.value == Ok(0) {
            assert_eq!(on_a_copy(&mut core, |c| c.bus_read(0xFF05).unwrap()), 0);
            return;
        }
    }
    panic!("never stopped inside a TIMA reload");
}

#[test]
fn fields_split_the_decoded_value() {
    let mut core = quick_dmg(BUSY);
    core.bus_write(0xFF07, 0x05).unwrap(); // TAC: enable, clock select 1
    let tac = read_register(&core, find("rTAC").unwrap());
    let fields = tac.fields().unwrap();
    let get = |name: &str| fields.iter().find(|f| f.name == name).unwrap().value;
    assert_eq!(get("enable"), 1);
    assert_eq!(get("clock"), 1);
    core.bus_write(0xFF40, 0x00).unwrap(); // LCD off: the PPU never batches
    let stat = read_register(&core, find("rSTAT").unwrap());
    assert_eq!(stat.reg.source, Source::Stored);
    let mode = stat.fields().unwrap();
    assert_eq!(mode.iter().find(|f| f.name == "mode").unwrap().value, 0);
}

#[test]
fn ie_reads_from_its_own_region() {
    let mut core = quick_dmg(BUSY);
    core.bus_write(0xFFFF, 0x1D).unwrap();
    let ie = read_register(&core, find("rIE").unwrap());
    assert_eq!((ie.raw, ie.value), (Some(0x1D), Ok(0x1D)));
}
