//! The sound-register write log and the sample stream.

mod common;

use common::*;
use gbcli_core::{ApuChannel, ApuWriteEffect, AudioCapture, Budget, Core, Feature, Support, Ticks};

const TUNE: &[u8] = include_bytes!("../../audio/tests/roms/tune.gb");

#[test]
fn sameboy_logs_apu_writes_and_renders_samples() {
    let core = quick_dmg(TUNE);
    assert_eq!(core.supports(Feature::ApuWriteLog), Support::Yes);
    assert_eq!(core.supports(Feature::AudioSamples), Support::Yes);
}

#[test]
fn the_log_records_each_sound_write_once_in_order_and_nothing_else() {
    let mut core = quick_dmg(TUNE);
    core.set_apu_write_log(true).unwrap();
    let start = core.now();
    run(&mut core, Budget::frames(35));
    let writes = core.take_apu_writes().unwrap();
    // tune.asm: 3 setup, 16 wave RAM, 14 at frame 0, 6 at frame 15, 4 at frame 30.
    assert_eq!(writes.len(), 3 + 16 + 14 + 6 + 4);
    assert!(writes.iter().all(|w| gbcli_core::is_sound_addr(w.addr)));
    assert!(writes.windows(2).all(|p| p[0].at < p[1].at));
    assert!(writes.iter().all(|w| w.at > start && w.at <= core.now()));
    assert_eq!((writes[0].addr, writes[0].value), (0xFF26, 0x80));
    assert!(core.take_apu_writes().unwrap().is_empty(), "taking drains");
}

#[test]
fn writes_while_the_apu_is_off_are_marked_ignored_except_dmg_length() {
    let mut core = quick_dmg(TUNE);
    core.set_apu_write_log(true).unwrap();
    run(&mut core, Budget::frames(35));
    let writes = core.take_apu_writes().unwrap();
    let tail: Vec<_> = writes[writes.len() - 4..]
        .iter()
        .map(|w| (w.addr, w.effect))
        .collect();
    assert_eq!(
        tail,
        [
            (0xFF1A, ApuWriteEffect::Applied),
            (0xFF26, ApuWriteEffect::Applied),
            (0xFF12, ApuWriteEffect::IgnoredApuOff),
            (0xFF11, ApuWriteEffect::Applied),
        ]
    );
}

#[test]
fn a_write_is_stamped_inside_its_instruction() {
    let mut core = quick_dmg(TUNE);
    core.set_apu_write_log(true).unwrap();
    // Step until the first sound write; it must fall between the instruction's
    // start and end, after its opcode and operand fetches.
    loop {
        let before = core.now();
        let pc = core.registers().pc;
        run(&mut core, Budget::instructions(1));
        let writes = core.take_apu_writes().unwrap();
        if let Some(w) = writes.first() {
            assert_eq!(w.pc, pc);
            assert!(
                w.at > before && w.at <= core.now(),
                "{w:?} in {before:?}..{:?}",
                core.now()
            );
            break;
        }
    }
}

#[test]
fn a_bus_write_is_logged_at_the_current_time() {
    let mut core = quick_dmg(TUNE);
    core.set_apu_write_log(true).unwrap();
    run(&mut core, Budget::frames(1));
    core.bus_write(0xFF24, 0x33).unwrap();
    let last = *core.take_apu_writes().unwrap().last().unwrap();
    assert_eq!((last.addr, last.value, last.at), (0xFF24, 0x33, core.now()));
}

#[test]
fn the_log_is_off_by_default_and_turning_it_off_drops_it() {
    let mut core = quick_dmg(TUNE);
    run(&mut core, Budget::frames(2));
    assert!(core.take_apu_writes().unwrap().is_empty());
    core.set_apu_write_log(true).unwrap();
    run(&mut core, Budget::frames(1));
    core.set_apu_write_log(false).unwrap();
    assert!(core.take_apu_writes().unwrap().is_empty());
}

#[test]
fn a_load_drops_the_pending_writes_and_samples() {
    let mut core = quick_dmg(TUNE);
    let state = core.save();
    core.set_apu_write_log(true).unwrap();
    core.set_audio_capture(Some(AudioCapture::MIXED)).unwrap();
    run(&mut core, Budget::frames(2));
    core.load(&state).unwrap();
    assert!(core.take_apu_writes().unwrap().is_empty());
    assert_eq!(core.take_audio().unwrap().samples.len(), 0);
}

