//! `gbcli help traps`: mistakes that assemble and run but go wrong. Each
//! names a demo in crates/gbcli/tests/traps/ that the CLI tests drive
//! (crates/gbcli/tests/cli/traps.rs), symptom and fix, with the commands
//! given here; `python3 -I scripts/help/check-doc-links.py` checks the
//! links.

pub const TRAPS: &str = "\
Traps — code that builds and runs, then goes wrong. Each has a demo ROM in
gbcli's repository, run by its tests. Rules behind them: gbcli help hardware.

VRAM write during mode 3
  Symptom  a tile or map byte keeps its old value: the PPU owns VRAM in mode
           3 and ignores the write
  See it   gbcli run --frames 2 --read '[$8000]' (the byte you wrote)
  Fix      write VRAM in VBlank or with the LCD off; mid-frame, wait for STAT
           mode 0 or 1 first, and keep the write within that mode
  Docs     https://gbdev.io/pandocs/Accessing_VRAM_and_OAM.html#accessing-vram-and-oam
  Demo     crates/gbcli/tests/traps/vram_mode3.asm

LCD turned off outside VBlank
  Symptom  none in an emulator: the screen just blanks. On a real DMG it can
           burn a line into the LCD
  See it   gbcli run warns: the LCD was turned off outside VBlank, with the
           PC, line and frame. gbcli run --frames 4 --raster lists each rLCDC
           write under its line; 144 to 153 is VBlank
  Fix      wait for VBlank (LY 144 or more, or the VBlank interrupt), then
           clear LCDC bit 7
  Docs     https://gbdev.io/pandocs/LCDC.html#lcdc7--lcd-enable
  Demo     crates/gbcli/tests/traps/lcd_off.asm

HALT bug
  Symptom  with IME off (di) and an interrupt enabled in IE and pending in IF,
           halt ends at once and the byte after it runs twice
  See it   gbcli run --to YourHalt --read ime --read '[rIE] & [rIF]' (0 and
           non-zero: it will bug), then gbcli step twice: pc doesn't move
  Fix      put a nop after halt (RGBDS no longer adds one), or clear IF or
           enable interrupts (ei) before halting
  Docs     https://gbdev.io/pandocs/halt.html#halt-bug
  Demo     crates/gbcli/tests/traps/halt_bug.asm

OAM DMA routine outside HRAM
  Symptom  the game locks up or runs wild just after starting OAM DMA: on DMG
           the CPU can read only HRAM during the copy, so a routine in ROM or
           WRAM executes the bytes being copied
  See it   gbcli run --to AfterDma never arrives: it fails with
           target_not_reached (exit 3), the stop often illegal_opcode with pc
           far from your code
  Fix      copy the routine into HRAM (LOAD \"…\", HRAM … ENDL in RGBDS) and
           call it there; gbcli new does
  Docs     https://gbdev.io/pandocs/OAM_DMA_Transfer.html#oam-dma-bus-conflicts
  Demo     crates/gbcli/tests/traps/dma_rom.asm

Old RGBDS syntax
  Symptom  gbcli build fails on code from an older tutorial or hardware.inc
  See it   gbcli build: diagnostic rows such as Undefined macro `X` (did you
           mean \"DEF X EQU ...\"?) and syntax error, unexpected MACRO
  Fix      X EQU 5 becomes DEF X EQU 5 (also EQUS, =, RB); name: MACRO
           becomes MACRO name; PRINTT becomes PRINT; jp [hl] becomes jp hl;
           ldh [$40] becomes ldh [$FF40]
  Docs     https://rgbds.gbdev.io/docs/v1.0.4/rgbasm-old.5#REPLACED
  Demo     crates/gbcli/tests/traps/rgbds/old_syntax.asm
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_trap_has_a_symptom_a_way_to_see_it_a_fix_a_link_and_a_demo() {
        let traps: Vec<&str> = TRAPS.split("\n\n").skip(1).collect();
        assert_eq!(traps.len(), 5);
        for trap in traps {
            for part in ["Symptom", "See it", "Fix", "Docs", "Demo"] {
                assert!(trap.contains(&format!("  {part} ")), "{part}:\n{trap}");
            }
            let docs = trap.lines().find(|l| l.trim().starts_with("Docs")).unwrap();
            assert!(docs.contains("https://") && docs.contains('#'), "{docs}");
        }
        for line in TRAPS.lines() {
            assert!(
                line.chars().count() <= 80 || line.contains("https://"),
                "{line}"
            );
        }
    }
}
