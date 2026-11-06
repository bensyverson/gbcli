//! `gbcli-corpus`: runs the test-ROM corpus on SameBoy and writes or checks a baseline.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use gbcli_core::{Core, CoreConfig, CoreError};
use gbcli_corpus::baseline::{self, Header};
use gbcli_corpus::judge::{self, Context, Outcome};
use gbcli_corpus::manifest::{self, Row};
use gbcli_corpus::models::{self, Target};
use gbcli_sameboy::SameBoy;

const USAGE: &str = "\
usage: gbcli-corpus [options]

Runs every manifest row on each model SameBoy has and prints per-suite totals.

options:
  --root DIR       repo root (default: the current directory)
  --suite NAME     run only this suite (repeatable)
  --skip-suite N   leave this suite out (repeatable)
  --id TEXT        run only rows whose id contains TEXT (repeatable)
  --jobs N         worker threads (default: available parallelism)
  --write FILE     write the baseline to FILE
  --check FILE     compare with a baseline; exit 1 on any difference
  --dump DIR       write the frame of each failing screenshot row to DIR
  -h, --help       this text";

/// A job slower than this (wall seconds) is named on stderr.
const SLOW_JOB_S: f64 = 20.0;

struct Args {
    root: PathBuf,
    suites: Vec<String>,
    skip_suites: Vec<String>,
    ids: Vec<String>,
    jobs: usize,
    write: Option<PathBuf>,
    check: Option<PathBuf>,
    dump: Option<PathBuf>,
}

fn parse_args() -> Result<Args, lexopt::Error> {
    use lexopt::prelude::*;
    let mut args = Args {
        root: PathBuf::from("."),
        suites: Vec::new(),
        skip_suites: Vec::new(),
        ids: Vec::new(),
        jobs: std::thread::available_parallelism().map_or(4, |n| n.get()),
        write: None,
        check: None,
        dump: None,
    };
    let mut parser = lexopt::Parser::from_env();
    while let Some(arg) = parser.next()? {
        match arg {
            Long("root") => args.root = parser.value()?.into(),
            Long("suite") => args.suites.push(parser.value()?.string()?),
            Long("skip-suite") => args.skip_suites.push(parser.value()?.string()?),
            Long("id") => args.ids.push(parser.value()?.string()?),
            Long("jobs") => args.jobs = parser.value()?.parse()?,
            Long("write") => args.write = Some(parser.value()?.into()),
            Long("check") => args.check = Some(parser.value()?.into()),
            Long("dump") => args.dump = Some(parser.value()?.into()),
            Short('h') | Long("help") => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            _ => return Err(arg.unexpected()),
        }
    }
    Ok(args)
}

fn sameboy(config: CoreConfig) -> Result<Box<dyn Core>, CoreError> {
    Ok(Box::new(SameBoy::new(config)?))
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(err) => {
            eprintln!("gbcli-corpus: {err}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(&args) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("gbcli-corpus: {err}");
            ExitCode::from(2)
        }
    }
}

fn run(args: &Args) -> Result<ExitCode, String> {
    let manifest_path = args.root.join("tests/corpus/manifest.json");
    let corpus = args.root.join("tests/corpus/fetched");
    let text = std::fs::read_to_string(&manifest_path)
        .map_err(|e| format!("{}: {e}", manifest_path.display()))?;
    let manifest = manifest::parse(&text)?;
    let jobs: Vec<(Row, Target)> = manifest
        .rows
        .into_iter()
        .filter(|r| args.suites.is_empty() || args.suites.contains(&r.suite))
        .filter(|r| !args.skip_suites.contains(&r.suite))
        .filter(|r| args.ids.is_empty() || args.ids.iter().any(|t| r.id.contains(t.as_str())))
        .flat_map(|r| {
            models::targets(&r.models)
                .into_iter()
                .map(move |t| (r.clone(), t))
        })
        .collect();
    let started = Instant::now();
    let ctx = Context {
        corpus: &corpus,
        factory: sameboy,
        dump: args.dump.as_deref(),
    };
    let outcomes = run_all(&jobs, ctx, args.jobs.max(1));
    let wall = started.elapsed().as_secs_f64();

    for (suite, t) in baseline::summarize(&outcomes) {
        println!(
            "{suite:<14} pass {:>5}  fail {:>5}  skip {:>5}",
            t.pass, t.fail, t.skip
        );
    }
    println!(
        "{} runs in {wall:.1} s on {} threads",
        outcomes.len(),
        args.jobs
    );

    if let Some(path) = &args.write {
        let header = Header {
            backend: "sameboy".into(),
            backend_commit: gbcli_sameboy_commit(),
            date: today(),
            command: command_line(),
        };
        std::fs::write(path, baseline::render(&header, &outcomes))
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    if let Some(path) = &args.check {
        let old = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let diffs = baseline::differences(&old, &outcomes)?;
        for d in &diffs {
            println!("changed: {d}");
        }
        if !diffs.is_empty() {
            return Ok(ExitCode::FAILURE);
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// Runs every job on a pool of threads; each thread builds its own cores.
fn run_all(jobs: &[(Row, Target)], ctx: Context<'_>, threads: usize) -> Vec<Outcome> {
    let next = AtomicUsize::new(0);
    let done: Mutex<Vec<(usize, Outcome)>> = Mutex::new(Vec::with_capacity(jobs.len()));
    std::thread::scope(|scope| {
        for _ in 0..threads.min(jobs.len()) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some((row, target)) = jobs.get(i) else {
                        break;
                    };
                    let started = Instant::now();
                    let outcome = judge::run(row, target, ctx);
                    let wall = started.elapsed().as_secs_f64();
                    if wall > SLOW_JOB_S {
                        eprintln!("slow: {} on {} took {wall:.0} s", row.id, outcome.model);
                    }
                    done.lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push((i, outcome));
                }
            });
        }
    });
    let mut done = done.into_inner().unwrap_or_else(|e| e.into_inner());
    done.sort_by_key(|(i, _)| *i);
    done.into_iter().map(|(_, o)| o).collect()
}

fn gbcli_sameboy_commit() -> String {
    let core = SameBoy::new(CoreConfig::new(gbcli_core::Model::DmgB, vec![0; 0x8000]))
        .expect("a blank ROM builds a core");
    core.info().version.to_string()
}

/// The command as typed, for the baseline header.
fn command_line() -> String {
    let args: Vec<String> = std::env::args().skip(1).collect();
    format!("cargo run --release -p gbcli-corpus -- {}", args.join(" "))
}

/// Today's UTC date, `YYYY-MM-DD` (civil-from-days, Howard Hinnant's algorithm).
fn today() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}
