//! What a run measures besides its stop (design §11 step 5): `--measure` on a
//! copy of the stop, and `--profile` and `--frame-budget`, collected from the
//! run's own execution events so its breakpoints, budget and keys still apply.

use std::cell::RefCell;

use gbcli_core::{Event, EventKinds, EventSink};
use gbcli_engine::Machine;
use gbcli_engine::profile::{FrameBudget, FrameSplit, MeasureSpec, Profile, Routines};
use gbcli_protocol::{
    ErrorBody, ErrorCode, FrameBudgetMade, FrameBudgetRow, MeasureMade, ProfileMade, RoutineRow,
    RunArgs,
};

use super::super::Failure;
use super::super::writes::parse_location;
use super::budget::DEFAULT_FRAMES;
use super::measure::{measure_on, spec};

/// The run's sinks: the profiler's and the frame split's, either or both.
#[derive(Default)]
pub(super) struct Sinks {
    routines: Option<Routines>,
    split: Option<FrameSplit>,
    frames: Vec<FrameBudget>,
}

impl EventSink for Sinks {
    fn kinds(&self) -> EventKinds {
        EventKinds {
            exec: true,
            ..EventKinds::default()
        }
    }

    fn event(&mut self, event: Event) {
        if let Some(r) = &mut self.routines {
            r.event(event);
        }
        if let Some(s) = &mut self.split {
            s.event(event);
        }
    }
}

impl Sinks {
    /// Ends the frame in progress at a vblank.
    pub(super) fn cut(&mut self) {
        if let Some(split) = &mut self.split {
            self.frames.push(split.cut());
        }
    }
}

/// The probes a run asked for, checked and resolved before it moves.
pub(super) struct Probes {
    measure: Option<(String, String, MeasureSpec)>,
    top: Option<u32>,
    pub(super) sinks: Option<RefCell<Sinks>>,
}

impl Probes {
    pub(super) fn plan(machine: &Machine, args: &RunArgs) -> Result<Probes, Failure> {
        if args.profile.as_ref().is_some_and(|p| p.top == Some(0)) {
            return Err(Failure::new(ErrorBody::new(
                ErrorCode::BadRequest,
                "--top 0 would list no routines",
                "pass --top 1 or more, or drop it for every routine",
            )));
        }
        let measure = match &args.measure {
            Some(m) => {
                let from = parse_location(machine, &m.from)?;
                let to = parse_location(machine, &m.to)?;
                let budget = gbcli_core::Budget::frames(DEFAULT_FRAMES);
                Some((m.from.clone(), m.to.clone(), spec(from, to, 1, budget)))
            }
            None => None,
        };
        let sinks = (args.profile.is_some() || args.frame_budget).then(|| {
            RefCell::new(Sinks {
                routines: args.profile.as_ref().map(|_| Routines::new()),
                split: args.frame_budget.then(FrameSplit::default),
                frames: Vec::new(),
            })
        });
        Ok(Probes {
            measure,
            top: args.profile.as_ref().and_then(|p| p.top),
            sinks,
        })
    }

    /// Whether a sink must see every vblank, to cut the frames.
    pub(super) fn wants_frames(&self) -> bool {
        self.sinks
            .as_ref()
            .is_some_and(|s| s.borrow().split.is_some())
    }

    /// One measurement on a copy of the stop, if asked for.
    pub(super) fn measure(&self, machine: &Machine) -> Option<Result<MeasureMade, Failure>> {
        let (from, to, spec) = self.measure.as_ref()?;
        Some(measure_on(machine, spec, from, to, None))
    }

    /// The profile and the frame budget the run collected.
    pub(super) fn finish(
        &self,
        machine: &Machine,
    ) -> (Option<ProfileMade>, Option<FrameBudgetMade>) {
        let Some(sinks) = &self.sinks else {
            return (None, None);
        };
        let sinks = std::mem::take(&mut *sinks.borrow_mut());
        let profile = sinks
            .routines
            .map(|r| profile_made(r.finish(machine.symbols()), self.top));
        let budget = sinks.split.map(|_| frame_budget_made(&sinks.frames));
        (profile, budget)
    }
}

fn profile_made(p: Profile, top: Option<u32>) -> ProfileMade {
    let keep = top.map_or(p.routines.len(), |n| n as usize);
    let omitted = p.routines.len().saturating_sub(keep) as u64;
    let routines = p
        .routines
        .into_iter()
        .take(keep)
        .map(|r| RoutineRow {
            name: r.name,
            bank: r.entry.bank,
            addr: r.entry.addr,
            calls: r.calls,
            exclusive_t: r.exclusive_t,
            inclusive_t: r.inclusive_t,
        })
        .collect();
    ProfileMade {
        routines,
        omitted,
        total_t: p.total_t,
        idle_t: p.idle_t,
        unwound: p.shadow.unwound,
        mismatched: p.shadow.mismatched,
        ret_jumps: p.shadow.ret_jumps,
        open: p.shadow.open,
    }
}

fn frame_budget_made(frames: &[FrameBudget]) -> FrameBudgetMade {
    FrameBudgetMade {
        frames: frames
            .iter()
            .map(|f| FrameBudgetRow {
                active_t: f.active_t,
                idle_t: f.idle_t,
                lag: f.lag,
            })
            .collect(),
        lag: frames.iter().filter(|f| f.lag).count() as u64,
    }
}
