//! `power-cycle`, `sav` and the ROM header warning through the binary, as a
//! golden transcript.

use std::fs;

use crate::common::{Project, V1, golden};

/// V1 under another cart type, ROM size and RAM size, its checksum fixed.
fn cart(kind: u8, rom_size: u8, ram: u8) -> Vec<u8> {
    let mut rom = V1.to_vec();
    rom[0x147] = kind;
    rom[0x148] = rom_size;
    rom[0x149] = ram;
    rom[0x14D] = rom[0x134..0x14D]
        .iter()
        .fold(0u8, |x, b| x.wrapping_sub(*b).wrapping_sub(1));
    rom
}

#[test]
fn battery_saves_cross_a_power_cycle_and_a_sav_file() {
    let project = Project::new("battery");
    fs::write(project.path("clock.gb"), cart(0x10, 0x00, 0x02)).unwrap();
    fs::write(project.path("plain.gb"), cart(0x1A, 0x00, 0x02)).unwrap();
    fs::write(project.path("bad.gb"), cart(0x00, 0x01, 0x00)).unwrap();
    let mut out = String::new();
    for args in [
        &["run", "clock.gb", "--frames", "2", "--session", "bat"][..],
        &["poke", "sram:$10=$AB", "--session", "bat"],
        &["sav", "game.sav", "--session", "bat"],
        &["power-cycle", "--session", "bat", "--rtc-advance", "2h"],
        &["read", "[sram:$10]", "--session", "bat"],
        &[
            "run",
            "--frames",
            "200",
            "--read",
            "[sram:$10]",
            "--session",
            "bat",
        ],
        &[
            "power-cycle",
            "--session",
            "other",
            "--rom",
            "clock.gb",
            "--sav",
            "game.sav",
        ],
        &["read", "[sram:$10]", "--session", "other", "--json"],
        &["power-cycle", "--session", "plain", "--rom", "plain.gb"],
        &["sav", "plain.sav", "--session", "plain"],
        &["power-cycle", "--session", "bat", "--rtc-advance", "soon"],
        &["show", "--session", "bad", "--rom", "bad.gb"],
    ] {
        out += &project.gbcli(args).transcript(args, &project.dir);
    }
    assert_eq!(
        fs::read(project.path("game.sav")).unwrap().len(),
        8 * 1024 + 48
    );
    assert!(!project.path("plain.sav").exists());
    golden("battery.txt", &out);
}
