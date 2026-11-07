//! The decoded write log of the tune ROM, checked against a hand-written
//! expectation (criterion qvV). The pitches were computed by hand from the
//! register values: a square's frequency is 131072 / (2048 - period) Hz, the
//! wave channel's 65536 / (2048 - period) Hz, the nearest note and cents from
//! A4 = 440 Hz (equal temperament).

mod common;

use common::*;
use gbcli_audio::{ApuSnapshot, Change, NoteName, decode};
use gbcli_core::{ApuChannel, Budget, Core};

fn tune_log() -> Vec<gbcli_audio::DecodedWrite> {
    let mut core = tune_core();
    let snapshot = ApuSnapshot::from_core(&core).unwrap();
    core.set_apu_write_log(true).unwrap();
    run(&mut core, Budget::frames(TUNE_FRAMES));
    decode(&snapshot, &core.take_apu_writes().unwrap())
}

#[test]
fn the_tune_decodes_to_the_hand_checked_log() {
    let mut expected = vec![
        "NR52 $80: APU on".to_string(),
        "NR50 $77: volume left 7, right 7".into(),
        "NR51 $FF: panning left 1234, right 1234".into(),
    ];
    let wave = [
        0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0xFE, 0xDC, 0xBA, 0x98, 0x76, 0x54, 0x32,
        0x10,
    ];
    for (i, byte) in wave.iter().enumerate() {
        expected.push(format!(
            "WAVE{i:X} ${byte:02X}: wave RAM samples {}, {}",
            byte >> 4,
            byte & 15
        ));
    }
    expected.extend(
        [
            "NR10 $00: sweep off",
            "NR11 $80: square 1 duty 50%, length 64",
            "NR12 $F3: square 1 envelope 15 down, pace 3",
            "NR13 $D6: square 1 period 214 = D2 -46.6c (71.47 Hz)",
            "NR14 $86: square 1 trigger, length off, period 1750 = A4 -0.6c (439.84 Hz)",
            "NR21 $60: square 2 duty 25%, length 32",
            "NR22 $A0: square 2 envelope 10 steady",
            "NR23 $72: square 2 period 114 = C#2 -38.5c (67.77 Hz)",
            "NR24 $C6: square 2 trigger, length on, period 1650 = E4 -1.6c (329.33 Hz)",
            "NR30 $80: wave DAC on",
            "NR31 $00: wave length 256",
            "NR32 $20: wave level 100%",
            "NR33 $B2: wave period 178 = C#1 +19.8c (35.05 Hz)",
            "NR34 $86: wave trigger, length off, period 1714 = G3 +1.9c (196.22 Hz)",
            "NR10 $17: sweep up, pace 1, shift 7",
            "NR13 $06: square 1 period 1542 = C4 -17.2c (259.04 Hz)",
            "NR14 $87: square 1 trigger, length off, period 1798 = C5 +3.4c (524.29 Hz)",
            "NR42 $A1: noise envelope 10 down, pace 1",
            "NR43 $52: noise shift 5, 15-bit, divisor 2 = 4096 Hz",
            "NR44 $80: noise trigger, length off",
            "NR30 $00: wave DAC off",
            "NR52 $00: APU off",
            "NR12 $F0: square 1 envelope 15 steady (ignored: the APU is off)",
            "NR11 $BF: square 1 length 1 (the APU is off: a DMG takes only the length)",
        ]
        .map(String::from),
    );
    let actual: Vec<String> = tune_log().iter().map(|w| w.to_string()).collect();
    assert_eq!(actual, expected);
}

#[test]
fn a_trigger_carries_the_note_as_typed_fields() {
    let log = tune_log();
    let a4 = log.iter().find(|w| w.write.addr == 0xFF14).unwrap();
    let Change::Control {
        channel,
        trigger,
        pitch: Some(pitch),
        ..
    } = a4.change
    else {
        panic!("NR14 decoded as {:?}", a4.change);
    };
    assert_eq!((channel, trigger), (ApuChannel::Square1, true));
    assert_eq!(pitch.period, 1750);
    assert_eq!((pitch.note.name, pitch.note.octave), (NoteName::A, 4));
    assert!((pitch.hz - 131072.0 / 298.0).abs() < 1e-9);
    assert!((pitch.cents - -0.63).abs() < 0.01, "{}", pitch.cents);
}

#[test]
fn the_log_is_timed_by_the_frame_the_tune_waits_for() {
    let log = tune_log();
    let at = |addr: u16, value: u8| {
        log.iter()
            .find(|w| w.write.addr == addr && w.write.value == value)
            .unwrap()
            .write
            .at
    };
    // NR14 $86 at frame 0 and $87 after 15 vblanks: 15 frames apart, to
    // within the few instructions between a vblank and the write.
    let gap = (at(0xFF14, 0x87) - at(0xFF14, 0x86)).0;
    let frame = gbcli_core::Ticks::PER_FRAME;
    assert!(gap > 14 * frame && gap < 15 * frame + frame / 10, "{gap}");
}

#[test]
fn a_snapshot_holds_the_bits_last_written_and_the_power_bit() {
    // NR13 and NR23 are write-only (they read back as $FF), so decoding a
    // period needs the stored byte, not a bus read.
    let mut core = tune_core();
    core.bus_write(0xFF26, 0x80).unwrap();
    core.bus_write(0xFF13, 0xD6).unwrap();
    core.bus_write(0xFF14, 0x05).unwrap();
    let on = ApuSnapshot::from_core(&core).unwrap();
    assert!(on.powered);
    assert_eq!((on.regs[0x03], on.regs[0x04] & 7), (0xD6, 5));
    core.bus_write(0xFF26, 0x00).unwrap();
    assert!(!ApuSnapshot::from_core(&core).unwrap().powered);
}
