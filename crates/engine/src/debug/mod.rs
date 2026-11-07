//! The debugger: breakpoints and watches with conditions, stepping, and the
//! stop record, built on the core's candidate hits (design §2.1, §2.2).
//! Breakpoints are PC checks in the run loop; nothing is patched into memory.

mod backtrace;
mod code;
mod decide;
mod drive;
mod keys;
mod record;
mod until;

use std::fmt;
use std::sync::atomic::AtomicBool;

use gbcli_core::{Budget, CoreError, canonical_addr};
use gbcli_protocol::{
    BreakpointSpec, KeyEvent, Location, StepKind, StopRecord, TimeBase, TimeMark, WatchAccess,
    WatchSpec,
};

use crate::cond::{Condition, EvalError, EvalWarning, ExprError};
use crate::machine::Machine;

pub use backtrace::CallFrame;
pub use code::{core_code, eval_code, expr_code};
use drive::Goal;
pub(crate) use keys::Schedule;
pub use keys::unapplied_keys;

/// What frame and cycle counts count from (design §9). The default is the
/// boot handoff; before it has been seen, records count from power-on and
/// say so.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Since {
    PowerOn,
    #[default]
    Boot,
    Checkpoint {
        name: String,
        mark: TimeMark,
    },
}

/// How a run or step may go.
#[derive(Clone, Debug)]
pub struct RunOptions<'a> {
    /// Always finite.
    pub budget: Budget,
    pub since: Since,
    /// Conditions reading never-written RAM warn instead of failing.
    pub allow_uninit: bool,
    /// Set from elsewhere to stop the run: a user break.
    pub interrupt: Option<&'a AtomicBool>,
    /// Key presses and releases at frame offsets from the run's start.
    pub keys: &'a [KeyEvent],
    /// Called at every vblank the run passes, after the key events due there.
    pub on_frame: Option<FrameSink<'a>>,
}

/// What a run calls at each vblank, with the frames it has completed: how a
/// caller collects frames during a run (filmstrips). It only observes.
#[derive(Clone, Copy)]
pub struct FrameSink<'a>(pub &'a dyn Fn(&Machine, u64));

impl std::fmt::Debug for FrameSink<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FrameSink")
    }
}

impl RunOptions<'_> {
    pub fn new(budget: Budget) -> Self {
        RunOptions {
            budget,
            since: Since::Boot,
            allow_uninit: false,
            interrupt: None,
            keys: &[],
            on_frame: None,
        }
    }
}

/// A finished run or step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub stop: StopRecord,
    /// Never-written reads that `allow_uninit` let through, then each stop
    /// trigger left waiting on a byte still never written (design §8).
    pub warnings: Vec<EvalWarning>,
}

/// A run that couldn't finish.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DebugError {
    /// A stored condition no longer compiles (the symbols changed).
    Condition {
        id: u32,
        error: ExprError,
    },
    /// A condition failed while evaluated; `stop` is where the machine is.
    Eval {
        id: u32,
        error: EvalError,
        stop: Box<StopRecord>,
    },
    /// An `--until` predicate doesn't compile.
    UntilSyntax {
        error: ExprError,
    },
    /// An `--until` predicate failed while evaluated (not a never-written
    /// read, which means "not yet"); `stop` is where the machine is, with
    /// reason `predicate_error`.
    Until {
        error: EvalError,
        stop: Box<StopRecord>,
    },
    Core(CoreError),
    /// `step` doesn't take `ToAddress`: use `run_to`.
    NeedsAddress,
}

impl fmt::Display for DebugError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DebugError::Condition { id, error } => write!(f, "condition of #{id}: {error}"),
            DebugError::Eval { id, error, .. } => write!(f, "condition of #{id}: {error}"),
            DebugError::UntilSyntax { error } => write!(f, "until: {error}"),
            DebugError::Until { error, .. } => write!(f, "until: {error}"),
            DebugError::Core(e) => write!(f, "{e}"),
            DebugError::NeedsAddress => write!(f, "stepping to an address needs the address"),
        }
    }
}

impl std::error::Error for DebugError {}

impl From<CoreError> for DebugError {
    fn from(e: CoreError) -> Self {
        DebugError::Core(e)
    }
}

