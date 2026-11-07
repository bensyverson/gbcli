//! What a run measures besides its stop (design §11 step 5): `--measure` on a
//! copy of the stop, and `--profile`, `--frame-budget` and `--raster`,
//! collected from the run's own events so its breakpoints, budget and keys
//! still apply.

use std::cell::RefCell;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use gbcli_core::{Event, EventKinds, EventSink, Gap};
use gbcli_engine::Machine;
use gbcli_engine::profile::{FrameBudget, FrameSplit, MeasureSpec, Profile, Raster, Routines};
use gbcli_protocol::{
    ErrorBody, ErrorCode, FrameBudgetMade, FrameBudgetRow, MeasureMade, ProfileMade, RasterMade,
    RoutineRow, RunArgs,
};

use super::super::Failure;
use super::super::writes::parse_location;
use super::budget::DEFAULT_FRAMES;
use super::measure::{measure_on, spec};
use super::raster::raster_made;

/// The run's sinks: the profiler's, the frame split's and the raster
/// timeline's, any of them.
#[derive(Default)]
pub(super) struct Sinks {
    routines: Option<Routines>,
    split: Option<FrameSplit>,
    frames: Vec<FrameBudget>,
    raster: Option<Raster>,
}

impl EventSink for Sinks {
    /// What the sinks present want, so a core arms nothing else.
    fn kinds(&self) -> EventKinds {
        let all = [
            self.routines.as_ref().map(EventSink::kinds),
            self.split.as_ref().map(EventSink::kinds),
            self.raster.as_ref().map(EventSink::kinds),
        ];
        all.into_iter()
            .flatten()
            .fold(EventKinds::default(), |a, k| EventKinds {
                exec: a.exec || k.exec,
                raster: a.raster || k.raster,
                ppu_modes: a.ppu_modes || k.ppu_modes,
            })
    }

    fn event(&mut self, event: Event) {
        if let Some(r) = &mut self.routines {
            r.event(event);
        }
        if let Some(s) = &mut self.split {
            s.event(event);
        }
        if let Some(r) = &mut self.raster {
            r.event(event);
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
    measure: Option<(String, String, MeasureSpec<'static>)>,
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
        let sinks = (args.profile.is_some() || args.frame_budget || args.raster).then(|| {
            RefCell::new(Sinks {
                routines: args.profile.as_ref().map(|_| Routines::new()),
                split: args.frame_budget.then(FrameSplit::default),
                frames: Vec::new(),
                raster: args.raster.then(|| Raster::new(machine.core())),
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

    /// One measurement on a copy of the stop, if asked for; `interrupt` is
    /// the request's flag, which `guard` sets.
    pub(super) fn measure(
        &self,
        machine: &mut Machine,
        interrupt: &AtomicBool,
        guard: Duration,
    ) -> Option<Result<MeasureMade, Failure>> {
        let (from, to, spec) = self.measure.as_ref()?;
        let mut spec = spec.clone();
        spec.interrupt = Some(interrupt);
        Some(measure_on(machine, &spec, (from, to), None, guard))
    }

    /// What the run's sinks collected.
    pub(super) fn finish(&self, machine: &Machine) -> Collected {
        let Some(sinks) = &self.sinks else {
            return Collected::default();
        };
        let sinks = std::mem::take(&mut *sinks.borrow_mut());
        let profile = sinks
            .routines
            .map(|r| profile_made(r.finish(machine.symbols()), self.top));
        let frames = &sinks.frames;
        let frame_budget = sinks
            .split
            .map(|mut split| frame_budget_made(frames, split.cut()));
        let timeline = sinks.raster.map(Raster::finish);
        let gaps = timeline.as_ref().map_or(Vec::new(), |t| t.gaps.clone());
        Collected {
            profile,
            frame_budget,
            raster: timeline.as_ref().map(raster_made),
            gaps,
        }
    }
}

/// What a run's sinks collected, and the features the core lacked for them,
/// for the gap ledger.
#[derive(Default)]
pub(super) struct Collected {
    pub(super) profile: Option<ProfileMade>,
    pub(super) frame_budget: Option<FrameBudgetMade>,
    pub(super) raster: Option<RasterMade>,
    pub(super) gaps: Vec<Gap>,
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

/// The whole frames, then `rest`, the frame the run stopped in, as a partial
/// row if it has any cycles: a run that stops at a vblank has none.
fn frame_budget_made(frames: &[FrameBudget], rest: FrameBudget) -> FrameBudgetMade {
    let whole = frames.iter().map(|f| FrameBudgetRow {
        active_t: f.active_t,
        idle_t: f.idle_t,
        lag: Some(f.lag),
        partial: false,
    });
    let partial = (rest.active_t + rest.idle_t > 0).then_some(FrameBudgetRow {
        active_t: rest.active_t,
        idle_t: rest.idle_t,
        lag: None,
        partial: true,
    });
    FrameBudgetMade {
        frames: whole.chain(partial).collect(),
        lag: frames.iter().filter(|f| f.lag).count() as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raster() -> Raster {
        let config =
            gbcli_core::CoreConfig::new(gbcli_core::Model::DmgB, crate::fixture::V1.to_vec());
        let machine = Machine::new(gbcli_engine::Backend::sameboy(), config, None).expect("core");
        Raster::new(machine.core())
    }

    #[test]
    fn a_raster_only_run_asks_the_core_for_no_execution_events() {
        let sinks = Sinks {
            raster: Some(raster()),
            ..Sinks::default()
        };
        let want = EventKinds {
            exec: false,
            raster: true,
            ppu_modes: false,
        };
        assert_eq!(sinks.kinds(), want);
    }

    #[test]
    fn the_kinds_are_those_of_the_sinks_present() {
        let split = Sinks {
            split: Some(FrameSplit::default()),
            ..Sinks::default()
        };
        assert!(split.kinds().exec && !split.kinds().raster);
        let both = Sinks {
            routines: Some(Routines::new()),
            raster: Some(raster()),
            ..Sinks::default()
        };
        let k = both.kinds();
        assert!(k.exec && k.raster && !k.ppu_modes, "{k:?}");
        assert_eq!(Sinks::default().kinds(), EventKinds::default());
    }
}
