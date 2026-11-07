//! `power_cycle`, `sav` and the ROM header warning through the handler.

use gbcli_protocol::{
    BatteryFrom, ErrorCode, Op, PowerCycleArgs, ReadArgs, Response, ResultBody, SavArgs,
    SavClockState, WriteArg, WriteArgs, WriteKind,
};

use super::Handler;
use super::test_support::{handler, request, run_frames};
use crate::fixture::{Fixture, V1, V2, V2_SYM};

const MBC3_TIMER_RAM_BATTERY: u8 = 0x10;
const MBC5_RAM: u8 = 0x1A;
const RAM_8K: u8 = 0x02;
const SAV_BYTES: u64 = 8 * 1024 + 48;

/// `rom` under another cart type and RAM size, its header checksum fixed.
fn as_cart(rom: &[u8], kind: u8, ram: u8) -> Vec<u8> {
    let mut rom = rom.to_vec();
    rom[0x147] = kind;
    rom[0x149] = ram;
    rom[0x14D] = rom[0x134..0x14D]
        .iter()
        .fold(0u8, |x, b| x.wrapping_sub(*b).wrapping_sub(1));
    rom
}

fn ok(response: Response) -> (ResultBody, Vec<String>) {
    let warnings = response.warnings.clone();
    let body = response.outcome.unwrap_or_else(|e| panic!("{e:?}"));
    (body, warnings)
}

fn poke_sram(h: &mut Handler, bytes: &[u8]) {
    ok(h.handle(&request(Op::Write(WriteArgs {
        writes: vec![WriteArg {
            kind: WriteKind::Poke,
            target: "sram:$10".into(),
            bytes: bytes.to_vec(),
        }],
    }))));
}

fn read(h: &mut Handler, expr: &str) -> i64 {
    let (body, _) = ok(h.handle(&request(Op::Read(ReadArgs {
        reads: vec![expr.into()],
        since: None,
        allow_uninit: true,
    }))));
    body.reads.unwrap()[0].value
}

fn power_cycle(h: &mut Handler, args: PowerCycleArgs) -> Response {
    h.handle(&request(Op::PowerCycle(args)))
}

#[test]
fn a_header_that_disagrees_with_its_file_warns_on_every_request_with_the_fix() {
    let mut rom = V1.to_vec();
    rom[0x148] = 0x01;
    let f = Fixture::with_rom("header-warn", &as_cart(&rom, 0x00, 0x00));
    let mut h = handler(&f);
    for _ in 0..2 {
        let (_, warnings) = ok(run_frames(&mut h, 1));
        assert!(
            warnings.iter().any(|w| w.contains("declares a 64 KiB ROM")
                && w.contains("file is 32 KiB")
                && w.contains("rgbfix -p 0xFF")),
            "{warnings:?}"
        );
    }
    let (_, warnings) = ok(h.handle(&request(Op::Show)));
    assert!(warnings.iter().any(|w| w.contains("$0148")), "{warnings:?}");
}

#[test]
fn a_well_formed_header_adds_no_warning() {
    let f = Fixture::new("header-quiet");
    let mut h = handler(&f);
    let (_, warnings) = ok(run_frames(&mut h, 1));
    assert_eq!(warnings, Vec::<String>::new());
}

#[test]
fn a_power_cycle_keeps_battery_ram_and_starts_again_at_power_on() {
    let f = Fixture::with_rom("cycle", &as_cart(V1, MBC3_TIMER_RAM_BATTERY, RAM_8K));
    let mut h = handler(&f);
    ok(run_frames(&mut h, 5));
    poke_sram(&mut h, &[0xAB]);
    let (body, _) = ok(power_cycle(&mut h, PowerCycleArgs::default()));
    let made = body.power_cycle.unwrap();
    assert_eq!(made.battery, BatteryFrom::Head);
    assert_eq!(made.bytes, Some(SAV_BYTES));
    assert_eq!(made.clock, SavClockState::Restored);
    assert!(!made.rom_rebuilt);
    assert_eq!(read(&mut h, "[sram:$10]"), 0xAB);
    let (shown, _) = ok(h.handle(&request(Op::Show)));
    assert_eq!(
        shown.registers.unwrap().pc,
        0x0000,
        "at power-on, in the boot ROM"
    );
}

#[test]
fn a_cart_without_a_battery_comes_back_with_blank_ram() {
    let f = Fixture::with_rom("cycle-nobat", &as_cart(V1, MBC5_RAM, RAM_8K));
    let mut h = handler(&f);
    ok(run_frames(&mut h, 2));
    poke_sram(&mut h, &[0xAB]);
    let (body, _) = ok(power_cycle(&mut h, PowerCycleArgs::default()));
    assert_eq!(body.power_cycle.unwrap().battery, BatteryFrom::None);
    assert_eq!(read(&mut h, "[sram:$10]"), 0xFF);
}