impl Machine {
    /// Sets a breakpoint; the condition is checked now so a bad one is
    /// refused when set, not when hit. Returns its id.
    pub fn add_breakpoint(
        &mut self,
        at: Location,
        condition: Option<&str>,
    ) -> Result<u32, ExprError> {
        self.compile(condition)?;
        let id = self.next_id();
        self.debugger_config_mut().breakpoints.push(BreakpointSpec {
            id,
            at: canonical(at),
            condition: condition.map(String::from),
        });
        Ok(id)
    }

    /// Sets a watch. `once` deletes it after its first stop.
    pub fn add_watch(
        &mut self,
        access: WatchAccess,
        at: Location,
        condition: Option<&str>,
        once: bool,
    ) -> Result<u32, ExprError> {
        self.compile(condition)?;
        let id = self.next_id();
        self.debugger_config_mut().watches.push(WatchSpec {
            id,
            access,
            at: canonical(at),
            condition: condition.map(String::from),
            once,
        });
        Ok(id)
    }

    /// Deletes a breakpoint or watch; false if no such id.
    pub fn delete(&mut self, id: u32) -> bool {
        let config = self.debugger_config_mut();
        let before = config.breakpoints.len() + config.watches.len();
        config.breakpoints.retain(|b| b.id != id);
        config.watches.retain(|w| w.id != id);
        before != config.breakpoints.len() + config.watches.len()
    }

    /// Runs until a breakpoint, watch or other stop, within the budget.
    pub fn run(&mut self, opts: &RunOptions<'_>) -> Result<Outcome, DebugError> {
        drive::drive(self, opts, Goal::Run)
    }

    /// Runs until `predicate` holds, evaluating it on the cadence derived from
    /// what it reads (design §8, §11). The predicate is evaluated once before
    /// the first instruction; if it already holds, no instructions run.
    pub fn run_until(
        &mut self,
        predicate: &str,
        opts: &RunOptions<'_>,
    ) -> Result<Outcome, DebugError> {
        let condition = Condition::compile(predicate, self.symbols())
            .map_err(|error| DebugError::UntilSyntax { error })?;
        drive::drive_until(self, opts, &condition)
    }

    /// Steps by `kind`; breakpoints and watches still stop a longer step.
    pub fn step(&mut self, kind: StepKind, opts: &RunOptions<'_>) -> Result<Outcome, DebugError> {
        let goal = match kind {
            StepKind::Instruction => Goal::Instruction,
            StepKind::Over => drive::over(self),
            StepKind::Out => Goal::Out {
                sp: self.core().registers().sp,
            },
            StepKind::Frame => Goal::Frame,
            StepKind::Scanline => Goal::Scanline {
                ly: self.core().ppu().ly,
            },
            StepKind::ToAddress => return Err(DebugError::NeedsAddress),
        };
        drive::drive(self, opts, goal)
    }

    /// Runs until PC reaches `at` (a temporary breakpoint).
    pub fn run_to(&mut self, at: Location, opts: &RunOptions<'_>) -> Result<Outcome, DebugError> {
        drive::drive(self, opts, Goal::To(canonical(at)))
    }

    /// What `since` counts from on this machine, and its mark, as a stop
    /// record names them: `Boot` before the handoff falls back to power-on.
    pub fn named_base(&self, since: &Since) -> (TimeBase, TimeMark) {
        record::base(self, since)
    }

    /// Return addresses found on the stack, innermost first.
    pub fn backtrace(&self) -> Vec<CallFrame> {
        backtrace::backtrace(self.core(), self.symbols())
    }

    fn compile(&self, condition: Option<&str>) -> Result<Option<Condition>, ExprError> {
        condition
            .map(|text| Condition::compile(text, self.symbols()))
            .transpose()
    }

    fn next_id(&self) -> u32 {
        let config = self.debugger_config();
        let ids = config.breakpoints.iter().map(|b| b.id);
        ids.chain(config.watches.iter().map(|w| w.id))
            .max()
            .map_or(1, |m| m + 1)
    }
}

fn canonical(at: Location) -> Location {
    Location {
        addr: canonical_addr(at.addr),
        ..at
    }
}
