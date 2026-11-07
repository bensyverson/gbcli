//! `equiv`: the session's ROM (side A) against another build or the same ROM
//! patched (side B), from equivalent starts under one key timeline, compared
//! at the end of every frame (design §11). Both sides run on copies, so the
//! head never moves; a divergence saves each side as a named checkpoint.

use std::path::Path;

use gbcli_bundle::{SessionStore, Start};
use gbcli_engine::equiv::{
    self, AudioCompare, Difference, Divergence, EquivError, EquivSpec, RamRange, SideState, Verdict,
};
use gbcli_engine::{Checkpoint, Machine};
use gbcli_protocol::{
    ApuWriteMade, AudioCompareName, CheckpointMade, DifferenceMade, DivergenceMade, EquivArgs,
    EquivMade, ErrorBody, ErrorCode, ResultBody, SessionFile, SideMade,
};

use super::super::history::checkpoint_frame;
use super::super::writes::apply_writes;
use super::super::{Failure, Handler};
use crate::convert;

/// Where a divergence leaves each side, overwritten by the next one.
pub(in crate::handle) const SIDE_CHECKPOINTS: [&str; 2] = ["equiv-a", "equiv-b"];

impl Handler {
    pub(in crate::handle) fn equiv(
        &mut self,
        store: &SessionStore,
        meta: &SessionFile,
        args: &EquivArgs,
        warnings: &mut Vec<String>,
    ) -> Result<ResultBody, Failure> {
        if args.frames == 0 {
            return Err(bad("equiv compares at least one frame", "pass --frames N"));
        }
        if args.at.is_some() && args.from.is_some() {
            return Err(bad(
                "equiv starts from --at or --from, not both",
                "drop one of them: --at names the state both sides start from",
            ));
        }
        let a = self.side(store, meta, args, warnings)?;
        let mut b = self.side(store, &side_b(store, meta, args), args, warnings)?;
        apply_writes(&mut b, &args.b.writes)?;
        let ram: Vec<RamRange> = args
            .ram
            .iter()
            .map(|r| RamRange {
                at: r.at.clone(),
                len: r.len,
            })
            .collect();
        let interrupt = self.interrupt.clone();
        let spec = EquivSpec {
            frames: args.frames,
            keys: &args.keys,
            ram: &ram,
            audio: match args.audio.unwrap_or_default() {
                AudioCompareName::Off => AudioCompare::Off,
                AudioCompareName::Writes => AudioCompare::Writes,
                AudioCompareName::Timed => AudioCompare::Timed,
            },
            interrupt: Some(&interrupt),
        };
        let verdict = equiv::compare(&a, &b, &spec).map_err(|e| match e {
            EquivError::Interrupted { frames } => Failure::new(ErrorBody::new(
                ErrorCode::BudgetExhausted,
                format!(
                    "the {:?} wall guard ended the check after {frames} equivalent frames",
                    self.wall_guard
                ),
                "compare fewer --frames per call",
            )),
            other => equiv_failure(&other),
        })?;
        match verdict {
            Verdict::Same { frames, ended } => Ok(ResultBody {
                equiv: Some(EquivMade {
                    frames,
                    ended,
                    divergence: None,
                }),
                ..ResultBody::default()
            }),
            Verdict::Diverged(d) => Err(diverged(store, meta, *d)),
        }
    }

    /// One side's starting machine: `at` restored under the request's load
    /// options, else the side's own boot handoff (or power-on).
    fn side(
        &self,
        store: &SessionStore,
        meta: &SessionFile,
        args: &EquivArgs,
        warnings: &mut Vec<String>,
    ) -> Result<Machine, Failure> {
        let from = args.from.unwrap_or(Start::Boot);
        let interrupt = Some(&*self.interrupt);
        let (mut machine, loaded) = match &args.at {
            Some(at) => store.start_machine(meta, Some(at), from, &self.load, interrupt),
            None => store.machine(meta).and_then(|mut m| {
                let loaded = match from {
                    Start::Boot => store.to_handoff(&mut m, &self.load, interrupt)?,
                    Start::PowerOn => Vec::new(),
                };
                Ok((m, loaded))
            }),
        }
        .map_err(|e| Failure::from_store(&e))?;
        warnings.extend(loaded);
        *machine.debugger_config_mut() = Default::default();
        Ok(machine)
    }
}

/// Side B's session settings: its ROM and `.sym` (the ROM's sibling `.sym`
/// when it names none), or the session's own.
fn side_b(store: &SessionStore, meta: &SessionFile, args: &EquivArgs) -> SessionFile {
    let mut b = meta.clone();
    if let Some(rom) = &args.b.rom {
        b.rom = rom.clone();
        b.sym = args.b.sym.clone().or_else(|| {
            let sibling = store.bundle().project().join(rom).with_extension("sym");
            sibling
                .is_file()
                .then(|| sibling.to_string_lossy().into_owned())
        });
    } else if let Some(sym) = &args.b.sym {
        b.sym = Some(sym.clone());
    }
    b
}

