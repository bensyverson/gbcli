//! Running a built ROM and reading what its music did: each channel's note
//! triggers and the timer setup, from the sound-register write log.

#![allow(dead_code)] // each test binary uses a different subset

use gbcli_core::{ApuWrite, Budget, CoreConfig, IoReg, Model, Region, RunRequest};
use gbcli_engine::{Backend, Machine};

/// A DMG machine on `rom` at power-on, with the built-in boot ROM: the
/// starter waits for the LCD's VBlank, which a boot ROM that leaves the LCD
/// off would never reach.
pub fn machine(rom: &[u8], sym: Option<&str>) -> Machine {
    let config = CoreConfig::new(Model::DmgB, rom.to_vec());
    Machine::new(Backend::sameboy(), config, sym).expect("machine")
}

/// A note-on: the trigger bit written to NRx4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Trigger {
    /// Emulated ticks (2^23 Hz) since power-on.
    pub ticks: u64,
    /// The 11-bit period (square, wave) or NR43's value (noise).
    pub period: u16,
}

pub struct Played {
    /// Per channel: square 1, square 2, wave, noise.
    pub triggers: [Vec<Trigger>; 4],
    pub writes: Vec<ApuWrite>,
    pub ie: u8,
    pub tac: u8,
    pub tma: u8,
}

/// Runs `rom` from power-on until the boot ROM hands over (its chime is
/// not the song's), then for `frames` frames with the write log on.
pub fn play(rom: &[u8], frames: u64) -> Played {
    let mut m = machine(rom, None);
    let core = m.core_mut();
    let mut boot = 0;
    while core.io_register(IoReg::BANK).unwrap() & 1 == 0 {
        assert!(boot < 600, "no handover after {boot} frames");
        core.run(&mut RunRequest::new(Budget::frames(1)))
            .expect("boot");
        boot += 1;
    }
    core.set_apu_write_log(true).unwrap();
    core.run(&mut RunRequest::new(Budget::frames(frames)))
        .expect("run");
    let writes = core.take_apu_writes().unwrap();
    let mut triggers: [Vec<Trigger>; 4] = Default::default();
    let mut low = [0u8; 4];
    for w in &writes {
        match w.addr {
            0xFF13 | 0xFF18 | 0xFF1D => low[usize::from((w.addr - 0xFF13) / 5)] = w.value,
            0xFF22 => low[3] = w.value,
            0xFF14 | 0xFF19 | 0xFF1E | 0xFF23 if w.value & 0x80 != 0 => {
                let c = usize::from((w.addr - 0xFF14) / 5);
                let period = if c == 3 {
                    u16::from(low[3])
                } else {
                    (u16::from(w.value & 7) << 8) | u16::from(low[c])
                };
                triggers[c].push(Trigger {
                    ticks: w.at.0,
                    period,
                });
            }
            _ => {}
        }
    }
    let io = |r| core.io_register(r).unwrap();
    Played {
        ie: core.region(Region::Ie).unwrap().bytes[0],
        tac: io(IoReg::TAC),
        tma: io(IoReg::TMA),
        triggers,
        writes,
    }
}
