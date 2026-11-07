//! The run loop: budgets, filters, frame stops, the illegal-opcode and STOP checks.
//!
//! SameBoy locks the CPU on an illegal opcode (IE=0, halted) without telling
//! anyone, so the loop checks the opcode the next step will execute before
//! running it (`gbcli_pre_run` in the shim). Execution filters are checked the
//! same way, so a breakpoint stops before its instruction runs; read and write
//! filters are checked in SameBoy's memory callbacks (`watch.rs`) and stop
//! after the instruction that made the access.

use gbcli_core::{
    Access, CoreError, CpuStep, Event, Feature, FrameTag, Hit, Hits, RunRequest, Stop, StopReason,
    Ticks, Usage,
};
use sameboy_sys as sb;

use crate::hooks::{Hooks, on_exec, vblank_kind};
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
}

impl Arming {
    fn new(core: &mut SameBoy, req: &RunRequest<'_>, exec: bool) -> Arming {
        let gb = core.raw();
        let read = !req.read.is_empty();
        let write = !(req.write.is_empty() && req.veto.is_empty() && req.stores.is_empty());
        let hooks = core.hooks_mut();
        hooks.in_run = true;
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
            if write {
                sb::GB_set_write_memory_callback(gb, Some(watch::on_write));
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
            if self.write && !hooks.write_hook_wanted() {
                sb::GB_set_write_memory_callback(self.gb, None);
            }
            if self.exec {
                hooks.executed = None;
                sb::GB_set_execution_callback(self.gb, None);
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
    if kinds.raster {
        return Err(unsupported_err(Feature::RasterEvents));
    }
    let exec_events = kinds.exec;
    let arming = Arming::new(core, req, exec_events);
    let check_exec = !req.exec.is_empty();
    let tapping = core.hooks().apu_log.is_some() || core.hooks().audio.is_some();
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
            sb::GBCLI_PRE_STOPPED => return Ok(stop(core, StopReason::Stopped, used)),
            _ => {}
        }
        // SAFETY: `regs` points into the live instance.
        let pc = unsafe { (*regs).pc };
        // A run never stops on an execution hit before its first step, so
        // resuming from a breakpoint makes progress.
        if check_exec && used.instructions > 0 && req.exec.may_match(pc) {
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
            core.hooks_mut().executed = None;
            // SAFETY: plain reads of a live instance outside GB_run.
            unsafe { ((*regs).sp, sb::gbcli_bank_for(gb, pc)) }
        });

        // SAFETY: no reference into the hooks is alive across this call.
        let ticks = u64::from(unsafe { sb::GB_run(gb) });
        if let (Some((sp_before, bank)), Some(sink)) = (before, req.sink.as_mut()) {
            // SAFETY: plain reads of a live instance outside GB_run.
            let (sp_after, next_pc, double_speed) =
                unsafe { ((*regs).sp, (*regs).pc, sb::gbcli_double_speed(gb)) };
            // SAFETY: a side-effect-free accessor on a live instance outside GB_run.
            let next_bank = unsafe { sb::gbcli_bank_for(gb, next_pc) };
            let step = match core.hooks().executed {
                Some(opcode) => CpuStep::Instruction { opcode },
                None if sp_after == sp_before.wrapping_sub(2) => CpuStep::Interrupt,
                None => CpuStep::Idle,
            };
            sink.event(Event::Exec {
                pc,
                bank,
                step,
                ticks: ticks as u32,
                sp_before,
                sp_after,
                next_pc,
                next_bank,
                double_speed,
            });
        }
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
