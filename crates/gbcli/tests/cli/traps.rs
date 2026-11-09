//! `gbcli help traps`, each trap's demo driven through the binary: the
//! symptom must still reproduce and the fix must still work, with the
//! command the topic gives. The demos are crates/gbcli/tests/traps/, built
//! by scripts/build-roms.sh (the RGBDS one by `gbcli build`).

use std::fs;

use serde_json::Value;

use crate::common::Project;

/// Each demo the topic names, and the ROM bytes and symbols the tests run.
const DEMOS: &[(&str, &[u8], &[u8])] = &[
    (
        "vram_mode3",
        include_bytes!("../traps/vram_mode3.gb"),
        include_bytes!("../traps/vram_mode3.sym"),
    ),
    (
        "lcd_off",
        include_bytes!("../traps/lcd_off.gb"),
        include_bytes!("../traps/lcd_off.sym"),
    ),
    (
        "halt_bug",
        include_bytes!("../traps/halt_bug.gb"),
        include_bytes!("../traps/halt_bug.sym"),
    ),
    (
        "dma_rom",
        include_bytes!("../traps/dma_rom.gb"),
        include_bytes!("../traps/dma_rom.sym"),
    ),
];

const OLD_SYNTAX: &str = include_str!("../traps/rgbds/old_syntax.asm");
const NEW_SYNTAX: &str = include_str!("../traps/rgbds/new_syntax.asm");

/// A project holding demo `name`'s ROM and .sym.
fn demo(name: &str) -> Project {
    demo_in(name, &format!("trap-{name}"))
}

/// As [`demo`], in a project of its own name, for a second test of a demo.
fn demo_in(name: &str, project: &str) -> Project {
    let (_, rom, sym) = DEMOS.iter().find(|d| d.0 == name).expect("a demo");
    let p = Project::new(project);
    fs::write(p.path(&format!("{name}.gb")), rom).unwrap();
    fs::write(p.path(&format!("{name}.sym")), sym).unwrap();
    p
}

/// `gbcli args… --json`, asserted to exit `code`, as JSON.
fn json(p: &Project, args: &[&str], code: i32) -> Value {
    let args: Vec<&str> = args.iter().copied().chain(["--json"]).collect();
    let run = p.gbcli(&args);
    assert_eq!(run.code, code, "{args:?}: {}{}", run.stdout, run.stderr);
    serde_json::from_str(&run.stdout).unwrap_or_else(|e| panic!("{e}: {}", run.stdout))
}

fn reads(response: &Value) -> Vec<i64> {
    response["result"]["reads"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["value"].as_i64().unwrap())
        .collect()
}

#[test]
fn every_trap_names_a_demo_these_tests_drive() {
    let topic = Project::new("traps-topic").gbcli(&["help", "traps"]);
    assert_eq!(topic.code, 0, "{}", topic.stderr);
    let named: Vec<&str> = topic
        .stdout
        .lines()
        .filter_map(|l| l.trim().strip_prefix("Demo"))
        .map(str::trim)
        .collect();
    assert_eq!(named.len(), 5, "one demo per trap:\n{}", topic.stdout);
    for path in named {
        let stem = path
            .trim_start_matches("crates/gbcli/tests/traps/")
            .trim_end_matches(".asm");
        let driven = DEMOS.iter().any(|d| d.0 == stem) || stem.starts_with("rgbds/");
        assert!(driven, "{path} has no test here");
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(path);
        assert!(file.is_file(), "{path} missing");
    }
}

#[test]
fn a_vram_write_in_mode_3_is_dropped_and_one_in_hblank_lands() {
    let p = demo("vram_mode3");
    let r = json(
        &p,
        &[
            "run",
            "vram_mode3.gb",
            "--session",
            "t",
            "--frames",
            "2",
            "--read",
            "[$8000]",
            "--read",
            "[$8010]",
        ],
        0,
    );
    assert_eq!(reads(&r), [0, 0xFF], "mode 3 dropped, HBlank kept");
}

/// The trap's check, as the topic gives it: a write watch on the byte that
/// holds only when the byte didn't take the value written stops after the
/// dropped write, naming it, and never after the one in HBlank. (A STAT
/// breakpoint, the old check, misses a write that starts at the end of mode
/// 2: `crates/engine/tests/it/ppu_mode.rs`.)
#[test]
fn a_watch_on_the_byte_stops_after_the_dropped_write_only() {
    let check = "gbcli watch write '$8000' --if '[$8000] != new'";
    let topic = Project::new("traps-mode-check").gbcli(&["help", "traps"]);
    assert!(topic.stdout.contains(check), "{}", topic.stdout);
    assert!(!topic.stdout.contains("[rSTAT]"), "{}", topic.stdout);
    let p = demo_in("vram_mode3", "trap-vram_mode3-watch");
    for (byte, writer) in [("$8000", Some("Dropped")), ("$8010", None)] {
        let session = byte.trim_start_matches('$');
        let condition = format!("[{byte}] != new");
        let rom = ["--rom", "vram_mode3.gb", "--session", session];
        let args: Vec<&str> = ["watch", "write", byte, "--if", &condition]
            .into_iter()
            .chain(rom)
            .collect();
        json(&p, &args, 0);
        let r = json(&p, &["run", "--session", session, "--frames", "2"], 0);
        let stop = &r["result"]["stop"];
        match writer {
            Some(label) => {
                assert_eq!(stop["reason"], "watchpoint", "{byte}: {stop}");
                let sym = std::fs::read_to_string(p.path("vram_mode3.sym")).unwrap();
                let at = sym
                    .lines()
                    .find(|l| l.ends_with(&format!(" {label}")))
                    .and_then(|l| l.split([':', ' ']).nth(1))
                    .unwrap();
                let pc = i64::from_str_radix(at, 16).unwrap();
                assert_eq!(stop["writer_pc"], pc, "{byte}: {stop}");
            }
            None => assert_eq!(stop["reason"], "budget_exhausted", "{byte}: {stop}"),
        }
    }
}

