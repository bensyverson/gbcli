//! Flash sessions through the binary (leaf 4hM0UN), as a golden transcript:
//! `--flash` at creation, the run's notes, the mask-ROM hint, and `sav` /
//! `power-cycle --sav` with the chip image.

use std::fs;

use crate::common::{Project, golden};

/// Saves $42 to bank 12 with GB Studio's swapped-data sequence, counting it
/// in $C000 once it reads back.
const FLASH_SAVE: &[u8] = include_bytes!("../../../engine/tests/roms/flash_save.gb");

#[test]
fn flash_sessions_save_warn_and_carry_their_chip_in_sav_files() {
    let project = Project::new("flash");
    fs::write(project.path("save.gb"), FLASH_SAVE).unwrap();
    let mut out = String::new();
    for args in [
        &[
            "run",
            "save.gb",
            "--frames",
            "20",
            "--read",
            "[$C000]",
            "--session",
            "fl",
            "--flash",
            "generic-wr-aaa-a9",
        ][..],
        &["sav", "game.sav", "--session", "fl"],
        &[
            "power-cycle",
            "--session",
            "copy",
            "--rom",
            "save.gb",
            "--flash",
            "generic-wr-aaa-a9",
            "--sav",
            "game.sav",
        ],
        &[
            "run",
            "save.gb",
            "--frames",
            "20",
            "--session",
            "straight",
            "--flash",
            "generic-wr-aaa-aa",
            "--json",
        ],
        &["run", "save.gb", "--frames", "20", "--session", "mask"],
        &[
            "show",
            "--session",
            "mask",
            "--rom",
            "save.gb",
            "--flash",
            "generic-wr-aaa-a9",
        ],
        &[
            "run",
            "save.gb",
            "--session",
            "typo",
            "--flash",
            "generic-wr-aaa-a8",
        ],
        &["show", "--session", "fl", "--flash", "generic-wr-aaa-a9"],
    ] {
        out += &project.gbcli(args).transcript(args, &project.dir);
    }
    let image = fs::read(project.path("game.flash.gb")).unwrap();
    assert_eq!(image.len(), 0x20_0000);
    assert_eq!(image[0x3_0000], 0x42);
    golden("flash.txt", &out);
}
