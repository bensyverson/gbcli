//! `doctor`, `new` and `build`: argv to the protocol's ops, which the server
//! answers (design §12), so `serve` and `batch` build exactly as the CLI
//! does. Only what argv alone can say is checked here.

use gbcli_protocol::{BuildArgs, NewArgs, Op};

use crate::Env;
use crate::argv::Invocation;
use crate::build::{Command, Target, target};
use crate::usage::Usage;

pub fn build(inv: &Invocation, env: &Env) -> Result<Command, Usage> {
    let target = target(inv, env, None)?;
    let args = BuildArgs {
        dir: inv.positionals.first().cloned(),
        src: inv.all("src").map(str::to_string).collect(),
        include: inv.all("include").map(str::to_string).collect(),
        out: inv.one("out").map(str::to_string),
        restore: inv.one("restore").map(str::to_string),
    };
    if args.restore.is_none() {
        let reload = ["session", "allow-rom-change", "rev", "dir", "as"];
        if let Some(flag) = reload.into_iter().find(|f| inv.has(f)) {
            return Err(Usage::new(
                format!("--{flag} only applies when --restore reloads the build"),
                "add --restore NAME (a checkpoint of the session), or drop it",
            ));
        }
    } else if target.session.is_none() {
        return Err(Usage::new(
            "--restore needs the session to restore into",
            "add --session NAME (or set GBCLI_SESSION), e.g. \
             `gbcli build --session game --restore start`",
        ));
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
