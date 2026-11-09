//! STAT at a write instruction's start against the mode the write lands in,
//! and `ppu().mode` against a CPU read of STAT, on `mode_sweep.asm` (a VRAM
//! write and an SCX write swept across line 50, one M-cycle later each frame)
//! and its window variant. First written as review-ppu14's probes.
//!
//! Why the help no longer offers a STAT breakpoint as proof (`help traps`,
//! `gbcli recipes`): a condition reads STAT as the instruction starts, and the
//! write lands M-cycles later, sometimes in the next mode. The raster write's
//! `mode` is where it landed.

use std::sync::OnceLock;

use crate::common;

use common::*;
use gbcli_core::{
    AddrFilter, BankSel, Budget, Core, Event, EventKinds, EventSink, IoReg, PpuMode, PpuModeKnown,
    Region, RunRequest, StepUnit, StopReason,
};

const ROM: &[u8] = include_bytes!("../roms/mode_sweep.gb");
// From mode_sweep.sym.
const WRITE_VRAM: u16 = 0x021F;
const WRITE_SCX: u16 = 0x0221;
const DONE: u16 = 0x022D;

/// The landed mode of each SCX write from `WriteScx`.
#[derive(Default)]
struct Scx(Vec<Option<PpuMode>>);

impl EventSink for Scx {
    fn kinds(&self) -> EventKinds {
        EventKinds {
            exec: false,
            raster: true,
            ppu_modes: false,
        }
    }
    fn event(&mut self, event: Event) {
        if let Event::IoWrite { pc, reg, mode, .. } = event
            && pc == WRITE_SCX
            && reg == IoReg::SCX.0
        {
            self.0.push(mode);
        }
    }
}

/// STAT's mode as a condition reads it: at an instruction boundary.
fn stat_mode(core: &dyn Core) -> u8 {
    core.io_register(IoReg::STAT)
        .expect("line 50 has no objects")
        & 3
}

fn filter(addrs: &[u16]) -> AddrFilter {
    let mut f = AddrFilter::new();
    for &a in addrs {
        f.insert(a, BankSel::Any);
    }
    f
}

/// Runs until the PC is one of `stops`' addresses (not counting where it
/// starts), passing raster events to `sink`.
fn run_to<'a>(
    core: &mut dyn Core,
    stops: &'a AddrFilter,
    sink: Option<&'a mut dyn EventSink>,
) -> u16 {
    let mut req = RunRequest::new(Budget::frames(10));
    req.exec = stops;
    req.sink = sink;
    let stop = core.run(&mut req).expect("run");
    assert!(
        matches!(stop.reason, StopReason::Candidate(_)),
        "{:?}",
        stop.reason
    );
    core.registers().pc
}

struct Sweep {
    /// Per frame k, STAT's mode as `WriteVram` and as `WriteScx` start.
    at_start: Vec<(u8, u8)>,
    /// Per frame k, the mode the SCX write landed in.
    landed: Vec<Option<PpuMode>>,
    /// VRAM $8000+k after the run: $FF where frame k's write landed.
    vram: Vec<u8>,
}

/// The sweep, run once for every test that reads it.
fn sweep() -> &'static Sweep {
    static SWEEP: OnceLock<Sweep> = OnceLock::new();
    SWEEP.get_or_init(|| {
        let mut core = quick_dmg(ROM);
        let mut sink = Scx::default();
        let stops = filter(&[WRITE_VRAM, WRITE_SCX, DONE]);
        let mut at_start = Vec::new();
        let mut vram_mode = None;
        loop {
            match run_to(&mut core, &stops, Some(&mut sink)) {
                WRITE_VRAM => vram_mode = Some(stat_mode(&core)),
                WRITE_SCX => at_start.push((vram_mode.take().unwrap(), stat_mode(&core))),
                _ => break,
            }
        }
        // The last write has landed by the next vblank.
        while core.ppu().ly != 145 {
            core.step(StepUnit::Instruction).unwrap();
        }
        let vram = (0..128u16)
            .map(|k| core.bus_read(0x8000 + k).unwrap())
            .collect();
        assert_eq!(at_start.len(), 128);
        assert_eq!(sink.0.len(), 128);
        Sweep {
            at_start,
            landed: sink.0,
            vram,
        }
    })
}

/// The trap's old check, `break Dropped --if '([rSTAT] & 3) == 3'`, missed
/// a dropped VRAM write: one that starts at the end of mode 2 lands in mode
/// 3. (It also stops before writes that start at the end of mode 3 and land
/// in HBlank.) `help traps` now watches the byte instead.
#[test]
fn a_vram_write_that_starts_in_mode_2_can_land_in_mode_3_and_be_dropped() {
    let s = sweep();
    let dropped = |k: usize| s.vram[k] != 0xFF;
    assert!((0..128).any(dropped), "the sweep never reached mode 3");
    let missed: Vec<_> = (0..128)
        .filter(|&k| dropped(k) && s.at_start[k].0 != 3)
        .collect();
    assert!(
        !missed.is_empty() && missed.iter().all(|&k| s.at_start[k].0 == 2),
        "dropped but STAT at the start wasn't 3: {missed:?}"
    );
}

