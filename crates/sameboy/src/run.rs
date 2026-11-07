//! The run loop: budgets, filters, frame stops, the illegal-opcode and STOP checks.
//!
//! In STOP, a run either steps through it, so emulated time passes as on
//! hardware (`InStop::PassTime`), or ends before the step when no key can
//! wake the CPU (`InStop::End`).
//!
//! SameBoy locks the CPU on an illegal opcode (IE=0, halted) without telling
//! anyone, so the loop checks the opcode the next step will execute before
//! running it (`gbcli_pre_run` in the shim). Execution filters are checked the
//! same way, so a breakpoint stops before its instruction runs; read and write
//! filters are checked in SameBoy's memory callbacks (`watch.rs`) and stop
//! after the instruction that made the access.

use gbcli_core::{
    Access, CoreError, CpuStep, Event, EventKinds, EventSink, Feature, FirstStep, FrameTag, Hit,
    Hits, InStop, LcdOff, RunRequest, Stop, StopReason, Ticks, Usage,
};
use sameboy_sys as sb;

use crate::hooks::{Hooks, Wake, on_exec, vblank_kind};
use crate::raster;
use crate::watch::{self, Armed};
use crate::{SameBoy, unsupported_err};

/// Arms the memory callbacks for one run and disarms them when dropped, on
/// every exit path, so no callback can see the request's filters after it ends.
struct Arming {
    gb: *mut sb::GB_gameboy_t,
    hooks: *mut Hooks,
    read: bool,
    write: bool,
    exec: bool,
    raster: bool,
}

impl Arming {
    fn new(core: &mut SameBoy, req: &RunRequest<'_>, kinds: EventKinds) -> Arming {
        let (exec, raster) = (kinds.exec, kinds.raster);
        let gb = core.raw();
        let read = !req.read.is_empty();
        let write = !(req.write.is_empty() && req.veto.is_empty() && req.stores.is_empty());
        let hooks = core.hooks_mut();
        hooks.in_run = true;
        if raster {
            hooks.raster = Some(Vec::new());
            hooks.lcd_turning_off = false;
        }
        if read || write {
            hooks.armed = Some(Armed {
                read: req.read,
                write: req.write,
                veto: req.veto,
                stores: req.stores,
            });
        }
        // SAFETY: installs our callbacks on the live instance outside GB_run.
        unsafe {
            if read {
                sb::GB_set_read_memory_callback(gb, Some(watch::on_read));
            }
            if write || raster {
                sb::GB_set_write_memory_callback(gb, Some(watch::on_write));
            }
            if raster {
                sb::GB_set_lcd_line_callback(gb, Some(raster::on_line));
            }
            if exec {
                sb::GB_set_execution_callback(gb, Some(on_exec));
            }
        }
        Arming {
            gb,
            hooks,
            read,
            write,
            exec,
            raster,
        }
    }

    fn armed(&self) -> bool {
        self.read || self.write
    }
}

impl Drop for Arming {
    fn drop(&mut self) {
        // SAFETY: the hooks and instance outlive the run; no GB_run is in progress.
        unsafe {
            let hooks = &mut *self.hooks;
            hooks.armed = None;
            hooks.hit = None;
            hooks.in_run = false;
            if self.read {
                sb::GB_set_read_memory_callback(self.gb, None);
            }
            if (self.write || self.raster) && !hooks.write_hook_wanted() {
                sb::GB_set_write_memory_callback(self.gb, None);
            }
            if self.exec {
                hooks.executed = None;
                hooks.wake = None;
                sb::GB_set_execution_callback(self.gb, None);
            }
            if self.raster {
                hooks.raster = None;
                sb::GB_set_lcd_line_callback(self.gb, None);
            }
        }
    }
}

