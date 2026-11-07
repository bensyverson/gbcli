//! Advancing: budgets, steps, emulated time, illegal opcodes, STOP.

mod common;

use common::*;
use gbcli_core::{
    Budget, BudgetKind, Core, CoreConfig, CoreError, Feature, IoReg, JoypadBounce, Keys, Model,
    Region, RunRequest, StepUnit, Stop, StopReason, Support, Ticks,
};
use gbcli_sameboy::SameBoy;

#[test]
fn info_names_the_backend_its_commit_patch_set_and_model() {
    let core = quick_dmg(BUSY);
    let info = core.info();
    assert_eq!(info.backend, "sameboy");
    assert_eq!(info.version, sameboy_sys::CORE_VERSION);
    assert!(info.version.starts_with(sameboy_sys::SAMEBOY_COMMIT));
    assert!(info.version.ends_with(sameboy_sys::PATCH_SET));
    assert_eq!(info.model, Model::DmgB);
}

#[test]
fn a_rom_too_short_for_a_header_is_refused() {
    let err = SameBoy::new(CoreConfig::new(Model::DmgB, vec![0; 0x100])).err();
    assert!(
        matches!(err, Some(CoreError::InvalidState { .. })),
        "{err:?}"
    );
}

#[test]
fn time_starts_at_zero_and_counts_every_tick_run() {
    let mut core = dmg(BUSY, 0);
    assert_eq!(core.now(), Ticks(0));
    let stop = run(&mut core, Budget::ticks(1_000));
    assert_eq!(stop.reason, StopReason::Budget(BudgetKind::Ticks));
    assert!(stop.used.ticks >= 1_000 && stop.used.ticks < 1_000 + 100);
    assert_eq!(core.now(), Ticks(stop.used.ticks));
    assert_eq!(stop.at, core.now());
    let again = run(&mut core, Budget::ticks(500));
    assert_eq!(core.now(), Ticks(stop.used.ticks + again.used.ticks));
}

#[test]
fn a_zero_budget_returns_without_running() {
    let mut core = quick_dmg(BUSY);
    let before = core.now();
    let stop = run(&mut core, Budget::instructions(0));
    assert_eq!(stop.reason, StopReason::Budget(BudgetKind::Instructions));
    assert_eq!(core.now(), before);
}

#[test]
fn an_instruction_budget_counts_cpu_steps() {
    let mut core = quick_dmg(BUSY);
    let stop = run(&mut core, Budget::instructions(10));
    assert_eq!(stop.reason, StopReason::Budget(BudgetKind::Instructions));
    assert_eq!(stop.used.instructions, 10);
    assert_eq!(stop.pc, core.registers().pc);
}

#[test]
fn a_frame_budget_ends_after_that_many_vblanks() {
    let mut core = quick_dmg(BUSY);
    run(&mut core, Budget::frames(1)); // align to a vblank
    let stop = run(&mut core, Budget::frames(3));
    assert_eq!(stop.reason, StopReason::Budget(BudgetKind::Frames));
    assert_eq!(stop.used.frames, 3);
    let frames = stop.used.ticks as f64 / Ticks::PER_FRAME as f64;
    assert!((frames - 3.0).abs() < 0.01, "{frames} frames of ticks");
}

#[test]
fn stop_at_frame_stops_after_the_next_vblank() {
    let mut core = quick_dmg(BUSY);
    let mut req = RunRequest::new(Budget::frames(10));
    req.stop_at_frame = true;
    let stop = core.run(&mut req).unwrap();
    assert_eq!(stop.reason, StopReason::Frame);
    assert_eq!(stop.used.frames, 1);
}

#[test]
fn a_step_runs_one_instruction() {
    let mut core = quick_dmg(BUSY);
    let stop = core.step(StepUnit::Instruction).unwrap();
    assert_eq!(stop.reason, StopReason::Step(StepUnit::Instruction));
    assert_eq!(stop.used.instructions, 1);
    assert_eq!(core.registers().pc, 0x0101); // the header's nop
    assert_eq!(stop.used.ticks, 8); // 4 T-cycles at single speed
}

#[test]
fn dot_and_m_cycle_steps_are_unsupported() {
    let mut core = quick_dmg(BUSY);
    for unit in [StepUnit::Dot, StepUnit::MCycle] {
        let err = core.step(unit).unwrap_err();
        assert!(matches!(err, CoreError::Unsupported { .. }), "{err:?}");
    }
}

#[test]
fn unsupported_features_say_no_with_a_reason() {
    let core = quick_dmg(BUSY);
    for feature in [
        Feature::DotStep,
        Feature::MCycleStep,
        Feature::PpuDot,
        Feature::WriteHookCoversDma,
        Feature::ExecHookCoversInterrupts,
        Feature::SideEffectFreeIo,
        Feature::ExtraOam,
        Feature::ApuInternals,
        Feature::Mmm01RomPatch,
        Feature::EventSinks,
    ] {
        assert!(
            matches!(core.supports(feature), Support::No { reason } if !reason.is_empty()),
            "{feature:?}"
        );
    }
}

