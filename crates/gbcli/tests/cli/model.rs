//! The model a session is created with (leaf d7DbmT): `--model gbc` runs a
//! dual-mode ROM as a GBC in CGB mode, while a ROM with no `--model` still
//! runs on the DMG, and a session keeps the model it was made with.

use std::fs;

use crate::common::{Project, read_png};

/// A $80 ROM filling the screen with BG color 0, set to $021F in palette RAM.
const GBC_PALETTE: &[u8] = include_bytes!("../../../engine/tests/roms/gbc_palette.gb");

/// $021F through SameBoy's channel scaling, (x << 3) | (x >> 2).
const ORANGE: [u8; 3] = [0xFF, 0x84, 0x00];
const DMG_WHITE: [u8; 3] = [0xFF; 3];

/// Runs `gbc.gb` for 30 frames in `session` with `extra` flags, and returns
/// the shot's pixels.
fn shot(p: &Project, session: &str, extra: &[&str]) -> Vec<[u8; 3]> {
    let png = format!("{session}.png");
    let mut args = vec![
        "run",
        "gbc.gb",
        "--session",
        session,
        "--frames",
        "30",
        "--shot",
        &png,
        "--scale",
        "1",
    ];
    args.extend(extra);
    let run = p.gbcli(&args);
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    let (width, pixels) = read_png(&p.path(&png));
    assert_eq!(width, 160);
    pixels
}

#[test]
fn a_dual_mode_rom_under_model_gbc_shows_palette_ram_colors() {
    let p = Project::new("model-gbc");
    fs::write(p.path("gbc.gb"), GBC_PALETTE).unwrap();
    let pixels = shot(&p, "color", &["--model", "gbc"]);
    assert!(pixels.iter().all(|&px| px == ORANGE), "{:?}", pixels[0]);
}

#[test]
fn a_dual_mode_rom_without_model_still_runs_on_the_dmg() {
    let p = Project::new("model-default");
    fs::write(p.path("gbc.gb"), GBC_PALETTE).unwrap();
    let pixels = shot(&p, "mono", &[]);
    assert!(pixels.iter().all(|&px| px == DMG_WHITE), "{:?}", pixels[0]);
    p.gbcli(&[
        "show",
        "--session",
        "mono",
        "--rom",
        "gbc.gb",
        "--model",
        "gbc",
    ])
    .fails(2, "bad_request", "drop --model");
}

#[test]
fn gbc_is_cgb_e_to_a_session_and_another_model_is_refused() {
    let p = Project::new("model-alias");
    fs::write(p.path("gbc.gb"), GBC_PALETTE).unwrap();
    shot(&p, "color", &["--model", "gbc"]);
    let same = p.gbcli(&[
        "show",
        "--session",
        "color",
        "--rom",
        "gbc.gb",
        "--model",
        "cgb-e",
    ]);
    assert_eq!(same.code, 0, "{}", same.stderr);
    let refused = p.gbcli(&[
        "show",
        "--session",
        "color",
        "--rom",
        "gbc.gb",
        "--model",
        "dmg",
    ]);
    refused.fails(2, "bad_request", "drop --model");
    assert!(refused.stderr.contains("cgb-e"), "{}", refused.stderr);
}