pub(crate) fn run(core: &mut SameBoy, req: &mut RunRequest<'_>) -> Result<Stop, CoreError> {
    let kinds = req
        .sink
        .as_ref()
        .map(|sink| sink.kinds())
        .unwrap_or_default();
    if kinds.ppu_modes {
        return Err(unsupported_err(Feature::PpuModeEvents));
    }
    let exec_events = kinds.exec;
    let arming = Arming::new(core, req, kinds);
    let check_exec = !req.exec.is_empty();
    let tapping = kinds.raster || core.hooks().apu_log.is_some() || core.hooks().audio.is_some();
    let gb = core.raw();
    let regs = core.regs;
    let mut used = Usage::default();
    loop {
        if let Some(kind) = req.budget.exhausted(&used) {
            return Ok(stop(core, StopReason::Budget(kind), used));
        }
        let mut opcode = 0;
        // SAFETY: a side-effect-free check on a live instance, outside GB_run.
        match unsafe { sb::gbcli_pre_run(gb, &mut opcode) } {
            sb::GBCLI_PRE_ILLEGAL => {
                return Ok(stop(core, StopReason::Locked { opcode }, used));
            }
            // Passing time, GB_run takes a STOP step (four T-cycles), as
            // SameBoy's own frontend does.
            sb::GBCLI_PRE_STOPPED if req.in_stop == InStop::End => {
                return Ok(stop(core, StopReason::Stopped, used));
            }
            _ => {}
        }
        // SAFETY: `regs` points into the live instance.
        let pc = unsafe { (*regs).pc };
        // By default a run never stops on an execution hit before its first
        // step, so resuming from a breakpoint makes progress. In HALT or
        // STOP, PC already points past it, but the next step is an idle tick
        // or a wake that doesn't run that instruction, except at the end of a
        // CGB speed switch's pause: there the step that ends the pause runs
        // it too (`GB_cpu_run`), so the hit is due before that step.
        if check_exec
            && (used.instructions > 0 || req.first_step == FirstStep::Check)
            && req.exec.may_match(pc)
            // SAFETY: plain reads of a live instance outside GB_run.
            && !unsafe {
                (sb::gbcli_halted(gb) && !sb::gbcli_speed_switch_halt_ends_into_pc(gb))
                    || sb::gbcli_stopped(gb)
            }
        {
            // SAFETY: side-effect-free accessors.
            let bank = unsafe { sb::gbcli_bank_for(gb, pc) };
            if req.exec.matches(pc, bank) {
                // SAFETY: a side-effect-free read of the live instance.
                let opcode = u8::try_from(unsafe { sb::gbcli_peek(gb, pc) }).ok();
                let hit = Hit {
                    access: Access::Exec,
                    addr: pc,
                    bank,
                    old: opcode,
                    new: opcode.unwrap_or(0xFF),
                    pc,
                    vetoed: false,
                };
                return Ok(stop(core, StopReason::Candidate(Hits::one(hit)), used));
            }
        }
        if arming.armed() || tapping {
            let ticks = core.ticks;
            let hooks = core.hooks_mut();
            hooks.step_pc = pc;
            hooks.step_ticks = ticks;
        }

        let before = exec_events.then(|| {
            // SAFETY: plain reads of a live instance outside GB_run.
            let (sp, bank, double_speed, halted) = unsafe {
                (
                    (*regs).sp,
                    sb::gbcli_bank_for(gb, pc),
                    sb::gbcli_double_speed(gb),
                    sb::gbcli_halted(gb) && !sb::gbcli_stopped(gb),
                )
            };
            let hooks = core.hooks_mut();
            hooks.executed = None;
            hooks.wake = halted.then_some(Wake::Halted);
            if halted && !arming.read {
                // SAFETY: installs our callback on the live instance outside GB_run.
                unsafe { sb::GB_set_read_memory_callback(gb, Some(watch::on_read)) };
            }
            Before {
                sp,
                bank,
                double_speed,
                halted,
            }
        });

        // SAFETY: no reference into the hooks is alive across this call.
        let ticks = u64::from(unsafe { sb::GB_run(gb) });
        if before.is_some_and(|b| b.halted) && !arming.read {
            // SAFETY: removes our callback outside GB_run.
            unsafe { sb::GB_set_read_memory_callback(gb, None) };
        }
        if kinds.raster
            && let Some(sink) = req.sink.as_mut()
        {
            pass_raster(core.hooks_mut(), &mut **sink);
        }
        if let (Some(before), Some(sink)) = (before, req.sink.as_mut()) {
            pass_exec(core, &mut **sink, pc, before, ticks as u32);
        }
        log_lcd_off(core, pc);
        core.ticks += ticks;
        used.ticks += ticks;
        used.instructions += 1;

        let hooks = core.hooks_mut();
        if hooks.panicked {
            hooks.panicked = false;
            return Err(CoreError::Backend {
                message: "a SameBoy callback panicked; the state may be inconsistent".into(),
            });
        }
        let hit = hooks.hit.take();
        let mut frame_ended = false;
        if let Some(kind) = hooks.vblank.take() {
            core.vblanks += 1;
            used.frames += 1;
            frame_ended = true;
            let (number, at) = (core.vblanks, Ticks(core.ticks));
            let hooks = core.hooks_mut();
            if std::mem::take(&mut hooks.frame_copied)
                && let Some(kind) = vblank_kind(kind)
            {
                hooks.frame.tag = Some(FrameTag { number, kind, at });
            }
        }
        if let Some(hits) = hit {
            return Ok(stop(core, StopReason::Candidate(hits), used));
        }
        if frame_ended && req.stop_at_frame {
            return Ok(stop(core, StopReason::Frame, used));
        }
    }
}