#[test]
fn pixel_layer_capture_is_off_until_asked_for() {
    let mut core = quick_dmg(BUSY);
    assert!(matches!(core.supports(Feature::PixelLayers), Support::Yes));
    run(&mut core, Budget::frames(2));
    assert!(matches!(
        core.pixel_layers(),
        Err(CoreError::InvalidState { .. })
    ));
    core.set_pixel_layers(true).unwrap();
    run(&mut core, Budget::frames(2));
    let map = core.pixel_layers().unwrap();
    assert_eq!((map.width, map.height), (160, 144));
    assert_eq!(map.layers.len(), 160 * 144);
    core.set_pixel_layers(false).unwrap();
    assert!(matches!(
        core.pixel_layers(),
        Err(CoreError::InvalidState { .. })
    ));
}

#[test]
fn the_builtin_dmg_boot_rom_hands_over_after_80_to_86_frames() {
    let mut core = dmg(BUSY, 0);
    let mut frames = 0;
    while core.io_register(IoReg::BANK).unwrap() & 1 == 0 {
        assert!(frames < 120, "no handover after {frames} frames");
        run(&mut core, Budget::frames(1));
        frames += 1;
    }
    assert!(
        (80..=86).contains(&frames),
        "handover after {frames} frames"
    );
}

#[test]
fn a_cgb_model_boots_its_builtin_boot_rom() {
    let mut core = SameBoy::new(CoreConfig::new(Model::CgbE, BUSY.to_vec())).unwrap();
    assert_eq!(core.info().model, Model::CgbE);
    assert_eq!(core.region(Region::Wram).unwrap().bytes.len(), 0x8000);
    run(&mut core, Budget::frames(400));
    assert_eq!(core.io_register(IoReg::BANK).unwrap() & 1, 1);
}

#[test]
fn an_illegal_opcode_stops_before_the_cpu_locks() {
    let mut core = quick_dmg(ILLEGAL);
    let stop = run(&mut core, Budget::frames(5));
    assert_eq!(stop.reason, StopReason::Locked { opcode: 0xD3 });
    assert_eq!(stop.pc, 0x0156);
    // SameBoy's lock clears IE and halts; neither happened.
    assert_eq!(core.region(Region::Ie).unwrap().bytes, [0x01]);
    assert!(!core.registers().halted);
    // The CPU can't get past it, so the next run stops in the same place.
    let again = run(&mut core, Budget::frames(5));
    assert_eq!(again.reason, StopReason::Locked { opcode: 0xD3 });
    assert_eq!(again.used.instructions, 0);
}

#[test]
fn stop_with_no_wake_source_ends_the_run() {
    let mut core = quick_dmg(STOP);
    let stop = run(&mut core, Budget::frames(10));
    assert_eq!(stop.reason, StopReason::Stopped);
    assert!(stop.used.frames < 10);
    assert_eq!(core.region(Region::Wram).unwrap().bytes[0], 0x00);
}

/// Runs `stop_wake.gb` into STOP, presses `keys`, and runs again.
fn press_while_stopped(mut core: SameBoy, keys: Keys) -> (SameBoy, Stop) {
    let stop = run(&mut core, Budget::frames(10));
    assert_eq!(stop.reason, StopReason::Stopped);
    assert_eq!(core.region(Region::Wram).unwrap().bytes[0], 0x00);
    core.set_keys(keys);
    let woke = run(&mut core, Budget::frames(2));
    (core, woke)
}

#[test]
fn a_key_in_the_selected_row_wakes_a_stopped_core() {
    let (core, woke) = press_while_stopped(quick_dmg(STOP_WAKE), Keys::UP);
    assert_eq!(woke.reason, StopReason::Budget(BudgetKind::Frames));
    assert_eq!(core.region(Region::Wram).unwrap().bytes[0], 0x99);
}

#[test]
fn a_bouncing_key_in_the_selected_row_wakes_a_stopped_core() {
    let mut config = CoreConfig::new(Model::DmgB, STOP_WAKE.to_vec());
    config.joypad_bounce = JoypadBounce::On;
    let (core, woke) = press_while_stopped(quick_boot(config), Keys::UP);
    assert_eq!(woke.reason, StopReason::Budget(BudgetKind::Frames));
    assert_eq!(core.region(Region::Wram).unwrap().bytes[0], 0x99);
}

#[test]
fn a_key_outside_the_selected_row_lets_time_pass_but_stays_stopped() {
    let (mut core, still) = press_while_stopped(quick_dmg(STOP_WAKE), Keys::A);
    assert_eq!(still.reason, StopReason::Stopped);
    // The CPU steps once in STOP to see the new keys, as on hardware.
    assert!(still.used.ticks > 0);
    assert_eq!(core.region(Region::Wram).unwrap().bytes[0], 0x00);
    // With the keys settled, nothing can wake it and no time passes.
    let again = run(&mut core, Budget::frames(2));
    assert_eq!(again.reason, StopReason::Stopped);
    assert_eq!(again.used.ticks, 0);
}
