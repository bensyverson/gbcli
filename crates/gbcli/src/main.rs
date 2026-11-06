//! The gbcli executable: a thin adapter over the engine and the server. Only
//! `serve` and `version` exist until the CLI leaf lands.

use std::io::{self, BufReader};
use std::path::PathBuf;
use std::process::ExitCode;

use gbcli_bundle::{Bundle, RomSource};
use gbcli_core::Model;
use gbcli_protocol::model_from_name;
use gbcli_server::{Handler, serve};
use lexopt::prelude::*;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("serve") => serve_command(args.collect()),
        Some("version") => {
            println!(
                "gbcli {} (SameBoy {})",
                env!("CARGO_PKG_VERSION"),
                &sameboy_sys::SAMEBOY_COMMIT[..7],
            );
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!(
                "gbcli: no `{other}` command yet. `gbcli serve` speaks JSON Lines over stdio; \
                 the rest of the CLI lands next."
            );
            ExitCode::from(2)
        }
        None => {
            eprintln!(
                "gbcli {} (SameBoy {}): only `serve` and `version` are built. \
                 `gbcli serve --session <name> --rom <file>` speaks JSON Lines over stdio.",
                env!("CARGO_PKG_VERSION"),
                &sameboy_sys::SAMEBOY_COMMIT[..7],
            );
            ExitCode::from(2)
        }
    }
}

struct ServeArgs {
    session: Option<String>,
    rom: Option<PathBuf>,
    sym: Option<PathBuf>,
    model: Model,
    seed: u64,
    actor: Option<String>,
    dir: Option<PathBuf>,
}

fn serve_command(args: Vec<String>) -> ExitCode {
    let parsed = match parse_serve(args) {
        Ok(parsed) => parsed,
        Err(e) => {
            eprintln!("gbcli serve: {e}");
            return ExitCode::from(2);
        }
    };
    let start = parsed
        .dir
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let bundle = match Bundle::open_or_create(&start) {
        Ok(bundle) => bundle,
        Err(e) => return store_failure(&e),
    };

    // A named session that doesn't exist yet is created from --rom.
    if let Some(name) = &parsed.session
        && let Err(e) = open_or_create_session(&bundle, name, &parsed)
    {
        return store_failure(&e);
    }

    let mut handler = Handler::new(bundle, parsed.session, parsed.actor);
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    match serve(BufReader::new(stdin.lock()), &mut out, &mut handler) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("gbcli serve: {e}");
            ExitCode::FAILURE
        }
    }
}

fn open_or_create_session(
    bundle: &Bundle,
    name: &str,
    args: &ServeArgs,
) -> Result<(), gbcli_bundle::StoreError> {
    let store = bundle.session(name)?;
    if store.exists() {
        return Ok(());
    }
    let Some(rom) = &args.rom else {
        return Err(gbcli_bundle::StoreError::NoSession {
            name: name.to_string(),
            known: bundle.session_names()?,
        });
    };
    store.create(&RomSource {
        rom: rom.clone(),
        sym: args.sym.clone(),
        model: args.model,
        seed: args.seed,
    })?;
    Ok(())
}

fn parse_serve(args: Vec<String>) -> Result<ServeArgs, String> {
    let mut parsed = ServeArgs {
        session: std::env::var("GBCLI_SESSION").ok(),
        rom: None,
        sym: None,
        model: Model::DmgB,
        seed: 0,
        actor: std::env::var("GBCLI_AS").ok(),
        dir: None,
    };
    let mut parser = lexopt::Parser::from_args(args);
    while let Some(arg) = parser.next().map_err(|e| e.to_string())? {
        match arg {
            Short('s') | Long("session") => parsed.session = Some(text_value(&mut parser)?),
            Short('r') | Long("rom") => parsed.rom = Some(PathBuf::from(text_value(&mut parser)?)),
            Long("sym") => parsed.sym = Some(PathBuf::from(text_value(&mut parser)?)),
            Long("model") => {
                let text = text_value(&mut parser)?;
                parsed.model = model_from_name(&text)
                    .ok_or_else(|| format!("unknown model {text:?}; try dmg-b or cgb-e"))?;
            }
            Long("seed") => {
                parsed.seed = text_value(&mut parser)?
                    .parse()
                    .map_err(|e| format!("--seed: {e}"))?;
            }
            Short('a') | Long("as") => parsed.actor = Some(text_value(&mut parser)?),
            Long("dir") => parsed.dir = Some(PathBuf::from(text_value(&mut parser)?)),
            Short('h') | Long("help") => {
                print_serve_help();
                std::process::exit(0);
            }
            other => {
                return Err(format!(
                    "unexpected argument {other:?}; see `gbcli serve --help`"
                ));
            }
        }
    }
    Ok(parsed)
}

fn print_serve_help() {
    println!(
        "gbcli serve — speak the JSON Lines protocol on stdin and stdout\n\
         \n\
         USAGE\n  gbcli serve [--session <name>] [--rom <file>] [--sym <file>]\n\
         \x20             [--model dmg-b|cgb-e|...] [--seed <n>] [--as <actor>] [--dir <path>]\n\
         \n\
         One request per line, one final response per request. stdout carries protocol\n\
         only; diagnostics go to stderr. A named session that doesn't exist is created\n\
         from --rom; naming an existing one opens it. The machine stays warm between\n\
         requests and reloads if another process moved the head.\n"
    );
}

fn text_value(parser: &mut lexopt::Parser) -> Result<String, String> {
    parser
        .value()
        .map_err(|e| e.to_string())?
        .string()
        .map_err(|e| e.to_string())
}

fn store_failure(e: &gbcli_bundle::StoreError) -> ExitCode {
    eprintln!("gbcli: {e}");
    eprintln!("  remedy: {}", e.remedy());
    ExitCode::from(e.code().exit_code())
}