/// The CPU state an execution event needs from before its step.
#[derive(Clone, Copy)]
struct Before {
    sp: u16,
    bank: u16,
    double_speed: bool,
    /// In HALT (not STOP) as the step began.
    halted: bool,
}

/// Sends the step at `pc` to the sink. A step that began in HALT and left it
/// to dispatch or run an instruction is sent as two: the HALT cycles SameBoy
/// runs first in the same step (`GB_cpu_run` in sm83_cpu.c advances 2 to 4
/// cycles in its halted branch before acting) as an idle step, then the rest.
/// The speed is the one the step began at: a speed-switching step mixes both.
fn pass_exec(core: &mut SameBoy, sink: &mut dyn EventSink, pc: u16, before: Before, ticks: u32) {
    let gb = core.raw();
    // SAFETY: plain reads of a live instance outside GB_run.
    let (sp_after, next_pc) = unsafe { ((*core.regs).sp, (*core.regs).pc) };
    // SAFETY: a side-effect-free accessor on a live instance outside GB_run.
    let next_bank = unsafe { sb::gbcli_bank_for(gb, next_pc) };
    let hooks = core.hooks_mut();
    let step = match hooks.executed {
        Some(opcode) => CpuStep::Instruction { opcode },
        None if sp_after == before.sp.wrapping_sub(2) => CpuStep::Interrupt,
        None => CpuStep::Idle,
    };
    let idle = match hooks.wake.take() {
        Some(Wake::At(idle)) if step != CpuStep::Idle => idle.min(ticks),
        _ => 0,
    };
    if idle > 0 {
        sink.event(Event::Exec {
            pc,
            bank: before.bank,
            step: CpuStep::Idle,
            ticks: idle,
            sp_before: before.sp,
            sp_after: before.sp,
            next_pc: pc,
            next_bank: before.bank,
            double_speed: before.double_speed,
        });
    }
    sink.event(Event::Exec {
        pc,
        bank: before.bank,
        step,
        ticks: ticks - idle,
        sp_before: before.sp,
        sp_after,
        next_pc,
        next_bank,
        double_speed: before.double_speed,
    });
}

/// Logs an LCD-off the step at `pc` made, stamped with the step's start.
fn log_lcd_off(core: &mut SameBoy, pc: u16) {
    let (at, vblanks) = (Ticks(core.ticks), core.vblanks);
    let hooks = core.hooks_mut();
    if let (Some(line), Some(log)) = (hooks.lcd_off_line.take(), &mut hooks.lcd_off_log) {
        log.push(LcdOff {
            pc,
            line,
            at,
            vblanks,
        });
    }
}

/// Hands the step's raster events to the sink, keeping the buffer.
fn pass_raster(hooks: &mut Hooks, sink: &mut dyn EventSink) {
    if let Some(events) = &mut hooks.raster {
        for event in events.drain(..) {
            sink.event(event);
        }
    }
}

pub(crate) fn stop(core: &SameBoy, reason: StopReason, used: Usage) -> Stop {
    let gb = core.raw();
    // SAFETY: plain reads of a live instance outside GB_run.
    let (pc, bank) = unsafe {
        let pc = (*core.regs).pc;
        (pc, sb::gbcli_bank_for(gb, pc))
    };
    Stop {
        reason,
        pc,
        bank,
        at: Ticks(core.ticks),
        used,
    }
}
