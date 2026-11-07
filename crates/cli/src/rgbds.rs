//! `doctor`, `new`, `build` and `opcode`: argv to the protocol's ops,
//! which the server answers (design §12), so `serve` and `batch` build
//! exactly as the CLI does. Only what argv alone can say is checked here.

use gbcli_protocol::{BuildArgs, NewArgs, Op, OpcodeArgs};

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
    Ok(request(target, Op::Build(args)))
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