#[test]
fn raster_shows_the_lcd_turned_off_mid_frame_and_in_vblank() {
    let p = demo("lcd_off");
    let r = json(
        &p,
        &[
            "run",
            "lcd_off.gb",
            "--session",
            "t",
            "--frames",
            "4",
            "--raster",
        ],
        0,
    );
    let mut off_lines = Vec::new();
    for row in r["result"]["raster"]["rows"].as_array().unwrap() {
        for w in row["writes"].as_array().unwrap() {
            if w["reg"] == "rLCDC" && w["value"].as_u64().unwrap() & 0x80 == 0 {
                off_lines.push(row["line"].as_u64().unwrap());
            }
        }
    }
    assert_eq!(off_lines, [60, 144], "off mid-frame, then in VBlank");
}

#[test]
fn turning_the_lcd_off_mid_frame_warns_and_in_vblank_does_not() {
    let p = demo_in("lcd_off", "trap-lcd_off-warning");
    let run = p.gbcli(&["run", "lcd_off.gb", "--session", "t", "--frames", "4"]);
    assert_eq!(run.code, 0, "a warning, not a failure: {}", run.stderr);
    let warnings: Vec<&str> = run
        .stderr
        .lines()
        .filter(|l| l.contains("turned off outside VBlank"))
        .collect();
    assert_eq!(warnings.len(), 1, "only the mid-frame one:\n{}", run.stderr);
    for part in [
        "warning: the LCD was turned off outside VBlank at $015C (OffMidFrame), line 60",
        "wait for LY >= 144 or use the VBlank interrupt",
    ] {
        assert!(warnings[0].contains(part), "{part:?} in {}", warnings[0]);
    }
    // The topic quotes the warning as it reads.
    let topic = p.gbcli(&["help", "traps"]).stdout;
    assert!(
        topic.contains("gbcli run warns: the LCD was turned off outside VBlank"),
        "{topic}"
    );
}

#[test]
fn the_halt_bug_runs_the_next_byte_twice_and_a_nop_absorbs_it() {
    let p = demo("halt_bug");
    let r = json(
        &p,
        &[
            "run",
            "halt_bug.gb",
            "--session",
            "t",
            "--frames",
            "1",
            "--read",
            "[wBugged]",
            "--read",
            "[wFixed]",
        ],
        0,
    );
    assert_eq!(reads(&r), [2, 1]);
    // As the topic says to see it: at the HALT, IME is off and IE & IF set,
    // and two steps leave PC on the byte after it.
    let at = json(
        &p,
        &[
            "run",
            "halt_bug.gb",
            "--session",
            "at",
            "--to",
            "Bugged",
            "--read",
            "ime",
            "--read",
            "[rIE] & [rIF]",
        ],
        0,
    );
    assert_eq!(reads(&at), [0, 1]);
    let pcs: Vec<u64> = (0..2)
        .map(|_| {
            let s = json(&p, &["step", "--session", "at"], 0);
            s["result"]["stop"]["pc"].as_u64().unwrap()
        })
        .collect();
    assert_eq!(pcs[0], pcs[1], "the byte after HALT runs twice");
}

#[test]
fn oam_dma_from_rom_runs_wild_and_from_hram_returns() {
    let p = demo("dma_rom");
    let hram = json(
        &p,
        &[
            "run",
            "dma_rom.gb",
            "--session",
            "t",
            "--to",
            "AfterHramDma",
        ],
        0,
    );
    assert_eq!(hram["result"]["stop"]["symbol"], "AfterHramDma");
    let rom = json(
        &p,
        &[
            "run",
            "--session",
            "t",
            "--to",
            "AfterRomDma",
            "--frames",
            "30",
        ],
        3,
    );
    // It never reached its target: a failure, with the stop attached.
    let error = &rom["error"];
    assert_eq!(error["code"], "target_not_reached", "{error}");
    assert!(
        error["remedy"].as_str().unwrap().contains("illegal_opcode"),
        "{error}"
    );
    let stop = &error["stop"];
    assert_eq!(stop["reason"], "illegal_opcode", "{stop}");
    assert!(
        stop["pc"].as_u64().unwrap() >= 0x8000,
        "ran off the end of ROM: {stop}"
    );
}

#[test]
fn old_rgbds_syntax_fails_the_build_with_located_diagnostics_and_new_builds() {
    let p = Project::new("trap-rgbds");
    for (dir, text) in [("old", OLD_SYNTAX), ("new", NEW_SYNTAX)] {
        fs::create_dir_all(p.path(&format!("{dir}/src"))).unwrap();
        fs::write(p.path(&format!("{dir}/src/main.asm")), text).unwrap();
    }
    let old = p.rgbds(&["build", "old"]);
    assert_eq!(old.code, 4, "{}{}", old.stdout, old.stderr);
    for (line, message) in [
        ("8", "did you mean \\\"DEF rLY EQU ...\\\"?"),
        ("10", "syntax error, unexpected MACRO"),
    ] {
        let row = old
            .stdout
            .lines()
            .find(|l| l.contains(&format!("line={line} ")) && l.contains(message));
        assert!(row.is_some(), "line {line}: {message}\n{}", old.stdout);
    }
    let new = p.rgbds(&["build", "new"]);
    assert_eq!(new.code, 0, "{}{}", new.stdout, new.stderr);
}
