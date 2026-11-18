//! The write log narrowed to some channels, and counted by register and by
//! the code that wrote it: what `--audio-log` floods with, in a few rows.

use std::collections::{BTreeMap, BTreeSet};

use gbcli_core::ApuChannel;
use gbcli_protocol::{AudioSummary, RegisterWrites, WriterWrites};

use crate::decode::DecodedWrite;
use crate::reg::SoundReg;

/// Wave RAM's first byte: where the summary counts all sixteen.
const WAVE_RAM: u16 = 0xFF30;

impl SoundReg {
    /// Whether a write to this register is `channel`'s: its own NRxy, and
    /// wave RAM for the wave channel. NR50-NR52 are no channel's.
    pub fn is_on(self, channel: ApuChannel) -> bool {
        match self {
            SoundReg::Wave(_) => channel == ApuChannel::Wave,
            reg => reg.channel() == Some(channel),
        }
    }
}

/// The writes to any of `channels`; every write when `channels` is empty.
pub fn on_channels<'a>(log: &'a [DecodedWrite], channels: &[ApuChannel]) -> Vec<&'a DecodedWrite> {
    log.iter()
        .filter(|w| channels.is_empty() || channels.iter().any(|&c| w.reg.is_on(c)))
        .collect()
}

/// A register as the summary counts it: wave RAM's bytes as one.
fn counted(w: &DecodedWrite) -> (u16, String) {
    match w.reg {
        SoundReg::Wave(_) => (WAVE_RAM, "WAVE".to_string()),
        reg => (w.write.addr, reg.to_string()),
    }
}

/// `log` counted by register (wave RAM as one, `WAVE`) and by writer PC.
pub fn summarize(log: &[&DecodedWrite]) -> AudioSummary {
    let mut registers: BTreeMap<(u16, String), u64> = BTreeMap::new();
    let mut writers: BTreeMap<u16, (u64, BTreeSet<(u16, String)>)> = BTreeMap::new();
    for w in log {
        let reg = counted(w);
        *registers.entry(reg.clone()).or_default() += 1;
        let writer = writers.entry(w.write.pc).or_default();
        writer.0 += 1;
        writer.1.insert(reg);
    }
    let mut writers: Vec<WriterWrites> = writers
        .into_iter()
        .map(|(pc, (writes, regs))| WriterWrites {
            pc,
            // Named by the caller, which holds the symbols.
            symbol: None,
            writes,
            registers: regs.into_iter().map(|(_, name)| name).collect(),
        })
        .collect();
    // Stable, so equal counts stay in PC order.
    writers.sort_by_key(|w| std::cmp::Reverse(w.writes));
    AudioSummary {
        writes: log.len() as u64,
        registers: registers
            .into_iter()
            .map(|((addr, reg), writes)| RegisterWrites { reg, addr, writes })
            .collect(),
        writers,
        // Measured from samples, by the caller that rendered them.
        loudness: Vec::new(),
    }
}

/// Names `summary`'s writers with `name` (a `.sym` lookup by PC), leaving
/// unnamed a writer that wrote from the boot ROM in `log`: that code is no
/// cart byte, and a row whose PC both ran counts both, so neither name fits.
pub fn name_writers(
    summary: &mut AudioSummary,
    log: &[&DecodedWrite],
    name: impl Fn(u16) -> Option<String>,
) {
    let boot: BTreeSet<u16> = log
        .iter()
        .filter(|w| w.write.boot_rom)
        .map(|w| w.write.pc)
        .collect();
    for w in &mut summary.writers {
        w.symbol = if boot.contains(&w.pc) {
            None
        } else {
            name(w.pc)
        };
    }
}

#[cfg(test)]
mod tests {
    use gbcli_core::{ApuWrite, ApuWriteEffect, Ticks};

    use super::*;
    use crate::decode::{ApuSnapshot, decode};

    fn write(pc: u16, addr: u16, value: u8) -> ApuWrite {
        ApuWrite {
            at: Ticks(u64::from(pc)),
            frame: 0,
            line: None,
            pc,
            boot_rom: false,
            addr,
            value,
            effect: ApuWriteEffect::Applied,
        }
    }

