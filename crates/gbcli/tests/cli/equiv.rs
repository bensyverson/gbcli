//! The equivalence check through the binary, as a golden transcript: the
//! same build, a patched build that diverges (exit 1, both sides saved), and
//! restoring side B.

use crate::common::{Project, golden};

#[test]
fn equiv_prints_the_verdict_and_saves_both_sides() {
    let project = Project::new("equiv");
    let mut out = String::new();
    for args in [
        &["run", "demo.gb", "--frames", "2"][..],
        &["equiv", "demo.gb", "--frames", "30", "--ram", "wFrame,2"],
        &[
            "equiv", "--frames", "30", "--patch", "$016B=0", "--ram", "wFrame,2",
        ],
        &["equiv", "--frames", "3", "--patch", "$016B=0", "--json"],
        &["restore", "equiv-b"],
        &["read", "[wFrame]"],
        &["equiv", "--frames", "3", "--ram", "wNowhere"],
        &["equiv", "--ram", "wFrame"],
    ] {
        let mut args = args.to_vec();
        args.extend(["--session", "demo"]);
        out += &project.gbcli(&args).transcript(&args, &project.dir);
    }
    golden("equiv.txt", &out);
}

/// The late-init program (`equiv_late.inc`): wX is first written after the
/// fourth vblank, and the B build keeps it elsewhere, so at frame 1 each side
/// reads its own random power-on byte. The row and the remedy say so. Then
/// side B is the session's ROM with wX's first write patched out: it differs
/// once A writes wX, and a checkpoint after that starts both sides past it.
#[test]
fn equiv_says_when_a_differing_byte_was_never_written() {
    let project = Project::new("equiv-late");
    for (name, bytes) in [
        ("late_a.gb", LATE_A),
        ("late_a.sym", LATE_A_SYM),
        ("late_b.gb", LATE_B),
        ("late_b.sym", LATE_B_SYM),
    ] {
        std::fs::write(project.path(name), bytes).unwrap();
    }
    let mut out = String::new();
    for args in [
        &[
            "equiv",
            "late_b.gb",
            "--frames",
            "20",
            "--ram",
            "wX",
            "--rom",
            "late_a.gb",
        ][..],
        &[
            "equiv",
            "--frames",
            "20",
            "--ram",
            "wX",
            "--patch",
            "$0164=0,0,0",
        ],
        &["run", "--frames", "6", "--checkpoint", "inited"],
        &[
            "equiv",
            "--at",
            "inited",
            "--frames",
            "20",
            "--ram",
            "wX",
            "--patch",
            "$0164=0,0,0",
        ],
    ] {
        let mut args = args.to_vec();
        args.extend(["--session", "late"]);
        out += &project.gbcli(&args).transcript(&args, &project.dir);
    }
    assert!(out.contains("never_written=a,b"), "{out}");
    golden("equiv_late.txt", &out);
}

/// Two builds whose frames end at different times (`equiv_slow_start.asm`
/// starts up 105,008 T longer): the screen diverges and a row says the frames
/// are offset, in text and in `--json`; with `--screen off` the RAM compare
/// stands alone; and with nothing left to compare the check is refused.
#[test]
fn equiv_says_when_the_frames_are_offset_and_compares_ram_alone_with_the_screen_off() {
    let project = Project::new("equiv-offset");
    for (name, bytes) in [
        ("a.gb", EQUIV_A),
        ("a.sym", EQUIV_A_SYM),
        ("slow.gb", SLOW),
        ("slow.sym", SLOW_SYM),
    ] {
        std::fs::write(project.path(name), bytes).unwrap();
    }
    let mut out = String::new();
    let mut json = String::new();
    for args in [
        &[
            "equiv", "slow.gb", "--frames", "60", "--ram", "wX", "--ram", "wY", "--rom", "a.gb",
        ][..],
        &[
            "equiv", "slow.gb", "--frames", "60", "--ram", "wX", "--ram", "wY", "--json",
        ],
        &[
            "equiv", "slow.gb", "--frames", "60", "--ram", "wX", "--ram", "wY", "--screen", "off",
        ],
        &[
            "equiv", "slow.gb", "--frames", "60", "--screen", "off", "--audio", "off",
        ],
    ] {
        let mut args = args.to_vec();
        args.extend(["--keys", "RIGHT@10 DOWN@20", "--session", "offset"]);
        let run = project.gbcli(&args);
        if args.contains(&"--json") {
            json = run.stdout.clone();
        }
        out += &run.transcript(&args, &project.dir);
    }
    assert!(out.contains("\nframe_offset frame="), "{out}");
    assert!(out.contains("--screen off"), "the remedy: {out}");
    let response: serde_json::Value = serde_json::from_str(json.trim()).expect("json");
    let offset = &response["error"]["result"]["equiv"]["divergence"]["frame_offset"];
    assert_eq!(offset["earlier"], "b", "{json}");
    assert!(offset["t"].as_u64().is_some_and(|t| t > 0), "{json}");
    assert!(offset["ticks"].as_u64().is_some(), "{json}");
    golden("equiv_offset.txt", &out);
}

const EQUIV_A: &[u8] = include_bytes!("../../../engine/tests/roms/equiv_a.gb");
const EQUIV_A_SYM: &[u8] = include_bytes!("../../../engine/tests/roms/equiv_a.sym");
const SLOW: &[u8] = include_bytes!("../../../engine/tests/roms/equiv_slow_start.gb");
const SLOW_SYM: &[u8] = include_bytes!("../../../engine/tests/roms/equiv_slow_start.sym");
const LATE_A: &[u8] = include_bytes!("../../../engine/tests/roms/equiv_late_a.gb");
const LATE_A_SYM: &[u8] = include_bytes!("../../../engine/tests/roms/equiv_late_a.sym");
const LATE_B: &[u8] = include_bytes!("../../../engine/tests/roms/equiv_late_b.gb");
const LATE_B_SYM: &[u8] = include_bytes!("../../../engine/tests/roms/equiv_late_b.sym");