/// The check's refusals, as error bodies.
pub(in crate::handle) fn equiv_failure(e: &EquivError) -> Failure {
    match e {
        EquivError::Start(m) => bad(
            &m.to_string(),
            "start both sides from their boot handoffs (--from boot), or from one \
             checkpoint (--at NAME)",
        ),
        EquivError::Range { side, at, error } => {
            let mut body = convert::addr(error);
            body.message = format!("--ram {at} on side {side:?}: {}", body.message);
            Failure::new(body)
        }
        EquivError::Run { error, .. } => Failure::from_debug(error),
        EquivError::Core { error, .. } => Failure::from_core(error),
        EquivError::Interrupted { frames } => Failure::new(ErrorBody::plain(
            ErrorCode::BudgetExhausted,
            format!("interrupted after {frames} equivalent frames"),
        )),
    }
}

/// A divergence: both sides saved as named checkpoints, and `predicate_false`
/// carrying the verdict.
fn diverged(store: &SessionStore, meta: &SessionFile, d: Divergence) -> Failure {
    let save = |name: &str, side: SideState| -> Result<SideMade, Failure> {
        Ok(SideMade {
            checkpoint: save_side(store, meta, name, &side.checkpoint)?,
            stop: side.stop,
        })
    };
    let [a_name, b_name] = SIDE_CHECKPOINTS;
    let sides = save(a_name, d.a).and_then(|a| Ok((a, save(b_name, d.b)?)));
    let (a, b) = match sides {
        Ok(sides) => sides,
        Err(failure) => return failure,
    };
    let summary = d
        .differences
        .first()
        .map(describe)
        .unwrap_or_else(|| "something".into());
    let made = EquivMade {
        frames: d.frame - 1,
        ended: None,
        divergence: Some(Box::new(DivergenceMade {
            frame: d.frame,
            ticks: d.ticks,
            differences: d.differences.into_iter().map(difference).collect(),
            a,
            b,
        })),
    };
    let mut body = ErrorBody::new(
        ErrorCode::PredicateFalse,
        format!(
            "the sides diverge at frame {}: {summary}; saved as {a_name} and {b_name}",
            d.frame
        ),
        format!("`gbcli restore {b_name}` to see side B where it went wrong"),
    );
    body.result = Some(Box::new(ResultBody {
        equiv: Some(made),
        ..ResultBody::default()
    }));
    Failure::new(body)
}

fn save_side(
    store: &SessionStore,
    meta: &SessionFile,
    name: &str,
    cp: &Checkpoint,
) -> Result<CheckpointMade, Failure> {
    store
        .write_checkpoint(name, cp)
        .map_err(|e| Failure::from_store(&e))?;
    let path = store.dir().join("checkpoints").join(format!("{name}.gbck"));
    Ok(CheckpointMade {
        name: name.to_string(),
        path: store.display_path(Path::new(&path)),
        frame: Some(checkpoint_frame(cp)),
        rev: meta.rev,
    })
}

fn describe(d: &Difference) -> String {
    match d {
        Difference::Stop { .. } => "one side stopped".into(),
        Difference::Pixels { x, y, count, .. } => {
            format!("{count} pixels, the first at ({x}, {y})")
        }
        Difference::Ram { range, offset, .. } => format!("{range}+{offset}"),
        Difference::Apu { index, .. } => format!("sound write {index}"),
    }
}

fn difference(d: Difference) -> DifferenceMade {
    let apu = |w: Option<gbcli_core::ApuWrite>| {
        w.map(|w| ApuWriteMade {
            ticks: w.at.0,
            pc: w.pc,
            addr: w.addr,
            value: w.value,
            effect: w.effect,
        })
    };
    match d {
        Difference::Stop { a, b } => DifferenceMade::Stop { a, b },
        Difference::Pixels { x, y, a, b, count } => DifferenceMade::Pixels {
            x,
            y,
            a,
            b,
            count: count as u64,
        },
        Difference::Ram {
            range,
            offset,
            a_at,
            b_at,
            a,
            b,
            count,
        } => DifferenceMade::Ram {
            range,
            offset,
            a_at,
            b_at,
            a,
            b,
            count: count as u64,
        },
        Difference::Apu { index, a, b } => DifferenceMade::Apu {
            index: index as u64,
            a: apu(a),
            b: apu(b),
        },
    }
}

fn bad(message: &str, remedy: &str) -> Failure {
    Failure::new(ErrorBody::new(ErrorCode::BadRequest, message, remedy))
}
