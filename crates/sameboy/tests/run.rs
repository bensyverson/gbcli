//! Advancing: budgets, steps, emulated time, illegal opcodes, STOP.

mod common;

use common::*;
use gbcli_core::{
    AddrFilter, BankSel, Budget, BudgetKind, Core, CoreConfig, CoreError, Feature, InStop, IoReg,
    JoypadBounce, Keys, Model, Region, RunRequest, StepUnit, Stop, StopReason, Support, Ticks,
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
        Feature::PpuModeEvents,
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

/// Runs with `in_stop` set.
fn run_in_stop(core: &mut dyn Core, budget: Budget, in_stop: InStop) -> Stop {
    let mut req = RunRequest::new(budget);
    req.in_stop = in_stop;
    core.run(&mut req).expect("run")
}

// The STOP contract changed (design §2, leaf 34Nx7n): a run now lets time
// pass in STOP, as the hardware and SameBoy's own frontend do, so a cartridge
// RTC counts through it. Ending at once with `Stopped` is kept as
// `InStop::End`. The tests that pinned "no time passes in STOP" now ask for
// `End` explicitly.

#[test]
fn time_passes_in_stop_until_the_budget_ends() {
    let mut core = quick_dmg(STOP);
    let stop = run(&mut core, Budget::frames(10));
    assert_eq!(stop.reason, StopReason::Budget(BudgetKind::Frames));
    assert_eq!(stop.used.frames, 10);
    let frames = stop.used.ticks as f64 / Ticks::PER_FRAME as f64;
    assert!((9.0..11.0).contains(&frames), "{frames} frames of ticks");
    // The CPU never left STOP.
    assert_eq!(core.region(Region::Wram).unwrap().bytes[0], 0x00);
    let again = run(&mut core, Budget::ticks(1_000));
    assert_eq!(again.reason, StopReason::Budget(BudgetKind::Ticks));
    assert!(again.used.ticks >= 1_000);
    assert_eq!(again.pc, stop.pc);
}

#[test]
fn ending_in_stop_stops_with_no_wake_source() {
    let mut core = quick_dmg(STOP);
    let stop = run_in_stop(&mut core, Budget::frames(10), InStop::End);
    assert_eq!(stop.reason, StopReason::Stopped);
    assert!(stop.used.frames < 10);
    assert_eq!(core.region(Region::Wram).unwrap().bytes[0], 0x00);
}

/// Runs `stop_wake.gb` into STOP, presses `keys`, and runs again.
fn press_while_stopped(mut core: SameBoy, keys: Keys) -> (SameBoy, Stop) {
    let stop = run_in_stop(&mut core, Budget::frames(10), InStop::End);
    assert_eq!(stop.reason, StopReason::Stopped);
    assert_eq!(core.region(Region::Wram).unwrap().bytes[0], 0x00);
    core.set_keys(keys);
    let woke = run_in_stop(&mut core, Budget::frames(2), InStop::End);
    (core, woke)
}

#[test]
fn a_key_in_the_selected_row_wakes_a_stopped_core() {
    let (core, woke) = press_while_stopped(quick_dmg(STOP_WAKE), Keys::UP);
    assert_eq!(woke.reason, StopReason::Budget(BudgetKind::Frames));
    assert_eq!(core.region(Region::Wram).unwrap().bytes[0], 0x99);
}

#[test]
fn a_key_pressed_while_time_passes_in_stop_wakes_the_core() {
    let mut core = quick_dmg(STOP_WAKE);
    let parked = run(&mut core, Budget::frames(3));
    assert_eq!(parked.reason, StopReason::Budget(BudgetKind::Frames));
    assert_eq!(core.region(Region::Wram).unwrap().bytes[0], 0x00);
    core.set_keys(Keys::UP);
    run(&mut core, Budget::frames(1));
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
    // With the keys settled, nothing can wake it: ending in STOP takes no
    // time, and passing time leaves it stopped.
    let again = run_in_stop(&mut core, Budget::frames(2), InStop::End);
    assert_eq!(again.reason, StopReason::Stopped);
    assert_eq!(again.used.ticks, 0);
    let passed = run(&mut core, Budget::frames(2));
    assert_eq!(passed.reason, StopReason::Budget(BudgetKind::Frames));
    assert_eq!(core.region(Region::Wram).unwrap().bytes[0], 0x00);
}

/// An execution filter on the instruction after STOP, which SameBoy's PC
/// already points at while the CPU sits in STOP.
fn after_stop() -> AddrFilter {
    let mut filter = AddrFilter::new();
    filter.insert(0x015D, BankSel::Any);
    filter
}

#[test]
fn a_breakpoint_after_stop_waits_while_time_passes_in_stop() {
    let mut core = quick_dmg(STOP_WAKE);
    let filter = after_stop();
    let mut req = RunRequest::new(Budget::frames(2));
    req.exec = &filter;
    let parked = core.run(&mut req).unwrap();
    assert_eq!(parked.reason, StopReason::Budget(BudgetKind::Frames));
    core.set_keys(Keys::UP);
    let mut req = RunRequest::new(Budget::frames(2));
    req.exec = &filter;
    let woke = core.run(&mut req).unwrap();
    assert!(
        matches!(woke.reason, StopReason::Candidate(_)),
        "{:?}",
        woke.reason
    );
    assert_eq!(woke.pc, 0x015D);
    // It stopped before the instruction ran.
    assert_eq!(core.region(Region::Wram).unwrap().bytes[0], 0x00);
}

#[test]
fn a_breakpoint_after_halt_waits_while_the_cpu_is_halted() {
    // halt.gb: HALT at $0154 with IE = 0, so nothing wakes it; SameBoy's PC
    // already points at the NOP after it.
    let mut core = quick_dmg(include_bytes!("../../engine/tests/roms/halt.gb"));
    let mut filter = AddrFilter::new();
    filter.insert(0x0155, BankSel::Any);
    let mut req = RunRequest::new(Budget::frames(2));
    req.exec = &filter;
    let stop = core.run(&mut req).unwrap();
    assert_eq!(stop.reason, StopReason::Budget(BudgetKind::Frames));
    assert!(core.registers().halted);
}