#[test]
fn capture_renders_one_sample_every_256_ticks() {
    let mut core = quick_dmg(TUNE);
    core.set_audio_capture(Some(AudioCapture::MIXED)).unwrap();
    let start = core.now();
    run(&mut core, Budget::frames(20));
    let audio = core.take_audio().unwrap();
    let elapsed = (core.now() - start).0;
    let expected = elapsed / AudioCapture::TICKS_PER_SAMPLE;
    assert!(
        audio.samples.len() as u64 + 1 >= expected && audio.samples.len() as u64 <= expected,
        "{} samples in {elapsed} ticks",
        audio.samples.len()
    );
    let first = audio.first.unwrap();
    assert!(first > start && first <= start + Ticks(2 * AudioCapture::TICKS_PER_SAMPLE));
    assert!(
        audio
            .samples
            .iter()
            .any(|s| s.left != audio.samples[0].left)
    );
}

#[test]
fn capture_does_not_change_what_the_machine_does() {
    let mut plain = quick_dmg(TUNE);
    let mut captured = quick_dmg(TUNE);
    plain.set_apu_write_log(true).unwrap();
    captured.set_apu_write_log(true).unwrap();
    captured
        .set_audio_capture(Some(AudioCapture::solo(ApuChannel::Wave)))
        .unwrap();
    run(&mut plain, Budget::frames(35));
    run(&mut captured, Budget::frames(35));
    assert_eq!(plain.now(), captured.now());
    assert_eq!(plain.registers(), captured.registers());
    assert_eq!(
        plain.take_apu_writes().unwrap(),
        captured.take_apu_writes().unwrap()
    );
    assert_eq!(
        plain.apu().unwrap().channels,
        captured.apu().unwrap().channels
    );
    // A save catches the batched APU cycles up, so the bytes agree too.
    assert!(plain.save() == captured.save(), "states differ");
}

#[test]
fn the_same_state_renders_the_same_samples_whatever_came_before() {
    let mut a = quick_dmg(TUNE);
    run(&mut a, Budget::frames(10));
    let state = a.save();
    let mut b = quick_dmg(TUNE);
    b.set_audio_capture(Some(AudioCapture::MIXED)).unwrap();
    run(&mut b, Budget::frames(17)); // a different history in b's renderer
    b.load(&state).unwrap();
    a.set_audio_capture(Some(AudioCapture::MIXED)).unwrap();
    run(&mut a, Budget::frames(10));
    run(&mut b, Budget::frames(10));
    assert_eq!(a.take_audio().unwrap(), b.take_audio().unwrap());
}

#[test]
fn soloed_channels_sum_to_the_mix_exactly() {
    let start = {
        let mut core = quick_dmg(TUNE);
        run(&mut core, Budget::frames(1));
        core.save()
    };
    let render = |capture: AudioCapture| {
        let mut core = quick_dmg(TUNE);
        core.load(&start).unwrap();
        core.set_audio_capture(Some(capture)).unwrap();
        run(&mut core, Budget::frames(32));
        core.take_audio().unwrap()
    };
    let mixed = render(AudioCapture::MIXED);
    let solos: Vec<_> = ApuChannel::ALL
        .iter()
        .map(|&c| render(AudioCapture::solo(c)))
        .collect();
    for solo in &solos {
        assert_eq!(solo.first, mixed.first);
        assert_eq!(solo.samples.len(), mixed.samples.len());
    }
    for (i, m) in mixed.samples.iter().enumerate() {
        let left: i32 = solos.iter().map(|s| i32::from(s.samples[i].left)).sum();
        let right: i32 = solos.iter().map(|s| i32::from(s.samples[i].right)).sum();
        assert_eq!(
            (left, right),
            (i32::from(m.left), i32::from(m.right)),
            "sample {i}"
        );
    }
}

#[test]
fn wave_ram_written_while_channel_3_plays_is_marked() {
    let mut core = quick_dmg(TUNE);
    core.set_apu_write_log(true).unwrap();
    core.bus_write(0xFF26, 0x80).unwrap(); // APU on
    core.bus_write(0xFF30, 0x12).unwrap(); // wave RAM, channel 3 idle
    core.bus_write(0xFF1A, 0x80).unwrap(); // wave DAC on
    core.bus_write(0xFF1E, 0x87).unwrap(); // trigger
    core.bus_write(0xFF31, 0x34).unwrap(); // wave RAM while it plays
    let effects: Vec<_> = core
        .take_apu_writes()
        .unwrap()
        .iter()
        .map(|w| (w.addr, w.effect))
        .collect();
    assert_eq!(effects[1], (0xFF30, ApuWriteEffect::Applied));
    assert_eq!(effects[4], (0xFF31, ApuWriteEffect::WaveRamWhilePlaying));
}

