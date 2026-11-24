//! The raster timeline on the wire (design §11): what `run --raster` reports.

use serde::{Deserialize, Serialize};

use crate::PpuModeView;

/// One IO-register write on a line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RasterWriteRow {
    /// The register's hardware.inc name (`rSCX`), or its
    /// address (`$FF4C`) when it has none.
    pub reg: String,
    pub value: u8,
    /// The PC of the instruction that wrote it.
    pub pc: u16,
    /// Emulated ticks (2^23 Hz; 912 per line) from the line's start; null
    /// when the core reported no start for the line. Not a dot.
    pub ticks_into_line: Option<u64>,
    /// The PPU's mode as the write landed, as a CPU read of STAT then would
    /// report it; null while the LCD is off, and where the core can't know
    /// it without running the PPU ahead (on SameBoy: late in a mode 3 it
    /// batched by an upper bound).
    #[serde(default)]
    pub mode: Option<PpuModeView>,
}

/// One line the run passed through that has writes on it, or a stretch with
/// the LCD off (`line` null).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RasterLineRow {
    /// Counts from 0 at the run's first line; goes up where the line number
    /// wraps or the LCD turns on or off.
    pub frame: u32,
    /// The PPU's line (0-153), null with the LCD off.
    pub line: Option<u8>,
    /// Emulated ticks since power-on at the line's start; null when the core
    /// reported none (on SameBoy: line 153, and the first line after the LCD
    /// turns on).
    pub start: Option<u64>,
    pub writes: Vec<RasterWriteRow>,
}

/// The scroll and LCDC one frame's visible lines were drawn with. A whole
/// frame has `first_line` 0 and 144 entries in each array; a run that starts or stops
/// mid-frame has fewer. Entry `i` is line `first_line + i`.
///
/// Each value is the register as that line's first tile fetch began, a few
/// dots after STAT reads 3. LCDC is the whole byte, so a client reads its own
/// bits from it: on a DMG, bit 0 clear draws the line's background and window
/// as color 0, which is how a program blanks a line. A later write is not in
/// the line's entry: the rest of that line shows it, the write is in the
/// line's row, `mode`
/// drawing, and the next line has it. A write in those first few dots also
/// reports `mode` drawing, but is in the entry, since the line is drawn with
/// it. The frame after the LCD comes on is listed too, though the core
/// publishes that frame blank.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RasterFrameScroll {
    /// The same count as [`RasterLineRow::frame`].
    pub frame: u32,
    pub first_line: u8,
    pub scx: Vec<u8>,
    pub scy: Vec<u8>,
    /// LCDC as each line began; empty in a document from before protocol 13.
    #[serde(default)]
    pub lcdc: Vec<u8>,
}

/// What `run --raster` found. Only the lines with writes are listed; `lines`
/// counts every line the run passed through.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RasterMade {
    /// Frames the timeline touched, counting a partial first and last.
    pub frames: u32,
    /// Lines the run passed through, with or without writes.
    pub lines: u64,
    pub rows: Vec<RasterLineRow>,
    /// The scroll every drawn line began with, a frame at a time.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drawn: Vec<RasterFrameScroll>,
    /// What the core can't place, by feature name (`ppu_dot`,
    /// `ppu_mode_events`, `line_scroll`); each is also in the gap ledger.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unsupported: Vec<String>,
}

/// One instruction's writes to one register over a raster timeline, counted
/// by the PPU mode each landed in: whether every scroll or palette write
/// lands in HBlank, answered in a row per writer however long the run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RasterWriter {
    pub reg: String,
    pub pc: u16,
    pub writes: u64,
    pub hblank: u64,
    pub vblank: u64,
    pub oam_scan: u64,
    pub drawing: u64,
    /// Writes whose mode the core couldn't know (`mode` null, LCD on).
    pub unknown: u64,
    /// Writes made while the LCD was off (`line` null): no mode, and safe.
    pub lcd_off: u64,
}

impl RasterMade {
    /// Every write counted by register and writer PC, riskiest first: by
    /// writes that landed in drawing, then of unknown mode, then in OAM scan,
    /// most first (a write with the LCD off is safe, `lcd_off`). A big game
    /// has hundreds of writers and a reader sees the first few, so those that
    /// may have changed a line being drawn lead; the safe ones keep the order
    /// each pair first wrote.
    pub fn writers(&self) -> Vec<RasterWriter> {
        let mut writers: Vec<RasterWriter> = Vec::new();
        for (r, w) in self
            .rows
            .iter()
            .flat_map(|r| r.writes.iter().map(move |w| (r, w)))
        {
            let at = match writers.iter().position(|x| x.pc == w.pc && x.reg == w.reg) {
                Some(at) => at,
                None => {
                    writers.push(RasterWriter {
                        reg: w.reg.clone(),
                        pc: w.pc,
                        ..RasterWriter::default()
                    });
                    writers.len() - 1
                }
            };
            let writer = &mut writers[at];
            writer.writes += 1;
            *match w.mode {
                _ if r.line.is_none() => &mut writer.lcd_off,
                Some(PpuModeView::Hblank) => &mut writer.hblank,
                Some(PpuModeView::Vblank) => &mut writer.vblank,
                Some(PpuModeView::OamScan) => &mut writer.oam_scan,
                Some(PpuModeView::Drawing) => &mut writer.drawing,
                None => &mut writer.unknown,
            } += 1;
        }
        // Stable, so ties keep first-write order.
        writers.sort_by_key(|w| std::cmp::Reverse((w.drawing, w.unknown, w.oam_scan)));
        writers
    }
}

