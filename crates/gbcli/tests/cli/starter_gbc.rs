//! The dual-mode starter (leaf gbHEPa): one `gbcli new` source, built with
//! `--gbc`, draws in color on a GBC and exactly as before on a DMG, and its
//! DMG path never touches the GBC's registers.

use std::fs;

use crate::common::{Project, png_golden, read_png};

/// The GBC-only registers the starter's color path writes. A DMG has none
/// of them, and with no VBK the attribute map would land on the tilemap.
const GBC_REGISTERS: [&str; 5] = ["rVBK", "rBCPS", "rBCPD", "rOCPS", "rOCPD"];

/// The ROM `starter` built: a --gbc build's own (build/gbc/, leaf XtevSe),
/// else the plain one.
fn rom(p: &Project) -> &'static str {
    const GBC: &str = "game/build/gbc/game.gbc";
    if p.path(GBC).is_file() {
        GBC
    } else {
        "game/build/game.gb"
    }
}

/// `gbcli new game`, then `gbcli build game` with `flags`.
fn starter(name: &str, flags: &[&str]) -> Project {
    let p = Project::new(name);
    let made = p.rgbds(&["new", "game"]);
    assert_eq!(made.code, 0, "{}", made.stderr);
    let mut args = vec!["build", "game"];
    args.extend(flags);
    let built = p.rgbds(&args);
    assert_eq!(built.code, 0, "{}{}", built.stdout, built.stderr);
    p
}

/// Runs the starter's ROM for 30 frames in a new session on `model`, and
/// returns the shot's PNG bytes and pixels.
fn shot(p: &Project, model: &str) -> (Vec<u8>, Vec<[u8; 3]>) {
    let png = format!("{model}.png");
    let run = p.gbcli(&[
        "run",
        rom(p),
        "--session",
        model,
        "--model",
        model,
        "--frames",
        "30",
        "--shot",
        &png,
        "--scale",
        "1",
    ]);
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    let (width, pixels) = read_png(&p.path(&png));
    assert_eq!(width, 160);
    (fs::read(p.path(&png)).unwrap(), pixels)
}

fn is_gray(&[r, g, b]: &[u8; 3]) -> bool {
    r == g && g == b
}

#[test]
fn one_gbc_starter_rom_draws_in_color_on_a_gbc_and_as_before_on_a_dmg() {
    let plain = starter("starter-plain", &[]);
    let (_, before) = shot(&plain, "dmg");

    let dual = starter("starter-dual", &["--gbc"]);
    let (dmg_png, dmg) = shot(&dual, "dmg");
    assert!(
        dmg == before,
        "the --gbc ROM draws on a DMG as the plain one"
    );
    png_golden(&dmg_png, "starter_dmg.png");

    let (gbc_png, gbc) = shot(&dual, "gbc");
    // The row of faces (tile row 8, from x = 32) is drawn, its outline
    // colored by the BG palettes: the face's top-middle is outline.
    let background = gbc[0];
    let outline = gbc[8 * 8 * 160 + 32 + 3];
    assert_ne!(outline, background, "the faces are on the GBC screen");
    assert!(!is_gray(&outline), "{outline:?} is a color");
    png_golden(&gbc_png, "starter_gbc.png");
}

#[test]
fn a_starter_built_without_gbc_takes_the_dmg_path_on_a_gbc_too() {
    let plain = starter("starter-plain-gbc", &[]);
    let writes = gbc_register_writes(&plain, "gbc");
    assert!(writes.is_empty(), "{writes:?}");
}

/// The color path is assembled only under `--gbc` (`IF DEF(GBCLI_GBC)`),
/// so a plain build carries none of it.
#[test]
fn only_a_gbc_build_carries_the_color_path() {
    let has = |p: &Project, label: &str| {
        let sym = fs::read_to_string(p.path(rom(p)).with_extension("sym")).unwrap();
        sym.lines().any(|l| l.ends_with(&format!(" {label}")))
    };
    let plain = starter("starter-plain-sym", &[]);
    let dual = starter("starter-dual-sym", &["--gbc"]);
    for label in ["hGbc", "ColorGbc", "FaceBgPalettes", "FaceObjPalettes"] {
        assert!(!has(&plain, label), "{label} in a plain build");
        assert!(has(&dual, label), "{label} in a --gbc build");
    }
}

/// The GBC-only registers written while the starter's ROM boots and draws
/// its first frame in a new session on `model`, in order.
fn gbc_register_writes(p: &Project, model: &str) -> Vec<String> {
    let run = p.gbcli(&[
        "run",
        rom(p),
        "--session",
        model,
        "--model",
        model,
        // Drawing its line of text keeps the LCD off for most of a frame
        // before the color path runs (leaf RDlaLo), so 2 frames end too soon.
        "--frames",
        "3",
        "--raster",
        "--json",
    ]);
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    let line: serde_json::Value = serde_json::from_str(run.stdout.trim()).unwrap();
    let rows = line["result"]["raster"]["rows"].as_array().unwrap();
    assert!(!rows.is_empty(), "the raster saw the boot's writes");
    rows.iter()
        .flat_map(|row| row["writes"].as_array().unwrap())
        .map(|w| w["reg"].as_str().unwrap().to_string())
        .filter(|reg| GBC_REGISTERS.contains(&reg.as_str()))
        .collect()
}

#[test]
fn the_dmg_session_writes_no_gbc_register_while_the_gbc_session_writes_each() {
    let dual = starter("starter-raster", &["--gbc"]);
    let dmg = gbc_register_writes(&dual, "dmg");
    assert!(dmg.is_empty(), "{dmg:?}");
    let gbc = gbc_register_writes(&dual, "gbc");
    for reg in GBC_REGISTERS {
        assert!(gbc.iter().any(|r| r == reg), "{reg} in {gbc:?}");
    }
    // The attribute map goes to bank 1, and bank 0 comes back last.
    assert_eq!(gbc.iter().filter(|r| *r == "rVBK").count(), 2, "{gbc:?}");
}
