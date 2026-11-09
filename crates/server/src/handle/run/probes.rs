//! What a run measures besides its stop (design §11 step 5): `--measure` on a
//! copy of the stop, and `--profile`, `--frame-budget` and `--raster`,
//! collected from the run's own events so its breakpoints, budget and keys
//! still apply.

use std::cell::RefCell;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use gbcli_core::{Event, EventKinds, EventSink, Gap};
use gbcli_engine::profile::{FrameBudget, FrameSplit, MeasureSpec, Profile, Raster, Routines};
use gbcli_engine::{Machine, Since};
use gbcli_protocol::{
    CycleStats, ErrorBody, ErrorCode, FrameBudgetMade, FrameBudgetRow, MeasureMade, ProfileMade,
    RasterMade, Real, RoutineRow, RunArgs,
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
    /// The run started mid-frame, so its first frame is only a tail.
    started_mid_frame: bool,
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
    /// Ends the frame in progress at a vblank, where `m` is.
    pub(super) fn cut(&mut self, m: &Machine) {
        if let Some(split) = &mut self.split {
            self.frames.push(split.cut(m));
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
                split: args.frame_budget.then(|| FrameSplit::starting(machine)),
                started_mid_frame: !machine.at_frame_start(),
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
        Some(measure_on(machine, &spec, (from, to), None, guard).map(|(made, _)| made))
    }

    /// What the run's sinks collected, frames numbered from `since`.
    pub(super) fn finish(&self, machine: &Machine, since: &Since) -> Collected {
        let Some(sinks) = &self.sinks else {
            return Collected::default();
        };
        let sinks = std::mem::take(&mut *sinks.borrow_mut());
        let profile = sinks
            .routines
            .map(|r| profile_made(r.finish(machine.symbols()), self.top));
        let frames = &sinks.frames;
        let base = machine.time_base(since).frame;
        let frame_budget = sinks.split.map(|mut split| {
            let rest = split.cut(machine);
            frame_budget_made(frames, rest, sinks.started_mid_frame, base)
        });
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

/// The frames the run ended at a vblank, then `rest`, the frame the run
/// stopped in, as a partial row if it has any cycles: a run that stops at a
/// vblank has none. When the run `started_mid_frame`, its first ended frame
/// is only that frame's tail, so it is partial too. Only the whole frames
/// are counted as lag or LCD-off, and only the whole LCD-on frames make the
/// summary and the busiest (a loading frame isn't play); `busiest` is a row
/// index. Each row's frame counts from `base`, the time base's frame.
fn frame_budget_made(
    frames: &[FrameBudget],
    rest: FrameBudget,
    started_mid_frame: bool,
    base: u64,
) -> FrameBudgetMade {
    let row = |f: &FrameBudget, partial: bool| FrameBudgetRow {
        frame: f.frame as i64 - base as i64,
        active_t: f.active_t,
        idle_t: f.idle_t,
        lag: (!partial).then_some(f.lag),
        lcd_off: f.lcd_off,
        partial,
    };
    let first_whole = usize::from(started_mid_frame && !frames.is_empty());
    let rows = frames
        .iter()
        .enumerate()
        .map(|(i, f)| row(f, i < first_whole))
        .chain((rest.active_t + rest.idle_t > 0).then(|| row(&rest, true)));
    let whole = &frames[first_whole..];
    let shown = || {
        whole
            .iter()
            .enumerate()
            .filter(|(_, f)| !f.lcd_off)
            .map(|(i, f)| (first_whole + i, f.active_t))
    };
    let busiest = shown().rev().max_by_key(|&(_, t)| t).map(|(i, _)| i as u64);
    let active = || shown().map(|(_, t)| t);
    let active_t = active()
        .min()
        .zip(active().max())
        .map(|(min, max)| CycleStats {
            min,
            max,
            mean: Real(active().sum::<u64>() as f64 / active().count() as f64),
        });
    FrameBudgetMade {
        frames: rows.collect(),
        lag: whole.iter().filter(|f| f.lag).count() as u64,
        lcd_off: whole.iter().filter(|f| f.lcd_off).count() as u64,
        active_t,
        busiest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn machine() -> Machine {
        let config =
            gbcli_core::CoreConfig::new(gbcli_core::Model::DmgB, crate::fixture::V1.to_vec());
        Machine::new(gbcli_engine::Backend::sameboy(), config, None).expect("core")
    }

    fn raster() -> Raster {
        Raster::new(machine().core())
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
            split: Some(FrameSplit::starting(&machine())),
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

    fn frame(active_t: u64, lag: bool) -> FrameBudget {
        FrameBudget {
            active_t,
            idle_t: 70_224 - active_t,
            lag,
            lcd_off: false,
            frame: 0,
        }
    }

    #[test]
    fn the_budget_sums_up_the_whole_frames_active_cycles_and_names_the_busiest() {
        let frames = [frame(100, false), frame(400, true), frame(400, true)];
        let rest = FrameBudget {
            active_t: 9_000,
            idle_t: 0,
            lag: false,
            lcd_off: false,
            frame: 0,
        };
        let made = frame_budget_made(&frames, rest, false, 0);
        // The partial frame (9,000 T) is in the rows, not the summary.
        assert_eq!(
            made.active_t,
            Some(CycleStats {
                min: 100,
                max: 400,
                mean: Real(300.0),
            })
        );
        assert_eq!(made.busiest, Some(1), "the first of the tied frames");
        assert_eq!(made.lag, 2);
    }

    #[test]
    fn a_run_started_mid_frame_leaves_its_first_frame_out_and_indexes_rows() {
        let frames = [frame(70_000, true), frame(100, false), frame(400, true)];
        let rest = FrameBudget::default();
        let made = frame_budget_made(&frames, rest, true, 0);
        assert!(made.frames[0].partial && made.frames[0].lag.is_none());
        assert_eq!(made.frames.len(), 3, "no trailing row at a vblank stop");
        assert_eq!(made.active_t.map(|s| (s.min, s.max)), Some((100, 400)));
        assert_eq!(made.busiest, Some(2), "a row index, past the partial row");
        assert_eq!(made.lag, 1);
    }

    #[test]
    fn a_budget_with_no_whole_frame_has_no_summary() {
        let rest = FrameBudget {
            active_t: 9_000,
            idle_t: 0,
            lag: false,
            lcd_off: false,
            frame: 0,
        };
        let made = frame_budget_made(&[], rest, false, 0);
        assert_eq!((made.active_t, made.busiest), (None, None));
    }

    #[test]
    fn lcd_off_frames_are_counted_apart_and_left_out_of_the_summary() {
        let off = |active_t| FrameBudget {
            lcd_off: true,
            ..frame(active_t, false)
        };
        let frames = [
            off(70_224),
            off(60_000),
            frame(100, false),
            frame(300, false),
        ];
        let made = frame_budget_made(&frames, FrameBudget::default(), false, 0);
        assert_eq!((made.lag, made.lcd_off), (0, 2));
        assert_eq!(made.active_t.map(|s| (s.min, s.max)), Some((100, 300)));
        assert_eq!(made.busiest, Some(3), "a frame of play, not a loading one");
        assert!(made.frames[0].lcd_off && !made.frames[2].lcd_off);
        assert_eq!(made.frames[0].lag, Some(false));
    }

    #[test]
    fn a_budget_of_only_lcd_off_frames_has_no_summary() {
        let off = FrameBudget {
            lcd_off: true,
            ..frame(70_224, false)
        };
        let made = frame_budget_made(&[off, off], FrameBudget::default(), false, 0);
        assert_eq!((made.active_t, made.busiest, made.lcd_off), (None, None, 2));
    }
}
