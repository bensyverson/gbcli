//! An [`Invocation`] → the [`Command`] to carry out: for most verbs a
//! protocol [`Request`], the same struct `gbcli serve` reads.

use std::path::PathBuf;

use gbcli_core::Model;
use gbcli_protocol::{
    BreakArgs, CheckpointArgs, DeleteArgs, EventsArgs, Op, PruneArgs, ReadArgs, Request,
    RestoreArgs, Rev, RmArgs, RunArgs, ShotReadArgs, StepArgs, StepKind, SymArgs, WatchAccess,
    WatchArgs, WriteArgs, WriteKind, model_from_name,
};
use gbcli_protocol::{PowerCycleArgs, SavArgs};

use crate::Env;
use crate::argv::Invocation;
use crate::help::Topic;
use crate::usage::Usage;
use crate::values;
use crate::verbs::{Verb, VerbSpec, by_name};

/// What one invocation does.
#[derive(Debug)]
pub enum Command {
    Primer,
    VerbHelp(&'static VerbSpec),
    Topic(Topic),
    Recipes,
    Schema,
    Serve(Target),
    Batch { target: Target, file: String },
    // Boxed: an Op is several hundred bytes and the other commands are small.
    Request { target: Target, op: Box<Op> },
    Song(Box<crate::song::SongCommand>),
}

/// Where a request goes and how its answer is printed: the flags every
/// session verb shares.
#[derive(Debug, Default)]
pub struct Target {
    pub session: Option<String>,
    pub actor: Option<String>,
    pub rev: Option<Rev>,
    pub allow_rom_change: bool,
    pub json: bool,
    /// Where to look for `.gbcli/`; it beats `GBCLI_DIR`.
    pub dir: Option<PathBuf>,
    /// Create the session from this when it doesn't exist.
    pub create: Option<Create>,
}

/// A new session's ROM and settings (`--rom`, `--sym`, `--model`, `--seed`, `--flash`).
/// A setting not given takes the default for a new session and is not
/// checked against an existing one.
#[derive(Debug)]
pub struct Create {
    pub rom: PathBuf,
    pub sym: Option<PathBuf>,
    pub model: Option<Model>,
    pub seed: Option<u64>,
    /// The flash board, by profile name; the bundle checks the name.
    pub flash: Option<String>,
}

impl Target {
    /// The request this target sends `op` as.
    pub fn request(&self, op: Op) -> Request {
        Request {
            id: "cli".into(),
            session: self.session.clone(),
            actor: self.actor.clone(),
            rev: self.rev,
            allow_rom_change: self.allow_rom_change,
            op,
        }
    }
}

pub fn build(inv: Invocation, env: &Env) -> Result<Command, Usage> {
    let spec = inv.spec;
    let pos = |i: usize| inv.positionals[i].clone();
    let op = match spec.verb {
        Verb::Help => return help(inv.positionals.first().map(String::as_str)),
        Verb::Recipes => return Ok(Command::Recipes),
        Verb::Schema => return Ok(Command::Schema),
        Verb::Version => return Ok(crate::rgbds::version(&inv)),
        Verb::Doctor => return Ok(crate::rgbds::doctor(&inv)),
        Verb::Opcode => return Ok(crate::rgbds::opcode(&inv)),
        Verb::New => return Ok(crate::rgbds::new(&inv)),
        Verb::Build => return crate::rgbds::build(&inv, env),
        Verb::Assets => return crate::rgbds::assets(&inv, env),
        Verb::Song => return Ok(crate::song::command(&inv, env)),
        Verb::Pixels => return Ok(crate::rgbds::pixels(&inv)),
        Verb::Font => return crate::rgbds::font(&inv),
        Verb::Serve => return Ok(Command::Serve(target(&inv, env, None)?)),
        Verb::Batch => {
            let Some(file) = inv.one("file") else {
                return Err(Usage::new(
                    "`gbcli batch` needs -F FILE",
                    "`gbcli batch -F ops.jsonl`, or -F - to read stdin",
                ));
            };
            return Ok(Command::Batch {
                file: file.to_string(),
                target: target(&inv, env, None)?,
            });
        }
        Verb::Run => Op::Run(run(&inv, env)?),
        Verb::Shot => Op::Shot(ShotReadArgs {
            shot: video::shot(&inv, &inv.positionals[0])?,
            since: inv.one("since").map(values::since),
        }),
        Verb::View => Op::View(video::view(&inv)?),
        Verb::Sym => Op::Sym(SymArgs { pattern: pos(0) }),
        Verb::Checkpoints => Op::Checkpoints,
        Verb::Sav => Op::Sav(SavArgs { path: pos(0) }),
        Verb::PowerCycle => Op::PowerCycle(PowerCycleArgs {
            sav: inv.one("sav").map(str::to_string),
            rtc_advance: inv.one("rtc-advance").map(str::to_string),
        }),
        Verb::Measure => Op::Measure(profile::measure(&inv, env, &pos(0))?),
        Verb::Equiv => Op::Equiv(Box::new(equiv::equiv(&inv, env)?)),
        Verb::Step => Op::Step(StepArgs {
            unit: match inv.positionals.first() {
                Some(unit) => values::step_unit(unit)?,
                None => StepKind::Instruction,
            },
        }),
        Verb::Show => Op::Show,
        Verb::Read => Op::Read(ReadArgs {
            reads: inv.positionals.clone(),
            since: inv.one("since").map(values::since),
            allow_uninit: inv.has("allow-uninit"),
        }),
        Verb::Break => Op::Break(BreakArgs {
            at: pos(0),
            condition: inv.one("if").map(str::to_string),
        }),
        Verb::Watch => Op::Watch(WatchArgs {
            access: access(&inv.positionals[0])?,
            at: pos(1),
            condition: inv.one("if").map(str::to_string),
            once: inv.has("once"),
        }),
        Verb::Delete => Op::Delete(DeleteArgs {
            id: inv.positionals[0].parse().map_err(|_| {
                Usage::new(
                    format!("`{}` isn't a breakpoint or watch id", inv.positionals[0]),
                    "ids are the numbers `break` and `watch` print, e.g. `gbcli delete 1`",
                )
            })?,
        }),
        Verb::Checkpoint => Op::Checkpoint(CheckpointArgs { name: pos(0) }),
        Verb::Restore => Op::Restore(RestoreArgs { name: pos(0) }),
        Verb::Undo => Op::Undo,
        Verb::Write | Verb::Poke | Verb::Patch => {
            let kind = match spec.verb {
                Verb::Write => WriteKind::Bus,
                Verb::Poke => WriteKind::Poke,
                _ => WriteKind::Patch,
            };
            let writes = inv
                .positionals
                .iter()
                .map(|text| values::write(kind, text))
                .collect::<Result<_, _>>()?;
            Op::Write(WriteArgs { writes })
        }
        Verb::Sessions => Op::Sessions,
        Verb::Gaps => Op::Gaps,
        Verb::Prune => Op::Prune(PruneArgs {
            unused_for: inv
                .one("unused-for")
                .ok_or_else(|| {
                    Usage::new(
                        "`gbcli sessions prune` needs --unused-for DURATION",
                        "e.g. `gbcli sessions prune --unused-for 7d`; it never prunes by default",
                    )
                })?
                .to_string(),
        }),
        Verb::Rm => Op::Rm(RmArgs {
            name: pos(0),
            with_checkpoints: inv.has("with-checkpoints"),
        }),
        Verb::Events => Op::Events(EventsArgs {
            limit: inv
                .one("limit")
                .map(|n| values::count("limit", n).map(|n| n as usize))
                .transpose()?,
        }),
    };
    // `run game.gb` is `run --rom game.gb`.
    let rom = match spec.verb {
        Verb::Run => inv.positionals.first().cloned(),
        _ => None,
    };
    if let Some(rom) = &rom {
        refuse_expression_as_rom(rom, &inv, env)?;
    }
    if rom.is_some() && inv.has("rom") {
        return Err(Usage::new(
            "the ROM is given twice: as an argument and as --rom",
            "give it once",
        ));
    }
    Ok(Command::Request {
        target: target(&inv, env, rom)?,
        op: Box::new(op),
    })
}

fn help(topic: Option<&str>) -> Result<Command, Usage> {
    let Some(word) = topic else {
        return Ok(Command::Primer);
    };
    match word {
        "recipes" => return Ok(Command::Recipes),
        "schema" => return Ok(Command::Schema),
        _ => {}
    }
    // A topic named like its verb teaches the verb's format (pixels), so it
    // comes first; the verb's flags stay under `gbcli VERB --help`.
    if let Some(topic) = Topic::from_name(word) {
        return Ok(Command::Topic(topic));
    }
    by_name(word).map(Command::VerbHelp).ok_or_else(|| {
        let names = Topic::ALL.iter().map(|t| t.name());
        let near = crate::usage::near(word, names);
        Usage::new(
            format!("no help topic `{word}`"),
            format!(
                "{}topics: {}; `gbcli help VERB` for a command",
                crate::usage::did_you_mean(&near, "gbcli help "),
                Topic::ALL.map(|t| t.name()).join(", ")
            ),
        )
    })
}

fn run(inv: &Invocation, env: &Env) -> Result<RunArgs, Usage> {
    let budget = |flag| inv.one(flag).map(|n| values::count(flag, n)).transpose();
    let mut writes = Vec::new();
    for (flag, value) in &inv.flags {
        let kind = match flag.long {
            "write" => WriteKind::Bus,
            "poke" => WriteKind::Poke,
            "patch" => WriteKind::Patch,
            _ => continue,
        };
        writes.push(values::write(kind, value)?);
    }
    Ok(RunArgs {
        at: inv.one("at").map(str::to_string),
        from: inv.one("from").map(values::start).transpose()?,
        since: inv.one("since").map(values::since),
        until: inv.one("until").map(str::to_string),
        to: inv.one("to").map(str::to_string),
        frames: budget("frames")?,
        instructions: budget("instructions")?,
        ticks: budget("ticks")?,
        writes,
        read: inv.all("read").map(str::to_string).collect(),
        expect: inv.all("expect").map(str::to_string).collect(),
        keys_since: video::keys_since(inv)?,
        checkpoint: inv.one("checkpoint").map(str::to_string),
        allow_uninit: inv.has("allow-uninit"),
        shot: video::run_shot(inv)?.map(Box::new),
        audio: audio::run_audio(inv)?.map(Box::new),
        filmstrip: video::run_filmstrip(inv)?.map(Box::new),
        measure: profile::run_measure(inv)?.map(Box::new),
        profile: profile::run_profile(inv)?,
        frame_budget: inv.has("frame-budget"),
        raster: inv.has("raster"),
        oam_log: inv.has("oam-log"),
        keys: video::keys(inv, env)?,
    })
}

/// The file names a ROM argument may end in.
const ROM_EXTENSIONS: [&str; 4] = ["gb", "gbc", "cgb", "sgb"];

/// A second expression after `--read` (or `--expect`) lands where run's ROM
/// argument goes, and the session then refused "another ROM", far from the
/// cause. So an argument that names no file, has no ROM's extension and
/// holds an expression's brackets or operators is refused here. A real ROM
/// named `Tetris [!].gb` still runs, as does any file that exists.
fn refuse_expression_as_rom(rom: &str, inv: &Invocation, env: &Env) -> Result<(), Usage> {
    let path = std::path::Path::new(rom);
    let rom_like = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| ROM_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()));
    // Not `-` or `/`, which real paths hold.
    let expression_like = rom.contains([
        '[', ']', '=', '<', '>', '!', '&', '|', '+', '*', '(', ')', '~', '^',
    ]);
    if rom_like || !expression_like || env.cwd.join(path).is_file() {
        return Ok(());
    }
    let (read, expect) = (inv.has("read"), inv.has("expect"));
    let (message, remedy) = match (read, expect) {
        (true, false) => (
            "--read takes one expression".to_string(),
            format!("repeat --read for each expression, e.g. --read '{rom}'"),
        ),
        (false, true) => (
            "--expect takes one condition".to_string(),
            format!("repeat --expect for each condition, e.g. --expect '{rom}'"),
        ),
        // Both, or neither: it can't tell which the stray followed.
        _ => (
            "--read and --expect each take one".to_string(),
            format!(
                "repeat --read for each expression and --expect for each condition, e.g. \
                 --read '{rom}' or --expect '{rom}'"
            ),
        ),
    };
    Err(Usage::new(
        format!("`{rom}` looks like an expression, not a ROM: {message}"),
        remedy,
    ))
}