#[cfg(test)]
mod tests {
    use super::{RasterFrameScroll, RasterLineRow, RasterMade, RasterWriteRow, RasterWriter};
    use crate::PpuModeView;

    /// A document from before protocol 13 has no `lcdc`; it reads as empty,
    /// not as an error, and an empty one is written out as `[]` beside the
    /// scroll arrays.
    #[test]
    fn a_frame_scroll_without_lcdc_reads_as_empty() {
        let old = r#"{"frame":0,"first_line":0,"scx":[1],"scy":[2]}"#;
        let f: RasterFrameScroll = serde_json::from_str(old).expect("parses");
        assert!(f.lcdc.is_empty());
        let new: RasterFrameScroll =
            serde_json::from_str(r#"{"frame":0,"first_line":0,"scx":[1],"scy":[2],"lcdc":[145]}"#)
                .unwrap();
        assert_eq!(new.lcdc, vec![0x91]);
    }

    fn write(reg: &str, pc: u16, mode: Option<PpuModeView>) -> RasterWriteRow {
        RasterWriteRow {
            reg: reg.into(),
            value: 0,
            pc,
            ticks_into_line: None,
            mode,
        }
    }

    fn line(line: u8, writes: Vec<RasterWriteRow>) -> RasterLineRow {
        RasterLineRow {
            frame: 0,
            line: Some(line),
            start: None,
            writes,
        }
    }

    #[test]
    fn writes_count_by_register_and_pc_and_by_the_mode_they_landed_in() {
        let made = RasterMade {
            frames: 1,
            lines: 154,
            rows: vec![
                line(
                    40,
                    vec![
                        write("rSCX", 0x16B, Some(PpuModeView::Hblank)),
                        write("rBGP", 0x17D, None),
                    ],
                ),
                line(41, vec![write("rSCX", 0x16B, Some(PpuModeView::Drawing))]),
                line(42, vec![write("rSCX", 0x200, Some(PpuModeView::Vblank))]),
                line(43, vec![write("rSCX", 0x16B, Some(PpuModeView::Hblank))]),
            ],
            drawn: Vec::new(),
            unsupported: Vec::new(),
        };
        assert_eq!(
            made.writers(),
            vec![
                RasterWriter {
                    reg: "rSCX".into(),
                    pc: 0x16B,
                    writes: 3,
                    hblank: 2,
                    drawing: 1,
                    ..RasterWriter::default()
                },
                RasterWriter {
                    reg: "rBGP".into(),
                    pc: 0x17D,
                    writes: 1,
                    unknown: 1,
                    ..RasterWriter::default()
                },
                RasterWriter {
                    reg: "rSCX".into(),
                    pc: 0x200,
                    writes: 1,
                    vblank: 1,
                    ..RasterWriter::default()
                },
            ]
        );
    }

    /// A big game has hundreds of writer sites and the text shows only the
    /// first few, so the ones that may have changed a line being drawn come
    /// first: by writes in drawing, then of unknown mode, then in OAM scan,
    /// most first; the safe ones (HBlank and VBlank only) keep their order.
    #[test]
    fn writers_come_riskiest_first_and_the_safe_ones_in_first_write_order() {
        use PpuModeView::{Drawing, Hblank, OamScan, Vblank};
        let made = RasterMade {
            frames: 1,
            lines: 154,
            rows: vec![
                line(10, vec![write("rSCX", 0xA, Some(Hblank))]),
                line(11, vec![write("rSCX", 0xB, Some(OamScan))]),
                line(12, vec![write("rSCX", 0xC, None), write("rSCX", 0xC, None)]),
                line(13, vec![write("rSCX", 0xD, Some(Drawing))]),
                line(14, vec![write("rSCY", 0xE, Some(Drawing))]),
                line(15, vec![write("rSCY", 0xE, Some(Drawing))]),
                line(16, vec![write("rSCX", 0xF, Some(Vblank))]),
                line(17, vec![write("rSCX", 0x10, None)]),
            ],
            drawn: Vec::new(),
            unsupported: Vec::new(),
        };
        let order: Vec<u16> = made.writers().iter().map(|w| w.pc).collect();
        assert_eq!(order, vec![0xE, 0xD, 0xC, 0x10, 0xB, 0xA, 0xF]);
    }

    /// From rev-wg's review: a write made with the LCD off has no mode, but
    /// the core knows why, and it is safe, so it counts as `lcd_off`, not
    /// `unknown`, and an init writer doesn't rank above one that wrote in
    /// OAM scan.
    #[test]
    fn writes_with_the_lcd_off_count_apart_and_rank_as_safe() {
        let off = RasterLineRow {
            frame: 0,
            line: None,
            start: None,
            writes: vec![write("rLCDC", 0x150, None), write("rLCDC", 0x150, None)],
        };
        let made = RasterMade {
            frames: 1,
            lines: 154,
            rows: vec![
                off,
                line(20, vec![write("rSCX", 0x200, Some(PpuModeView::OamScan))]),
            ],
            drawn: Vec::new(),
            unsupported: Vec::new(),
        };
        let writers = made.writers();
        assert_eq!(writers[0].pc, 0x200);
        assert_eq!((writers[1].lcd_off, writers[1].unknown), (2, 0));
    }
}
