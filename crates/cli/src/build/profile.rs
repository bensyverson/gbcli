//! `measure FROM..TO` and run's `--measure`, `--profile` and `--top` → their
//! protocol args.

use gbcli_protocol::{MeasureArgs, ProfileArgs, RunMeasure};

use crate::Env;
use crate::argv::Invocation;
use crate::usage::Usage;
use crate::values;

/// `FROM..TO`: two addresses in the §7 grammar. The first `..` splits them,
/// so `Main.loop...loop` is `Main.loop` to `.loop`.
pub fn span(text: &str) -> Result<RunMeasure, Usage> {
    match text.split_once("..") {
        Some((from, to)) if !from.is_empty() && !to.is_empty() => Ok(RunMeasure {
            from: from.to_string(),
            to: to.to_string(),
        }),
        _ => Err(Usage::new(
            format!("`{text}` is not FROM..TO"),
            "give two addresses joined by `..`, e.g. MainLoop+1..MainLoop",
        )),
    }
}

/// The `measure` verb's args; its key timeline reads as `run`'s.
pub fn measure(inv: &Invocation, env: &Env, text: &str) -> Result<MeasureArgs, Usage> {
    let RunMeasure { from, to } = span(text)?;
    let number = |flag| inv.one(flag).map(|n| values::count(flag, n)).transpose();
    let iterations = number("iterations")?
        .map(|n| {
            u32::try_from(n).map_err(|_| {
                Usage::new(
                    format!("--iterations {n} is too many"),
                    "pass at most 4294967295",
                )
            })
        })
        .transpose()?;
    Ok(MeasureArgs {
        at: inv.one("at").map(str::to_string),
        from,
        to,
        start: inv.one("from").map(values::start).transpose()?,
        iterations,
        max_t: number("max")?,
        frames: number("frames")?,
        instructions: number("instructions")?,
        ticks: number("ticks")?,
        keys: super::video::keys(inv, env)?,
    })
}

/// `run --measure FROM..TO`, or `None`.
pub fn run_measure(inv: &Invocation) -> Result<Option<RunMeasure>, Usage> {
    inv.one("measure").map(span).transpose()
}

/// `run --profile [--top N]`, or `None`; `--top` without `--profile` is refused.
pub fn run_profile(inv: &Invocation) -> Result<Option<ProfileArgs>, Usage> {
    let top = inv
        .one("top")
        .map(|n| {
            values::count("top", n).and_then(|n| {
                u32::try_from(n)
                    .map_err(|_| Usage::new(format!("--top {n} is too many"), "pass a smaller N"))
            })
        })
        .transpose()?;
    match (inv.has("profile"), top) {
        (true, top) => Ok(Some(ProfileArgs { top })),
        (false, Some(_)) => Err(Usage::new(
            "--top only applies with --profile",
            "add --profile, or drop it",
        )),
        (false, None) => Ok(None),
    }
}
