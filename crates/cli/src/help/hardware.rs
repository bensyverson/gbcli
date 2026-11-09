//! `gbcli help hardware`: the DMG on one page. Every figure cites the Pan
//! Docs section it comes from (checked against the page on 2026-11-07;
//! `python3 -I scripts/help/check-doc-links.py` re-checks every link).

pub const HARDWARE: &str = "\
Hardware — the DMG on one page. Each figure cites the Pan Docs section it is
from; read there for more. `gbcli help traps` for the mistakes these cause.

Memory map  https://gbdev.io/pandocs/Memory_Map.html#memory-map
  $0000-$3FFF ROM bank 0              $A000-$BFFF cartridge RAM, if any
  $4000-$7FFF switchable ROM bank     $C000-$DFFF WRAM (8 KiB)
  $8000-$9FFF VRAM (8 KiB)            $FE00-$FE9F OAM
  $FF00-$FF7F IO registers            $FF80-$FFFE HRAM      $FFFF IE
  $E000-$FDFF (echo RAM) and $FEA0-$FEFF: Nintendo says not to use them

Time: a dot is one T-cycle on DMG (two in CGB double speed); 4 dots make an
M-cycle at normal speed. gbcli counts T-cycles in t and measure; --ticks
and ticks count 2^23 Hz ticks: two per T-cycle at normal speed, one at double.
  https://gbdev.io/pandocs/STAT.html#lcd-status-registers
  https://gbdev.io/pandocs/Rendering.html#terminology

Frame: a line is 456 dots; a frame is 154 lines, 70,224 dots, at 59.7 fps.
Lines 0-143 draw; 144-153 are VBlank: 10 lines, 4,560 dots. In M-cycles
(dots / 4) a frame is 17,556 and VBlank 1,140.
  https://gbdev.io/pandocs/Rendering.html#ppu-modes

PPU modes, every drawn line  https://gbdev.io/pandocs/Rendering.html#ppu-modes
  mode 2  OAM scan   80 dots               OAM blocked
  mode 3  drawing    172-289 dots          VRAM and OAM blocked
  mode 0  HBlank     376 - mode 3's dots   VRAM and OAM free
  mode 1  VBlank     lines 144-153         VRAM and OAM free
Blocked writes are ignored; blocked reads return garbage, usually $FF.
Mode 3 grows by SCX % 8 dots, 6 for the window and 6-11 per object drawn.
  https://gbdev.io/pandocs/Rendering.html#mode-3-length
STAT ($FF41) bits 0-1 hold the mode; LY ($FF44) is the line, 0-153, but
LY reads 0 through most of line 153 (153 only for its first few dots). Test
VBlank by the mode (mode == 1 in gbcli's expressions), not by LY > 143.
  https://gbdev.io/pandocs/STAT.html#ff41--stat-lcd-status
  https://gbdev.io/pandocs/STAT.html#ff44--ly-lcd-y-coordinate-read-only

LCD off: LCDC ($FF40) bit 7 = 0 stops the LCD and PPU; VRAM and OAM are then
free and the screen blank. Turn it off only in VBlank.
  https://gbdev.io/pandocs/LCDC.html#lcdc7--lcd-enable

Interrupts: IE ($FFFF) enables, IF ($FF0F) requests. Bits: 0 VBlank, 1 LCD
(STAT), 2 timer, 3 serial, 4 joypad; bit 0 has the highest priority.
  https://gbdev.io/pandocs/Interrupts.html#ffff--ie-interrupt-enable
  https://gbdev.io/pandocs/Interrupts.html#interrupt-priorities
Handlers sit at $40, $48, $50, $58 and $60. One runs only if IME is set (ei,
reti) and its bit is set in IE and IF; dispatch clears IME and the IF bit and
takes 5 M-cycles. ei takes effect one instruction late.
  https://gbdev.io/pandocs/Interrupts.html#interrupt-handling
  https://gbdev.io/pandocs/Interrupts.html#ime-interrupt-master-enable-flag-write-only
VBlank is requested at LY 144, about 59.7 times a second.
  https://gbdev.io/pandocs/Interrupt_Sources.html#int-40--vblank-interrupt

HALT waits until IE & IF != 0. With IME = 0 and an interrupt already
pending it ends at once and the byte after it is read twice: the HALT bug.
  https://gbdev.io/pandocs/halt.html#halt
  https://gbdev.io/pandocs/halt.html#halt-bug

Objects: 40 in OAM, 4 bytes each, at most 10 per line; 8x8 or 8x16 (LCDC
bit 2).
  https://gbdev.io/pandocs/OAM.html#object-attribute-memory-oam
  https://gbdev.io/pandocs/LCDC.html#lcdc2--obj-size
OAM DMA: writing $XX to DMA ($FF46) copies $XX00-$XX9F to OAM in 160
M-cycles. On DMG the CPU can reach only HRAM meanwhile, so the routine that
starts it and waits runs from HRAM.
  https://gbdev.io/pandocs/OAM_DMA_Transfer.html#oam-dma-transfer
  https://gbdev.io/pandocs/OAM_DMA_Transfer.html#oam-dma-bus-conflicts

More: https://gbdev.io/pandocs/Joypad_Input.html#ff00--p1joyp-joypad
  https://gbdev.io/pandocs/Timer_and_Divider_Registers.html#timer-and-divider-registers
  https://gbdev.io/pandocs/MBCs.html#mbcs
";

#[cfg(test)]
mod tests {
    use super::*;

    /// The Pan Docs links in `text`.
    fn pandocs_links(text: &str) -> Vec<&str> {
        text.split_whitespace()
            .filter(|w| w.starts_with("https://gbdev.io/pandocs/"))
            .collect()
    }

    #[test]
    fn every_figure_in_the_hardware_cheat_sheet_cites_a_pan_docs_section() {
        // A block is a run of lines between blank lines: each figure's
        // citations sit in its own block.
        for block in HARDWARE.split("\n\n").skip(1) {
            let links = pandocs_links(block);
            if block.chars().any(|c| c.is_ascii_digit()) {
                assert!(!links.is_empty(), "no citation for:\n{block}");
            }
            for link in links {
                assert!(link.contains(".html#"), "{link} names no section");
            }
        }
        // A link can't wrap, so only a line that is just a link may be longer.
        for line in HARDWARE.lines() {
            let just_a_link = pandocs_links(line).len() == 1 && !line.trim().contains(' ');
            assert!(line.chars().count() <= 80 || just_a_link, "{line}");
        }
    }

    #[test]
    fn the_cheat_sheet_says_ly_reads_0_through_most_of_line_153() {
        let flat = HARDWARE.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            flat.contains("LY reads 0 through most of line 153"),
            "{flat}"
        );
        assert!(flat.contains("mode == 1"), "{flat}");
    }

    /// `--ticks` counts 2^23 Hz ticks (`gbcli_core::Ticks`), not T-cycles:
    /// `--ticks 1000` stops near t 500 at normal speed (leaf dwM7t6).
    #[test]
    fn the_cheat_sheet_says_a_tick_is_half_a_t_cycle() {
        let flat = HARDWARE.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            flat.contains("--ticks and ticks count 2^23 Hz ticks: two per T-cycle at normal speed"),
            "{flat}"
        );
    }
}
