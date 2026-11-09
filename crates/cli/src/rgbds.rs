//! `doctor`, `new`, `build`, `assets`, `opcode` and `version`: argv to the
//! protocol's ops, which the server answers (design §12), so `serve` and
//! `batch` build exactly as the CLI does. Only what argv alone can say is
//! checked here.

use std::path::Path;

use gbcli_protocol::{AssetManifest, AssetsArgs, BuildArgs, BuildTarget, NewArgs, Op, OpcodeArgs};

use crate::Env;
use crate::argv::Invocation;
use crate::build::{Command, Target, target};
use crate::usage::Usage;

/// The server refuses `--session`, `--rev` and `--allow-rom-change` without
/// `--restore`, and a restore with no session or one that doesn't exist,
/// exactly as it does for a `serve` client; argv adds only what never
/// reaches the wire.
pub fn build(inv: &Invocation, env: &Env) -> Result<Command, Usage> {
    let mut target = target(inv, env, None)?;
    let args = BuildArgs {
        dir: inv.positionals.first().cloned(),
        src: inv.all("src").map(str::to_string).collect(),
        include: inv.all("include").map(str::to_string).collect(),
        out: inv.one("out").map(str::to_string),
        restore: inv.one("restore").map(str::to_string),
        target: if inv.has("gbc") {
            BuildTarget::Gbc
        } else {
            BuildTarget::Dmg
        },
    };
    if args.restore.is_none() {
        // --dir and --as only steer the restore's session; neither changes
        // a build that restores nothing, so either is a mistake.
        if let Some(flag) = ["dir", "as"].into_iter().find(|f| inv.has(f)) {
            return Err(Usage::new(
                format!("--{flag} only applies when --restore reloads the build"),
                "add --restore NAME (a checkpoint of the session), or drop it",
            ));
        }
        // GBCLI_SESSION is a default, like `serve --session`, not a session
        // this build names: only the flag reaches the server.
        if !inv.has("session") {
            target.session = None;
        }
    }
    // DIR is the project, so the session to restore into is found from it,
    // unless --dir or GBCLI_DIR says where.
    if let Some(dir) = &args.dir
        && target.session.is_some()
        && target.dir.is_none()
        && env.gbcli_dir.is_none()
    {
        target.dir = Some(env.cwd.join(dir));
    }
    Ok(request(target, Op::Build(args)))
}

/// `assets`: the manifest file, read and parsed here, rides in the op as
/// the wire's struct, so a `serve` client sends what the CLI sends.
pub fn assets(inv: &Invocation, env: &Env) -> Result<Command, Usage> {
    let manifest = match inv.one("manifest") {
        Some(file) => Some(manifest(file, env)?),
        None => None,
    };
    let args = AssetsArgs {
        dir: inv.positionals.first().cloned(),
        manifest,
        target: if inv.has("gbc") {
            BuildTarget::Gbc
        } else {
            BuildTarget::Dmg
        },
        keep: inv.one("keep").map(str::to_string),
    };
    Ok(request(only_json(inv), Op::Assets(args)))
}

fn manifest(file: &str, env: &Env) -> Result<AssetManifest, Usage> {
    let text = std::fs::read_to_string(env.cwd.join(file)).map_err(|e| {
        Usage::new(
            format!("the manifest {file} can't be read: {e}"),
            "pass a JSON file naming the images (`gbcli schema`, `manifest`)",
        )
    })?;
    let mut manifest: AssetManifest = serde_json::from_str(&text).map_err(|e| {
        Usage::new(
            format!("the manifest {file} doesn't read: {e}"),
            "fix it to the shape `gbcli schema` gives for `manifest`",
        )
    })?;
    // A manifest's paths are its folder's, as in any manifest file. The
    // wire stays relative to the server's working directory, so the folder
    // is joined as typed, not made absolute.
    let folder = Path::new(file).parent().unwrap_or(Path::new(""));
    let resolve = |path: &mut String| {
        if !Path::new(path.as_str()).is_absolute() {
            *path = folder.join(&*path).to_string_lossy().into_owned();
        }
    };
    for image in &mut manifest.images {
        resolve(&mut image.file);
        image.dmg.iter_mut().for_each(resolve);
    }
    Ok(manifest)
}

pub fn new(inv: &Invocation) -> Command {
    let dir = inv.positionals[0].clone();
    request(only_json(inv), Op::New(NewArgs { dir }))
}

pub fn doctor(inv: &Invocation) -> Command {
    request(only_json(inv), Op::Doctor)
}

/// `opcode`: the words of the query, joined, so `gbcli opcode xor a` needs
/// no quotes.
pub fn opcode(inv: &Invocation) -> Command {
    let query = inv.positionals.join(" ");
    request(only_json(inv), Op::Opcode(OpcodeArgs { query }))
}

/// `version` (and `--version`): the op, so the text line and a `serve`
/// client's answer come from one place.
pub fn version(inv: &Invocation) -> Command {
    request(only_json(inv), Op::Version)
}

/// A verb with `--json` as its only flag sends no session or actor.
fn only_json(inv: &Invocation) -> Target {
    Target {
        json: inv.has("json"),
        ..Target::default()
    }
}

fn request(target: Target, op: Op) -> Command {
    Command::Request {
        target,
        op: Box::new(op),
    }
}
