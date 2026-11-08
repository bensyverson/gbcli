//! `equiv`: the session's ROM (side A) against another build or the same ROM
//! patched (side B), from equivalent starts under one key timeline, compared
//! at the end of every frame (design §11). Both sides run on copies, so the
//! head never moves; a divergence saves each side as a named checkpoint.

use std::path::Path;

use gbcli_bundle::{SessionStore, Start};
use gbcli_engine::addr::AddrError;
use gbcli_engine::equiv::{
    self, AudioCompare, Difference, Divergence, EquivError, EquivSpec, RamRange, Side, SideState,
    Verdict,
};
use gbcli_engine::{Checkpoint, Machine};
use gbcli_protocol::{
    ApuWriteMade, AudioCompareName, CheckpointMade, ComparedMade, DifferenceMade, DivergenceMade,
    EquivArgs, EquivMade, ErrorBody, ErrorCode, ResultBody, SessionFile, SideMade, SideName,
};

use super::super::history::checkpoint_frame;
use super::super::writes::apply_writes;
use super::super::{Failure, Handler};
use super::budget::wall_guard_ended;
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
        let mut a = self.side(store, meta, args, false, warnings)?;
        let mut b = self.side(store, &side_b(store, meta, args), args, true, warnings)?;
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
        let verdict = equiv::compare(&mut a, &mut b, &spec).map_err(|e| match e {
            EquivError::Interrupted { frames } => Failure::new(wall_guard_ended(
                self.wall_guard,
                &format!("after {frames} equivalent frames"),
                "compare fewer --frames per call",
            )),
            other => equiv_failure(&other),
        })?;
        // `equiv::compare` checks every frame's pixels, unconditionally.
        let compared = ComparedMade {
            screen: true,
            ram: args.ram.clone(),
            audio: args.audio.unwrap_or_default(),
        };
        match verdict {
            Verdict::Same { frames, ended } => Ok(ResultBody {
                equiv: Some(EquivMade {
                    frames,
                    compared,
                    ended,
                    divergence: None,
                }),
                ..ResultBody::default()
            }),
            Verdict::Diverged(d) => Err(diverged(store, meta, *d, compared, args.b.rom.as_deref())),
        }
    }

    /// One side's starting machine: `at` restored under the request's load
    /// options, else the side's own boot handoff (or power-on).
    fn side(
        &self,
        store: &SessionStore,
        meta: &SessionFile,
        args: &EquivArgs,
        is_b: bool,
        warnings: &mut Vec<String>,
    ) -> Result<Machine, Failure> {
        let from = args.from.unwrap_or(Start::Boot);
        let interrupt = Some(&*self.interrupt);
        let (mut machine, loaded) = match &args.at {
            Some(at) if is_b && args.b.rom.is_some() => store.start_side_b_at(meta, at, &self.load),
            Some(at) => store.start_machine(meta, Some(at), from, &self.load, interrupt),
            None => store.machine(meta).and_then(|mut m| {
                let loaded = match from {
                    Start::Boot => store.to_handoff(meta, &mut m, &self.load, interrupt)?,
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
            // Side B's symbols come from its own flag, not the session's.
            if *side == Side::B && matches!(error, AddrError::NoSymbols { .. }) {
                body.remedy = "pass --sym-b FILE (by default side B reads ROM_B's name with \
                               .sym, when that file exists), or give an address such as $C0A0"
                    .into();
            }
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
fn diverged(
    store: &SessionStore,
    meta: &SessionFile,
    d: Divergence,
    compared: ComparedMade,
    b_rom: Option<&str>,
) -> Failure {
    let save = |name: &str, side: SideState| -> Result<SideMade, Failure> {
        Ok(SideMade {
            checkpoint: save_side(store, meta, name, &side.checkpoint)?,
            stop: side.stop,
        })
    };
    let [a_name, b_name] = SIDE_CHECKPOINTS;
    let remedy = next_step(&d, b_rom);
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
        compared,
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
        remedy,
    );
    body.result = Some(Box::new(ResultBody {
        equiv: Some(made),
        ..ResultBody::default()
    }));
    Failure::new(body)
}

/// Where to look next. A restore rebuilds the core from the session's ROM
/// (plus the checkpoint's patches), so side B restores only when it ran the
/// session's ROM; a side B built from another ROM would run on A's code, or
/// be refused as `rom_changed`.
/// A byte that was never written comes first: until the program sets it,
/// stepping either side through the divergence shows nothing to fix.
fn next_step(d: &Divergence, b_rom: Option<&str>) -> String {
    unwritten_cause(d).unwrap_or_else(|| restore_step(d, b_rom))
}

fn restore_step(d: &Divergence, b_rom: Option<&str>) -> String {
    let [a_name, b_name] = SIDE_CHECKPOINTS;
    if d.a.checkpoint.header.rom_sha256 == d.b.checkpoint.header.rom_sha256 {
        return format!("`gbcli restore {b_name}` to see side B where it went wrong");
    }
    let rom = b_rom.unwrap_or("another ROM");
    format!(
        "side B ran {rom}, which this session can't restore, so its stop is in the \
         result; `gbcli restore {a_name}` shows side A where it went wrong"
    )
}

/// The likeliest cause when a compared byte still holds its power-on value:
/// the program hasn't initialized it yet, and power-on RAM is random, so the
/// two sides' bytes (at different addresses when the build moved them)
/// differ by chance rather than by any fault of the rewrite. A checkpoint
/// carries side A's RAM layout, so `--at` can start both sides past the
/// program's init only while the byte sits at one address on both.
fn unwritten_cause(d: &Divergence) -> Option<String> {
    d.differences.iter().find_map(|x| match x {
        Difference::Ram {
            range,
            never_written,
            a_at,
            b_at,
            ..
        } => {
            let sides = sides(never_written)?;
            let moved = (a_at.addr, a_at.bank) != (b_at.addr, b_at.bank);
            let skip = if moved {
                "initialize it before the first vblank, e.g. clear WRAM at startup (--at \
                 can't skip the init here: a checkpoint carries side A's RAM layout, and \
                 this build moved it)"
            } else {
                "start the comparison after the init (--at a checkpoint saved after it), \
                 or initialize it earlier"
            };
            Some(match never_written.as_slice() {
                [one] => {
                    let other = if *one == Side::A { "side B" } else { "side A" };
                    format!(
                        "{range} was never written on {sides}, though {other} wrote it, \
                         so {sides} holds random power-on RAM there: if {sides} sets it \
                         later by design, {skip}; else {sides} is missing the write"
                    )
                }
                _ => format!(
                    "{range} was never written on {sides} (power-on RAM is random, so \
                     the two bytes differ by chance): {skip}"
                ),
            })
        }
        _ => None,
    })
}

/// "side A", "side B" or "both sides"; `None` for no side.
fn sides(of: &[Side]) -> Option<&'static str> {
    match of {
        [] => None,
        [Side::A] => Some("side A"),
        [Side::B] => Some("side B"),
        _ => Some("both sides"),
    }
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
        Difference::Ram {
            range,
            offset,
            never_written,
            ..
        } => match sides(never_written) {
            Some(sides) => format!("{range}+{offset}, never written on {sides}"),
            None => format!("{range}+{offset}"),
        },
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
            never_written,
        } => DifferenceMade::Ram {
            range,
            offset,
            a_at,
            b_at,
            a,
            b,
            count: count as u64,
            never_written: never_written
                .into_iter()
                .map(|side| match side {
                    Side::A => SideName::A,
                    Side::B => SideName::B,
                })
                .collect(),
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