fn access(text: &str) -> Result<WatchAccess, Usage> {
    match text {
        "read" => Ok(WatchAccess::Read),
        "write" => Ok(WatchAccess::Write),
        "access" => Ok(WatchAccess::Access),
        other => Err(Usage::new(
            format!("`{other}` isn't read, write or access"),
            "e.g. `gbcli watch write wFrame`",
        )),
    }
}

/// The shared flags, with the environment's defaults (`GBCLI_SESSION`,
/// `GBCLI_AS`). `rom` is a ROM given as an argument.
pub(crate) fn target(inv: &Invocation, env: &Env, rom: Option<String>) -> Result<Target, Usage> {
    let rom = rom.or_else(|| inv.one("rom").map(str::to_string));
    let create = match rom {
        Some(rom) => Some(Create {
            rom: env.cwd.join(rom),
            sym: inv.one("sym").map(|p| env.cwd.join(p)),
            model: inv
                .one("model")
                .map(|name| {
                    model_from_name(name).ok_or_else(|| {
                        Usage::new(
                            format!("unknown model `{name}`"),
                            "dmg-b (or dmg), mgb, sgb, sgb2, cgb0 (or cgb), cgb-a … cgb-e, \
                             gbc (cgb-e) or agb",
                        )
                    })
                })
                .transpose()?,
            seed: inv
                .one("seed")
                .map(|n| values::count("seed", n))
                .transpose()?,
            flash: inv.one("flash").map(str::to_string),
        }),
        None => {
            if let Some(flag) = ["sym", "model", "seed", "flash"]
                .into_iter()
                .find(|f| inv.has(f))
            {
                return Err(Usage::new(
                    format!("--{flag} only applies when a ROM creates a session"),
                    "add --rom FILE (or the ROM argument to run), or drop it",
                ));
            }
            None
        }
    };
    Ok(Target {
        // GBCLI_SESSION reaches only verbs that take a session, so it can't
        // make `gbcli sessions` fail over a session that is gone.
        session: inv
            .one("session")
            .map(str::to_string)
            .or_else(|| inv.spec.flag("session").and_then(|_| env.session.clone())),
        actor: inv
            .one("as")
            .map(str::to_string)
            .or_else(|| env.actor.clone()),
        rev: inv
            .one("rev")
            .map(|r| {
                r.parse()
                    .map_err(|e: String| Usage::new(format!("--rev: {e}"), "e.g. `--rev s6`"))
            })
            .transpose()?,
        allow_rom_change: inv.has("allow-rom-change"),
        json: inv.has("json"),
        // `--dir` already moved the working directory there (`Env::anchored`).
        dir: inv.has("dir").then(|| env.cwd.clone()),
        create,
    })
}

mod audio;
mod equiv;
mod profile;
mod video;

#[cfg(test)]
mod flag_tests;
#[cfg(test)]
mod tests;