#[test]
fn a_power_cycle_carries_the_battery_across_a_rebuild_without_the_flag() {
    let f = Fixture::with_rom(
        "cycle-rebuild",
        &as_cart(V1, MBC3_TIMER_RAM_BATTERY, RAM_8K),
    );
    let mut h = handler(&f);
    ok(run_frames(&mut h, 2));
    poke_sram(&mut h, &[0xCD]);
    f.rebuild(&as_cart(V2, MBC3_TIMER_RAM_BATTERY, RAM_8K), V2_SYM);
    let (body, warnings) = ok(power_cycle(&mut h, PowerCycleArgs::default()));
    let made = body.power_cycle.unwrap();
    assert!(made.rom_rebuilt);
    assert_eq!(made.battery, BatteryFrom::Head);
    assert!(
        warnings.iter().any(|w| w.contains("another build")),
        "{warnings:?}"
    );
    assert_eq!(read(&mut h, "[sram:$10]"), 0xCD);
}

#[test]
fn across_a_rebuild_the_cart_on_disk_decides_whether_there_is_a_battery() {
    // The old ROM is gone, so the head's state is read under the new header:
    // a build that adds a battery keeps the RAM, one that drops it doesn't.
    for (before, after, kept) in [
        (MBC5_RAM, MBC3_TIMER_RAM_BATTERY, true),
        (MBC3_TIMER_RAM_BATTERY, MBC5_RAM, false),
    ] {
        let f = Fixture::with_rom("cycle-cart", &as_cart(V1, before, RAM_8K));
        let mut h = handler(&f);
        ok(run_frames(&mut h, 2));
        poke_sram(&mut h, &[0xCD]);
        f.rebuild(&as_cart(V2, after, RAM_8K), V2_SYM);
        let (body, _) = ok(power_cycle(&mut h, PowerCycleArgs::default()));
        let made = body.power_cycle.unwrap();
        assert!(made.rom_rebuilt);
        let expected = if kept {
            BatteryFrom::Head
        } else {
            BatteryFrom::None
        };
        assert_eq!(made.battery, expected);
        let byte = if kept { 0xCD } else { 0xFF };
        assert_eq!(
            read(&mut h, "[sram:$10]"),
            byte,
            "{before:#x} -> {after:#x}"
        );
    }
}

#[test]
fn an_exported_sav_imports_into_another_sessions_power_cycle() {
    let f = Fixture::with_rom("sav", &as_cart(V1, MBC3_TIMER_RAM_BATTERY, RAM_8K));
    let mut h = handler(&f);
    ok(run_frames(&mut h, 2));
    poke_sram(&mut h, &[0x5A, 0xA5]);
    let path = f.dir.join("game.sav");
    let (body, _) = ok(h.handle(&request(Op::Sav(SavArgs {
        path: path.to_string_lossy().into(),
    }))));
    let made = body.sav.unwrap();
    assert_eq!((made.bytes, made.ram_bytes), (SAV_BYTES, 8 * 1024));
    assert_eq!(std::fs::read(&path).unwrap().len() as u64, SAV_BYTES);

    let g = Fixture::with_rom("sav-into", &as_cart(V1, MBC3_TIMER_RAM_BATTERY, RAM_8K));
    let mut other = handler(&g);
    let (body, _) = ok(power_cycle(
        &mut other,
        PowerCycleArgs {
            sav: Some(path.to_string_lossy().into()),
            rtc_advance: Some("2h".into()),
        },
    ));
    let made = body.power_cycle.unwrap();
    assert_eq!(made.battery, BatteryFrom::Sav);
    assert_eq!(made.rtc_advanced_s, 7200);
    assert_eq!(read(&mut other, "u16[sram:$10]"), 0xA55A);
}

#[test]
fn exporting_from_a_cart_without_a_battery_is_refused_with_the_fix() {
    let f = Fixture::with_rom("sav-nobat", &as_cart(V1, MBC5_RAM, RAM_8K));
    let mut h = handler(&f);
    let path = f.dir.join("x.sav");
    let error = h
        .handle(&request(Op::Sav(SavArgs {
            path: path.to_string_lossy().into(),
        })))
        .outcome
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::BadRequest);
    assert!(error.remedy.contains("rgbfix -m"), "{}", error.remedy);
    assert!(!path.exists());
}

#[test]
fn a_bad_rtc_advance_is_refused_before_anything_changes() {
    let f = Fixture::with_rom("cycle-bad", &as_cart(V1, MBC3_TIMER_RAM_BATTERY, RAM_8K));
    let mut h = handler(&f);
    let response = power_cycle(
        &mut h,
        PowerCycleArgs {
            sav: None,
            rtc_advance: Some("soon".into()),
        },
    );
    assert_eq!(response.outcome.unwrap_err().code, ErrorCode::BadRequest);
}
