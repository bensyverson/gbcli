//! docboy's post-boot state, reached through gbcli's own boot ROM
//! (`crates/corpus/roms/docboy_boot.asm`). docboy runs its tests with no boot
//! ROM, from a hand-coded DMG hand-over state; these pin that state, and its
//! timing against the boot tests docboy itself passes.

use std::path::PathBuf;

use gbcli_core::{Core, CoreConfig, CoreError, IoReg, Model, PpuMode, Region};
use gbcli_corpus::docboy;
use gbcli_corpus::drive::Driver;
use gbcli_corpus::judge;
use gbcli_corpus::manifest;
use gbcli_corpus::models::Target;
use gbcli_corpus::rules::Verdict;
use gbcli_sameboy::SameBoy;

const LOGO: [u8; 48] = [
    0xCE, 0xED, 0x66, 0x66, 0xCC, 0x0D, 0x00, 0x0B, 0x03, 0x73, 0x00, 0x83, 0x00, 0x0C, 0x00, 0x0D,
    0x00, 0x08, 0x11, 0x1F, 0x88, 0x89, 0x00, 0x0E, 0xDC, 0xCC, 0x6E, 0xE6, 0xDD, 0xDD, 0xD9, 0x99,
    0xBB, 0xBB, 0x67, 0x63, 0x6E, 0x0E, 0xEC, 0xCC, 0xDD, 0xDC, 0x99, 0x9F, 0xBB, 0xB9, 0x33, 0x3E,
];

fn sameboy(config: CoreConfig) -> Result<Box<dyn Core>, CoreError> {
    Ok(Box::new(SameBoy::new(config)?))
}

fn dmg() -> Target {
    Target {
        model: Model::DmgB,
        substitute_for: None,
    }
}

/// A 32 KiB cartridge holding the logo and `JR -2` at the entry point.
fn idle_rom() -> Vec<u8> {
    let mut rom = vec![0u8; 0x8000];
    rom[0x100..0x102].copy_from_slice(&[0x18, 0xFE]);
    rom[0x104..0x134].copy_from_slice(&LOGO);
    rom
}

fn boot(rom: &[u8]) -> Driver {
    judge::boot(rom, &dmg(), sameboy, docboy::post_boot_rom()).expect("boots")
}

/// A nibble with each bit doubled, as the DMG boot ROM scales the logo.
fn doubled(nibble: u8) -> u8 {
    (0..4).fold(0, |acc, i| acc | (((nibble >> i) & 1) * 0b11) << (2 * i))
}

/// VRAM as docboy's `VRAM0_INITIAL_DATA` holds it for this logo.
fn docboy_vram(logo: &[u8; 48]) -> Vec<u8> {
    let mut vram = vec![0u8; 0x2000];
    let mut at = 0x10;
    for &byte in logo {
        for nibble in [byte >> 4, byte & 0x0F] {
            for _ in 0..2 {
                vram[at] = doubled(nibble);
                at += 2;
            }
        }
    }
    for row in [0x3C, 0x42, 0xB9, 0xA5, 0xB9, 0xA5, 0x42, 0x3C] {
        vram[at] = row;
        at += 2;
    }
    for i in 0..12u8 {
        vram[0x1904 + usize::from(i)] = 1 + i;
        vram[0x1924 + usize::from(i)] = 13 + i;
    }
    vram[0x1910] = 0x19;
    vram
}

#[test]
fn hands_over_with_docboys_registers() {
    let mut d = boot(&idle_rom());
    let r = d.core.registers();
    assert_eq!(
        (r.a, r.f, r.b, r.c, r.d, r.e, r.h, r.l),
        (0x01, 0xB0, 0x00, 0x13, 0x00, 0xD8, 0x01, 0x4D)
    );
    assert_eq!((r.sp, r.pc), (0xFFFE, 0x0100));
    let ppu = d.core.ppu();
    assert_eq!(ppu.mode, Some(PpuMode::VBlank));
    assert!(ppu.lcd_on);
    let mut io = |IoReg(low)| d.core.bus_read(0xFF00 | u16::from(low)).unwrap();
    assert_eq!(io(IoReg::DIV), 0xAB);
    assert_eq!(io(IoReg::LCDC), 0x91);
    assert_eq!(io(IoReg::STAT), 0x85, "LY=LYC, mode 1");
    assert_eq!(io(IoReg::LY), 0x00, "395 dots into line 153, which reads 0");
    assert_eq!(io(IoReg::BGP), 0xFC);
    assert_eq!(io(IoReg::SCY), 0x00);
    assert_eq!(io(IoReg::IF), 0xE1);
    assert_eq!(io(IoReg(0x24)), 0x77, "NR50");
    assert_eq!(io(IoReg(0x25)), 0xF3, "NR51");
    assert_eq!(io(IoReg::NR52), 0xF1, "APU on, channel 1 still on");
    assert_eq!(io(IoReg(0x12)), 0xF3, "NR12");
}

