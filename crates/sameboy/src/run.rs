//! The run loop: budgets, filters, frame stops, the illegal-opcode and STOP checks.
//!
//! SameBoy locks the CPU on an illegal opcode (IE=0, halted) without telling
//! anyone, so the loop checks the opcode the next step will execute before
//! running it (`gbcli_pre_run` in the shim). Execution filters are checked the
//! same way, so a breakpoint stops before its instruction runs; read and write
//! filters are checked in SameBoy's memory callbacks (`watch.rs`) and stop
//! after the instruction that made the access.

use gbcli_core::{
    Access, CoreError, Feature, FrameTag, Hit, RunRequest, Stop, StopReason, Ticks, Usage,
};
use sameboy_sys as sb;

use crate::hooks::{Hooks, vblank_kind};
use crate::watch::{self, Armed};
use crate::{SameBoy, unsupported_err};

/// Arms the memory callbacks for one run and disarms them when dropped, on
/// every exit path, so no callback can see the request's filters after it ends.
struct Arming {
    gb: *mut sb::GB_gameboy_t,
    hooks: *mut Hooks,
    read: bool,
    write: bool,
}

impl Arming {
    fn new(core: &mut SameBoy, req: &RunRequest<'_>) -> Arming {
        let gb = core.raw();
        let read = !req.read.is_empty();
        let write = !(req.write.is_empty() && req.veto.is_empty());
        let hooks = core.hooks_mut();
        if read || write {
            hooks.armed = Some(Armed {
                read: req.read,
                write: req.write,
                veto: req.veto,
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
        }
        Arming {
            gb,
            hooks,
            read,
            write,
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
            if self.read {
                sb::GB_set_read_memory_callback(self.gb, None);
            }
            if self.write && hooks.tracking.is_none() {
                sb::GB_set_write_memory_callback(self.gb, None);
            }
        }
    }
}

pub(crate) fn run(core: &mut SameBoy, req: &mut RunRequest<'_>) -> Result<Stop, CoreError> {
    if req.sink.is_some() {
        return Err(unsupported_err(Feature::EventSinks));
    }
    let arming = Arming::new(core, req);
    let check_exec = !req.exec.is_empty();
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
                return Ok(stop(core, StopReason::Candidate(hit), used));
            }
        }
        if arming.armed() {
            core.hooks_mut().step_pc = pc;
        }

        // SAFETY: no reference into the hooks is alive across this call.
        let ticks = u64::from(unsafe { sb::GB_run(gb) });
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
        if let Some(hit) = hit {
            return Ok(stop(core, StopReason::Candidate(hit), used));
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