/// The HBlank recipe's old proof, "no stop of `break W --if '([rSTAT] & 3)
/// != 0'`", missed a write that starts late in mode 0 and lands in the next
/// line's OAM scan. The raster write's `mode` is where it landed, so the
/// recipe now makes that the proof.
#[test]
fn an_scx_write_that_starts_in_hblank_can_land_in_the_oam_scan() {
    let s = sweep();
    let late: Vec<_> = (0..128)
        .filter(|&k| s.at_start[k].1 == 0 && s.landed[k] != Some(PpuMode::HBlank))
        .map(|k| (k, s.landed[k]))
        .collect();
    assert!(
        !late.is_empty(),
        "no write started in mode 0 and landed elsewhere"
    );
    assert!(
        late.iter().all(|(_, m)| *m == Some(PpuMode::OamScan)),
        "{late:?}"
    );
    for mode in [PpuMode::OamScan, PpuMode::Drawing, PpuMode::HBlank] {
        assert!(s.landed.contains(&Some(mode)), "{mode:?}: {:?}", s.landed);
    }
}

const WIN_ROM: &[u8] = include_bytes!("../roms/mode_sweep_win.gb");
// From mode_sweep_win.sym: `Frame` + 5 is the instruction after `call
// SleepToLine`, where the CPU wakes at line 50's start.
const WIN_WOKE: u16 = 0x018E + 5;
const WIN_DONE: u16 = 0x0236;

fn stat_mode_of(read: u8) -> PpuMode {
    match read & 3 {
        0 => PpuMode::HBlank,
        1 => PpuMode::VBlank,
        2 => PpuMode::OamScan,
        _ => PpuMode::Drawing,
    }
}

/// `ppu().mode` (what `show` prints as mode= and the live frame's label
/// names) is "HBlank or drawing" where SameBoy batched a window line's mode 3
/// by its 300-dot bound and only running the PPU could tell 3 from 0, and
/// equal to a CPU read of STAT (on a copy, since a read runs the PPU)
/// everywhere else. In the window the read finds both 3 and 0, never 1 or 2.
#[test]
fn ppu_mode_is_unknown_not_wrong_in_the_late_window() {
    let mut core = quick_dmg(WIN_ROM);
    let stops = filter(&[WIN_WOKE, WIN_DONE]);
    let (mut wrong, mut unknown, mut known) = (Vec::new(), 0, 0);
    let mut window_truths = Vec::new();
    // The late frames (k from 100): the sled's NOPs put boundaries all
    // through line 50 and into 51 there.
    while run_to(&mut core, &stops, None) == WIN_WOKE {
        if core.region(Region::Hram).unwrap().bytes[0] < 100 {
            continue;
        }
        while matches!(core.ppu().ly, 50 | 51) {
            let shown = core.ppu().mode;
            let state = core.save();
            let truth = stat_mode_of(core.bus_read(0xFF41).unwrap());
            core.load(&state).unwrap();
            match shown {
                PpuModeKnown::HBlankOrDrawing => {
                    unknown += 1;
                    if !window_truths.contains(&truth) {
                        window_truths.push(truth);
                    }
                }
                PpuModeKnown::Known(mode) if mode == truth => known += 1,
                other => wrong.push((core.registers().pc, other, truth)),
            }
            core.step(StepUnit::Instruction).unwrap();
        }
    }
    assert!(
        wrong.is_empty(),
        "ppu().mode differs from STAT at {} boundaries, first: {:?}",
        wrong.len(),
        &wrong[..wrong.len().min(3)]
    );
    assert!(unknown > 0, "the sweep never reached the late window");
    assert!(known > unknown, "known {known}, unknown {unknown}");
    window_truths.sort_by_key(|m| *m as u8);
    assert_eq!(
        window_truths,
        [PpuMode::HBlank, PpuMode::Drawing],
        "STAT's mode where ppu() says HBlank or drawing"
    );
}

/// On window lines (the 300-dot batch on a line with no objects), every
/// decoded STAT and IF at every boundary on lines 50-51 equals a CPU read on
/// a copy. Returns (boundaries where a register was refused, decoded).
fn window_truth(mut core: gbcli_sameboy::SameBoy) -> (usize, usize) {
    let (mut refused, mut decoded) = (0, 0);
    while core.registers().pc != WIN_DONE {
        if matches!(core.ppu().ly, 50 | 51) {
            for reg in [IoReg::STAT, IoReg::IF] {
                match core.io_register(reg) {
                    Ok(value) => {
                        let state = core.save();
                        let read = core.bus_read(0xFF00 | u16::from(reg.0)).unwrap();
                        core.load(&state).unwrap();
                        assert_eq!(value, read, "io {:#04x} at {:?}", reg.0, core.now());
                        decoded += 1;
                    }
                    Err(_) => refused += 1,
                }
            }
        }
        core.step(StepUnit::Instruction).unwrap();
    }
    (refused, decoded)
}

#[test]
#[ignore = "pre-release: steps every instruction of 128 frames (about 45 s)"]
fn window_line_stat_matches_cpu_reads_on_a_dmg() {
    let (refused, decoded) = window_truth(quick_dmg(WIN_ROM));
    assert!(decoded > 0, "refused {refused}, decoded {decoded}");
}

#[test]
#[ignore = "pre-release: steps every instruction of 128 frames (about 45 s)"]
fn window_line_stat_matches_cpu_reads_on_a_cgb() {
    let core = quick_boot(gbcli_core::CoreConfig::new(
        gbcli_core::Model::CgbE,
        WIN_ROM.to_vec(),
    ));
    let (refused, decoded) = window_truth(core);
    assert!(decoded > 0, "refused {refused}, decoded {decoded}");
}