#[test]
fn a_cgb_ignores_length_writes_while_the_apu_is_off() {
    let mut core = quick_boot(gbcli_core::CoreConfig::new(
        gbcli_core::Model::CgbE,
        TUNE.to_vec(),
    ));
    core.set_apu_write_log(true).unwrap();
    core.bus_write(0xFF26, 0x00).unwrap();
    core.bus_write(0xFF11, 0x3F).unwrap();
    let last = core.take_apu_writes().unwrap()[1];
    assert_eq!(last.effect, ApuWriteEffect::IgnoredApuOff);
}

#[test]
fn capture_is_refused_on_the_sgb() {
    let mut core = gbcli_sameboy::SameBoy::new(gbcli_core::CoreConfig::new(
        gbcli_core::Model::Sgb,
        TUNE.to_vec(),
    ))
    .unwrap();
    assert!(matches!(
        core.supports(Feature::AudioSamples),
        Support::No { .. }
    ));
    assert!(core.set_audio_capture(Some(AudioCapture::MIXED)).is_err());
}

#[test]
fn a_save_is_the_same_bytes_with_or_without_capture() {
    // Observation must not change the head: SameBoy batches APU cycles, and
    // rendering makes it catch up more often, so a save must not depend on it.
    for frames in [1, 7, 35] {
        let mut plain = quick_dmg(TUNE);
        let mut captured = quick_dmg(TUNE);
        captured
            .set_audio_capture(Some(AudioCapture::MIXED))
            .unwrap();
        run(&mut plain, Budget::frames(frames));
        run(&mut captured, Budget::frames(frames));
        assert!(
            plain.save() == captured.save(),
            "saves differ after {frames} frames"
        );
    }
}

#[test]
fn saving_during_a_capture_does_not_change_what_it_renders() {
    let mut straight = quick_dmg(TUNE);
    let mut saved = quick_dmg(TUNE);
    straight
        .set_audio_capture(Some(AudioCapture::MIXED))
        .unwrap();
    saved.set_audio_capture(Some(AudioCapture::MIXED)).unwrap();
    run(&mut straight, Budget::frames(20));
    for _ in 0..20 {
        run(&mut saved, Budget::frames(1));
        saved.save();
    }
    assert_eq!(straight.now(), saved.now());
    assert_eq!(straight.take_audio().unwrap(), saved.take_audio().unwrap());
    assert!(straight.save() == saved.save());
}

#[test]
fn capture_renders_the_same_whatever_the_apu_had_batched() {
    // Two cores at the same moment, one with its batched APU cycles caught up
    // (a save does that) and one not: capture must start from the moment,
    // not from wherever the APU last caught up.
    let mut batched = quick_dmg(TUNE);
    let mut synced = quick_dmg(TUNE);
    assert_eq!(batched.apu().unwrap().sync, gbcli_core::ApuSync::Lagging);
    synced.save();
    assert_eq!(synced.apu().unwrap().sync, gbcli_core::ApuSync::Current);
    for core in [&mut batched, &mut synced] {
        core.set_audio_capture(Some(AudioCapture::MIXED)).unwrap();
        run(core, Budget::frames(20));
    }
    assert_eq!(batched.take_audio().unwrap(), synced.take_audio().unwrap());
}

#[test]
fn a_save_renders_no_sample() {
    // SameBoy earns a sample's cycles in timers_run and renders it in the
    // GB_apu_run that ends the same GB_advance_cycles, so no run ends owing
    // one and the sync in `save` never renders. Were that to change, such a
    // sample would need stamping at the save, not the last step's start.
    // Each try starts from one state, so no earlier save caught the APU up.
    let mut core = quick_dmg(TUNE);
    core.set_audio_capture(Some(AudioCapture::MIXED)).unwrap();
    let start = core.save();
    for n in 1..2000 {
        core.load(&start).unwrap();
        run(&mut core, Budget::instructions(n));
        core.take_audio().unwrap();
        core.save();
        assert!(
            core.take_audio().unwrap().samples.is_empty(),
            "after {n} instructions"
        );
    }
}
