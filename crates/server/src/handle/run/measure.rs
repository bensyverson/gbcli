//! `measure`: cycles from each arrival at one address to the next arrival at
//! another, on a copy of `--at` or the head (design §11). The head never
//! moves, so nothing is saved and the revision stays.

use gbcli_bundle::{SessionStore, Start};
use gbcli_core::Budget;
use gbcli_engine::Machine;
use gbcli_engine::profile::{CpuSpeed, MeasureError, MeasureSpec, Measurement};
use gbcli_protocol::{
    CpuSpeedName, CycleStats, ErrorBody, ErrorCode, Location, MeasureArgs, MeasureMade,
    PercentStats, Real, ResultBody, SessionFile,
};

use super::super::writes::parse_location;
use super::super::{Failure, Handler, Need};
use super::budget::budget_from;

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
            return Err(failure(&MeasureError::NoIterations));
        }
        let (mut machine, loaded) =
            self.machine_for(store, meta, Need::Start(Start::Boot), args.at.as_deref())?;
        warnings.extend(loaded);
        let (budget, _, _) = budget_from(args.frames, args.instructions, args.ticks);
        let measured = parse_location(&machine, &args.from)
            .and_then(|from| Ok((from, parse_location(&machine, &args.to)?)))
            .and_then(|(from, to)| {
                let spec = spec(from, to, iterations, budget);
                measure_on(&mut machine, &spec, &args.from, &args.to, args.max_t)
            });
        // Only the head's machine may stay warm: `--at` loaded another state.
        if args.at.is_none() {
            self.keep_warm(store, meta, machine);
        }
        Ok(ResultBody {
            measure: Some(measured?),
            ..ResultBody::default()
        })
    }
}

pub(super) fn spec(from: Location, to: Location, iterations: u32, budget: Budget) -> MeasureSpec {
    MeasureSpec {
        from,
        to,
        iterations,
        budget,
    }
}

/// Measures `spec` on a copy of `machine`; with `max_t`, a slowest iteration
/// over it is `predicate_false` carrying the measurement.
pub(super) fn measure_on(
    machine: &mut Machine,
    spec: &MeasureSpec,
    from: &str,
    to: &str,
    max_t: Option<u64>,
) -> Result<MeasureMade, Failure> {
    let m = machine.measure(spec).map_err(|e| failure(&e))?;
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
        _ => Ok(made),
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

fn failure(e: &MeasureError) -> Failure {
    let (code, remedy) = match e {
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
