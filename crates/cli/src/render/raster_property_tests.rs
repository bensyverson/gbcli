//! From rev-wg's review of 5cd60dd, kept as regression coverage:
//! property-style checks of `--raster` text over generated inputs. The text must stay within 64 lines, never panic,
//! show every risky writer or count its risky writes in the `more` row, and
//! account for every line and write it leaves out.

use gbcli_protocol::{PpuModeView, RasterLineRow, RasterMade, RasterWriteRow};

use super::profile::raster;

/// xorshift64*: deterministic, no dependency.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
}

fn mode(r: &mut Rng) -> Option<PpuModeView> {
    match r.below(5) {
        0 => Some(PpuModeView::Hblank),
        1 => Some(PpuModeView::Vblank),
        2 => Some(PpuModeView::OamScan),
        3 => Some(PpuModeView::Drawing),
        _ => None,
    }
}

fn generate(r: &mut Rng) -> RasterMade {
    // Sizes cluster around the caps (19, 20, 21, 39, 40, 41) and go far past.
    let edges = [0u64, 1, 2, 3, 19, 20, 21, 22, 38, 39, 40, 41, 300];
    let n_rows = edges[r.below(edges.len() as u64) as usize];
    let sites = 1 + r.below(60);
    let rows = (0..n_rows)
        .map(|i| {
            let n_writes = edges[r.below(edges.len() as u64) as usize];
            let lcd_off = r.below(10) == 0;
            RasterLineRow {
                frame: (i / 154) as u32,
                line: (!lcd_off).then_some((i % 154) as u8),
                start: None,
                writes: (0..n_writes)
                    .map(|_| RasterWriteRow {
                        reg: ["rSCX", "rSCY", "rBGP", "rWX"][r.below(4) as usize].into(),
                        value: 0,
                        pc: 0x150 + r.below(sites) as u16,
                        ticks_into_line: None,
                        mode: if lcd_off { None } else { mode(r) },
                    })
                    .collect(),
            }
        })
        .collect();
    let unsupported = match r.below(3) {
        0 => vec![],
        1 => vec!["ppu_dot".into()],
        _ => vec!["ppu_dot".into(), "ppu_mode_events".into()],
    };
    RasterMade {
        frames: 1,
        lines: 154,
        rows,
        drawn: Vec::new(),
        unsupported,
    }
}

fn field(line: &str, key: &str) -> u64 {
    line.split(' ')
        .find_map(|f| f.strip_prefix(&format!("{key}=")))
        .map_or(0, |v| v.parse().unwrap())
}

#[test]
fn rev_wg_raster_text_is_bounded_and_loses_no_risky_writer() {
    let mut r = Rng(0x9E37_79B9_7F4A_7C15);
    for case in 0..3000 {
        let made = generate(&mut r);
        let text = raster(&made);
        let lines: Vec<&str> = text.lines().collect();
        assert!(
            lines.len() <= 64,
            "case {case}: {} lines\n{text}",
            lines.len()
        );

        // Every risky write is either on a shown writer row or in the more row.
        let writers = made.writers();
        let risky = |d: u64, u: u64, o: u64| (d, u, o);
        let total = writers.iter().fold((0, 0, 0), |(d, u, o), w| {
            (d + w.drawing, u + w.unknown, o + w.oam_scan)
        });
        let mut seen = (0u64, 0u64, 0u64);
        let mut shown = 0u64;
        for l in lines.iter().filter(|l| l.starts_with("raster_writer ")) {
            shown += 1;
            seen.0 += field(l, "drawing");
            seen.1 += field(l, "unknown");
            seen.2 += field(l, "oam_scan");
        }
        if let Some(more) = lines
            .iter()
            .find(|l| l.starts_with("more kind=raster_writer "))
        {
            shown += field(more, "count");
            seen.0 += field(more, "drawing");
            seen.1 += field(more, "unknown");
            seen.2 += field(more, "oam_scan");
        }
        assert_eq!(shown, writers.len() as u64, "case {case}: writers\n{text}");
        assert_eq!(
            seen,
            risky(total.0, total.1, total.2),
            "case {case}: risky sums\n{text}"
        );

        // Every line and write is shown or counted.
        let mut line_rows = 0u64;
        let mut more_lines = 0u64;
        let mut writes_seen = 0u64;
        for l in &lines {
            if l.starts_with("raster_line ") {
                line_rows += 1;
            } else if l.starts_with("more kind=raster_line ") {
                more_lines += field(l, "count");
            } else if l.starts_with("raster_write ") {
                writes_seen += 1;
            } else if l.starts_with("more kind=raster_write ") {
                writes_seen += field(l, "count");
            }
        }
        assert_eq!(
            line_rows + more_lines,
            made.rows.len() as u64,
            "case {case}\n{text}"
        );
        let writes_in_shown: u64 = made.rows[..line_rows as usize]
            .iter()
            .map(|row| row.writes.len() as u64)
            .sum();
        assert_eq!(writes_seen, writes_in_shown, "case {case}\n{text}");
        // A line row that shows no writes while it has some is a lie by omission.
        let timeline_rows = lines
            .iter()
            .filter(|l| {
                l.starts_with("raster_line ")
                    || l.starts_with("raster_write ")
                    || l.starts_with("more kind=raster_line ")
                    || l.starts_with("more kind=raster_write ")
            })
            .count();
        assert!(timeline_rows <= 40, "case {case}: {timeline_rows}\n{text}");
    }
}