    /// A writer is named by `name` unless one of its writes ran from the
    /// boot ROM: the cart's labels don't name the boot ROM's code, and the
    /// row counts both when they share a PC.
    #[test]
    fn a_writer_is_named_unless_the_boot_rom_wrote_from_its_pc() {
        let writes = [
            ApuWrite {
                boot_rom: true,
                ..write(0x000E, 0xFF26, 0x80)
            },
            write(0x0400, 0xFF12, 0xF0),
            write(0x0050, 0xFF24, 0x77),
            ApuWrite {
                boot_rom: true,
                ..write(0x0050, 0xFF25, 0xF3)
            },
        ];
        let powered = ApuSnapshot {
            regs: [0; 0x17],
            powered: true,
        };
        let log = decode(&powered, &writes);
        let all: Vec<&DecodedWrite> = log.iter().collect();
        let mut s = summarize(&all);
        name_writers(&mut s, &all, |pc| Some(format!("L{pc:04X}")));
        let names: Vec<(u16, Option<&str>)> = s
            .writers
            .iter()
            .map(|w| (w.pc, w.symbol.as_deref()))
            .collect();
        assert_eq!(
            names,
            vec![(0x0050, None), (0x000E, None), (0x0400, Some("L0400"))]
        );
    }

    /// Music on square 1 from $0400, effects on noise from $1595 and
    /// $1598, the wave channel's RAM, and the master volume.
    fn log() -> Vec<DecodedWrite> {
        let writes = [
            write(0x0400, 0xFF12, 0xF0),
            write(0x0400, 0xFF14, 0x86),
            write(0x1595, 0xFF21, 0xF1),
            write(0x1598, 0xFF23, 0x80),
            write(0x1595, 0xFF21, 0xA1),
            write(0x0500, 0xFF30, 0x01),
            write(0x0500, 0xFF31, 0x23),
            write(0x0600, 0xFF24, 0x77),
        ];
        let powered = ApuSnapshot {
            regs: [0; 0x17],
            powered: true,
        };
        decode(&powered, &writes)
    }

    #[test]
    fn a_channel_owns_its_registers_and_the_wave_channel_its_ram() {
        assert!(SoundReg::Nr41.is_on(ApuChannel::Noise));
        assert!(!SoundReg::Nr41.is_on(ApuChannel::Square1));
        assert!(SoundReg::Nr10.is_on(ApuChannel::Square1));
        assert!(SoundReg::Wave(3).is_on(ApuChannel::Wave));
        assert!(!SoundReg::Wave(3).is_on(ApuChannel::Square2));
        for channel in ApuChannel::ALL {
            assert!(!SoundReg::Nr51.is_on(channel), "{channel:?}");
        }
    }

    #[test]
    fn a_filter_keeps_only_its_channels_writes() {
        let log = log();
        let noise = on_channels(&log, &[ApuChannel::Noise]);
        assert_eq!(noise.len(), 3);
        assert!(noise.iter().all(|w| w.reg.is_on(ApuChannel::Noise)));
        let wave_and_noise = on_channels(&log, &[ApuChannel::Wave, ApuChannel::Noise]);
        assert_eq!(wave_and_noise.len(), 5);
        assert_eq!(on_channels(&log, &[]).len(), log.len(), "no filter");
    }

    #[test]
    fn the_summary_counts_by_register_in_address_order_and_by_writer() {
        let log = log();
        let all: Vec<&DecodedWrite> = log.iter().collect();
        let s = summarize(&all);
        assert_eq!(s.writes, 8);
        let regs: Vec<(&str, u16, u64)> = s
            .registers
            .iter()
            .map(|r| (r.reg.as_str(), r.addr, r.writes))
            .collect();
        assert_eq!(
            regs,
            vec![
                ("NR12", 0xFF12, 1),
                ("NR14", 0xFF14, 1),
                ("NR42", 0xFF21, 2),
                ("NR44", 0xFF23, 1),
                ("NR50", 0xFF24, 1),
                ("WAVE", 0xFF30, 2),
            ]
        );
        let writers: Vec<(u16, u64, Vec<&str>)> = s
            .writers
            .iter()
            .map(|w| {
                (
                    w.pc,
                    w.writes,
                    w.registers.iter().map(String::as_str).collect(),
                )
            })
            .collect();
        assert_eq!(
            writers,
            vec![
                (0x0400, 2, vec!["NR12", "NR14"]),
                (0x0500, 2, vec!["WAVE"]),
                (0x1595, 2, vec!["NR42"]),
                (0x0600, 1, vec!["NR50"]),
                (0x1598, 1, vec!["NR44"]),
            ]
        );
    }
}
