//! `measure`: cycles from each arrival at one address to the next arrival at
//! another, on a copy of `--at` or the head (design §11). The head never
//! moves, so nothing is saved and the revision stays.

use std::time::Duration;

use gbcli_bundle::SessionStore;
use gbcli_core::Budget;
use gbcli_engine::profile::{CpuSpeed, Leg, MeasureError, MeasureSpec, Measurement};
use gbcli_engine::{Machine, unapplied_keys};
use gbcli_protocol::{
    CpuSpeedName, CycleStats, ErrorBody, ErrorCode, KeyEvent, Location, MeasureArgs, MeasureMade,
    PercentStats, Real, ResultBody, SessionFile,
};

use super::super::writes::parse_location;
use super::super::{Failure, Handler, Need};
use super::budget::{WALL_GUARD_REMEDY, budget_from, wall_guard_ended};

impl Handler {
    pub(in crate::handle) fn measure(
        &mut self,
        store: &SessionStore,
        meta: &SessionFile,
        args: &MeasureArgs,
        warnings: &mut Vec<String>,
    ) -> Result<ResultBody, Failure> {
        let iterations = args.iterations.unwrap_or(1);
        if iterations == 0 {
            return Err(failure(&MeasureError::NoIterations, self.wall_guard));
        }
        if args.at.is_some() && args.start.is_some() {
            return Err(Failure::new(ErrorBody::new(
                ErrorCode::BadRequest,
                "a measurement starts from --at or --from, not both",
                "drop one of them: --at names the state to start from",
            )));
        }
        let (mut machine, loaded) =
            self.machine_for(store, meta, Need::Start(args.start), args.at.as_deref())?;
        warnings.extend(loaded);
        let (budget, _, _) = budget_from(args.frames, args.instructions, args.ticks);
        let interrupt = self.interrupt.clone();
        let guard = self.wall_guard;
        let measured = parse_location(&machine, &args.from)
            .and_then(|from| Ok((from, parse_location(&machine, &args.to)?)))
            .and_then(|(from, to)| {
                let mut spec = spec(from, to, iterations, budget);
                spec.interrupt = Some(&interrupt);
                spec.keys = &args.keys;
                let names = (args.from.as_str(), args.to.as_str());
                measure_on(&mut machine, &spec, names, args.max_t, guard)
            });
        // Only the head's machine may stay warm: `--at` and `--from` loaded
        // another state.
        if args.at.is_none() && args.start.is_none() {
            self.keep_warm(store, meta, machine);
        }
        let (measure, frames) = measured?;
        Ok(ResultBody {
            measure: Some(measure),
            keys_unapplied: unapplied(&args.keys, frames),
            ..ResultBody::default()
        })
    }
}

/// The key events the measurement ended before, for the result, as a run
/// reports them.
fn unapplied(keys: &[KeyEvent], frames: u64) -> Option<Vec<KeyEvent>> {
    let late = unapplied_keys(keys, frames);
    (!late.is_empty()).then_some(late)
}

/// A spec without an interrupt flag or keys: the caller sets the request's.
pub(super) fn spec(
    from: Location,
    to: Location,
    iterations: u32,
    budget: Budget,
) -> MeasureSpec<'static> {
    MeasureSpec {
        from,
        to,
        iterations,
        budget,
        interrupt: None,
        keys: &[],
    }
}

/// Measures `spec` on a copy of `machine`, `(from, to)` as the request named
/// them; with `max_t`, a slowest iteration over it is `predicate_false`
/// carrying the measurement. `guard` is the wall guard that sets the spec's
/// interrupt flag. With the measurement, the frames it ran.
pub(super) fn measure_on(
    machine: &mut Machine,
    spec: &MeasureSpec,
    (from, to): (&str, &str),
    max_t: Option<u64>,
    guard: Duration,
) -> Result<(MeasureMade, u64), Failure> {
    let m = machine.measure(spec).map_err(|e| failure(&e, guard))?;
    let made = made(&m, from, to, max_t);
    match max_t {
        Some(max) if m.exceeds(max) => {
            let mut body = ErrorBody::new(
                ErrorCode::PredicateFalse,
                format!(
                    "the slowest of {} iterations took {} T, more than --max {max}",
                    made.n, made.t.max
                ),
                format!("make the code from {from} to {to} faster, or raise --max"),
            );
            body.result = Some(Box::new(ResultBody {
                measure: Some(made),
                ..ResultBody::default()
            }));
            Err(Failure::new(body))
        }
        _ => Ok((made, m.frames)),
    }
}

fn made(m: &Measurement, from: &str, to: &str, max_t: Option<u64>) -> MeasureMade {
    let stats = |s: gbcli_engine::profile::Stats| CycleStats {
        min: s.min,
        max: s.max,
        mean: Real(s.mean),
    };
    let (min, max, mean) = m.frame_percent();
    MeasureMade {
        from: from.to_string(),
        to: to.to_string(),
        n: m.n() as u64,
        speed: match m.speed {
            CpuSpeed::Single => CpuSpeedName::Single,
            CpuSpeed::Double => CpuSpeedName::Double,
        },
        t: stats(m.t()),
        m: stats(m.m()),
        frame_pct: PercentStats {
            min: Real(min),
            max: Real(max),
            mean: Real(mean),
        },
        max_t,
    }
}

fn failure(e: &MeasureError, guard: Duration) -> Failure {
    let (code, remedy) = match e {
        // The wall guard is the only thing that sets a request's flag.
        MeasureError::Interrupted { completed, leg } => {
            let leg = match leg {
                Leg::ToStart => "before reaching --from",
                Leg::ToEnd => "before reaching --to",
            };
            let reached = format!("{leg} after {completed} iterations");
            return Failure::new(wall_guard_ended(guard, &reached, WALL_GUARD_REMEDY));
        }
        MeasureError::NoIterations => (ErrorCode::BadRequest, "pass --iterations 1 or more"),
        // Exit 1: the measurement ran, and its answer is that T-cycles mean
        // two things across the span.
        MeasureError::SpeedSwitch { .. } => (
            ErrorCode::PredicateFalse,
            "measure a span on one side of the speed switch: move --from past the switch's \
             STOP, or --to before it",
        ),
        MeasureError::Budget { .. } => (
            ErrorCode::BudgetExhausted,
            "raise --frames, or check that the code reaches --from and --to with \
             `gbcli run --to ADDR`",
        ),
        MeasureError::Halted { .. } => (
            ErrorCode::PredicateFalse,
            "the next arrival can never come; check the path with `gbcli run --to ADDR`",
        ),
        MeasureError::Core(core) => return Failure::from_core(core),
    };
    Failure::new(ErrorBody::new(code, e.to_string(), remedy))
}