#[test]
fn hands_over_with_docboys_stack_bytes_in_hram() {
    let d = boot(&idle_rom());
    let hram = d.region_bytes(Region::Hram).unwrap();
    // docboy's hram.cpp: $FFFA-$FFFD from the boot ROM's stack, $FFFE from power-on.
    assert_eq!(hram[0x7A..0x7F], [0x39, 0x01, 0x2E, 0x00, 0x4C]);
}

#[test]
fn hands_over_with_docboys_vram() {
    let d = boot(&idle_rom());
    let vram = d.region_bytes(Region::Vram).unwrap();
    let expected = docboy_vram(&LOGO);
    // Spot-check the expansion against docboy's vram0.cpp ($8010, $8190).
    assert_eq!(expected[0x10..0x18], [0xF0, 0, 0xF0, 0, 0xFC, 0, 0xFC, 0]);
    assert_eq!(expected[0x190..0x194], [0x3C, 0, 0x42, 0]);
    assert!(vram == expected, "VRAM differs from docboy's");
}

/// The boot tests docboy passes from its hand-over state that are in the
/// suites `cargo test` already needs: mooneye's DIV, IO and register checks,
/// and GBMicrotest's DIV, LY, STAT and VRAM-lock edges, which pin the DIV and
/// PPU phase to the M-cycle. (`poweron_oam_005` would too, but it reads
/// power-on OAM, which SameBoy's seed 0 makes $FF at $FE35: it fails from any
/// boot.)
const CALIBRATION: &[&str] = &[
    "mooneye/acceptance/boot_div-dmgABCmgb.gb",
    "mooneye/acceptance/boot_hwio-dmgABCmgb.gb",
    "mooneye/acceptance/boot_regs-dmgABC.gb",
    "gbmicrotest/poweron_div_004.gb",
    "gbmicrotest/poweron_div_005.gb",
    "gbmicrotest/poweron_ly_119.gb",
    "gbmicrotest/poweron_ly_120.gb",
    "gbmicrotest/poweron_stat_005.gb",
    "gbmicrotest/poweron_stat_006.gb",
    "gbmicrotest/poweron_oam_006.gb",
    "gbmicrotest/poweron_vram_025.gb",
    "gbmicrotest/poweron_vram_026.gb",
];

#[test]
fn docboys_boot_calibration_rows_pass_from_its_hand_over() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let text = std::fs::read_to_string(root.join("tests/corpus/manifest.json")).unwrap();
    let rows = manifest::parse(&text).unwrap().rows;
    let corpus = root.join("tests/corpus/fetched");
    let ctx = judge::Context {
        corpus: &corpus,
        factory: sameboy,
        dump: None,
    };
    let calibration: Vec<_> = CALIBRATION
        .iter()
        .map(|id| {
            let row = rows.iter().find(|r| r.id == *id).expect("row in manifest");
            assert!(
                corpus.join(&row.path).exists(),
                "{} is missing: fetch the corpus with `python3 -I scripts/corpus/fetch.py`",
                row.path
            );
            row
        })
        .collect();
    // Each boot runs 0.77 emulated seconds; one thread per row keeps the warm suite fast.
    let failed: Vec<String> = std::thread::scope(|scope| {
        let runs: Vec<_> = calibration
            .iter()
            .map(|row| {
                scope.spawn(|| judge::run_with_boot(row, &dmg(), ctx, docboy::post_boot_rom()))
            })
            .collect();
        runs.into_iter()
            .map(|run| run.join().expect("no panic"))
            .filter(|outcome| outcome.verdict != Verdict::Pass)
            .map(|outcome| format!("{}: {:?}", outcome.id, outcome.verdict))
            .collect()
    });
    assert!(failed.is_empty(), "{failed:#?}");
}
