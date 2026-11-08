//! A flash save across a real rebuild (leaf RMi3Kk): `gbcli build`, a flash
//! session that saves, a rebuild that changes code elsewhere, and the save
//! still there. Needs the pinned RGBDS on `PATH`, as the `rgbds` tests do.

use std::fs;

use crate::common::Project;

/// An MBC5 cart whose only ROMX code is in bank 20, so banks 2-19 are
/// unused. From WRAM, as GB Studio's gbvm (flasher_s.s) does, it erases the
/// sector holding bank 12 ($30000-$3FFFF) and programs $42 at its start with
/// the swapped-data commands, then counts the save in wSaved.
const GAME: &str = "\
DEF SAVE_BANK EQU 12

SECTION \"Header\", ROM0[$100]
    nop
    jp Main
    ds $147 - @, 0
    db $1B              ; MBC5+RAM+BATTERY
    db $00              ; rgbfix sets the ROM size
    db $02              ; 8 KiB of cart RAM
    ds $150 - @, 0

SECTION \"Main\", ROM0[$150]
Main:
    di
    ld sp, $DFFF
    xor a
    ld [wSaved], a
    ld hl, Program
    ld de, wProgram
    ld b, ProgramEnd - Program
.copy:
    ld a, [hl+]
    ld [de], a
    inc de
    dec b
    jr nz, .copy
    call wProgram
    ld hl, wSaved
    inc [hl]
.done:
    jr .done

Program:
    ld a, SAVE_BANK
    ld [$2000], a
    ld a, $F0
    ld [$4000], a
    ld a, $A9
    ld [$0AAA], a
    ld a, $56
    ld [$0555], a
    ld a, $80
    ld [$0AAA], a
    ld a, $A9
    ld [$0AAA], a
    ld a, $56
    ld [$0555], a
    ld a, $30
    ld [$4000], a
.erasing:
    ld a, [$4000]
    cp $FF
    jr nz, .erasing
    ld a, $A9
    ld [$0AAA], a
    ld a, $56
    ld [$0555], a
    ld a, $A0
    ld [$0AAA], a
    ld a, $42
    ld [$4000], a
.programming:
    ld a, [$4000]
    cp $42
    jr nz, .programming
    ld a, 1
    ld [$2000], a
    ret
ProgramEnd:

SECTION \"Late\", ROMX, BANK[20]
    db $01

SECTION \"WRAM\", WRAM0[$C000]
wSaved: ds 1
wProgram: ds 128
";

/// Bank 12's first byte, where GAME saves.
const SAVED_AT: usize = 0x3_0000;
/// Bank 20's first byte, the code the rebuild changes.
const LATE_AT: usize = 0x5_0000;

#[test]
fn a_save_in_an_unused_lower_bank_survives_a_rebuild_elsewhere() {
    let p = Project::new("flash-build");
    let main = p.path("game/src/main.asm");
    fs::create_dir_all(main.parent().unwrap()).unwrap();
    fs::write(&main, GAME).unwrap();
    let built = p.rgbds(&["build", "game"]);
    assert_eq!(built.code, 0, "{}{}", built.stdout, built.stderr);
    let ok = |args: &[&str]| {
        let run = p.rgbds(args);
        assert_eq!(run.code, 0, "{args:?}: {}{}", run.stdout, run.stderr);
        run
    };
    let saved = ok(&[
        "run",
        "game/build/game.gb",
        "--frames",
        "120",
        "--read",
        "[$C000]",
        "--session",
        "fl",
        "--flash",
        "generic-wr-aaa-a9",
    ]);
    assert!(
        saved.stdout.contains("read expr=[$C000] value=1 "),
        "{}",
        saved.stdout
    );

    // A code change in bank 20, far from the save: an ordinary run loads
    // the head across the rebuild.
    fs::write(&main, GAME.replace("    db $01", "    db $02")).unwrap();
    ok(&["build", "game"]);
    let crossed = ok(&[
        "run",
        "game/build/game.gb",
        "--frames",
        "1",
        "--session",
        "fl",
    ]);
    assert!(!crossed.stderr.contains("dropped"), "{}", crossed.stderr);
    ok(&["sav", "game.sav", "--session", "fl"]);
    let image = fs::read(p.path("game.flash.gb")).unwrap();
    assert_eq!(image[SAVED_AT], 0x42, "the save crossed the rebuild");
    assert_eq!(image[LATE_AT], 0x02, "and the code is the new build's");
}
