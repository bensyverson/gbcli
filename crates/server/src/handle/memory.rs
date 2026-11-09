//! `view memory` (design §7, §12): a block of bytes read from the head as
//! `read` reads each one, so the bank rules, the side-effect-free IO reads
//! and the never-written check are read's own. The head stays where it was.

use gbcli_bundle::SessionStore;
use gbcli_engine::cond::{BlockStart, EvalCtx, EvalWarning, MAX_BLOCK};
use gbcli_engine::{Machine, Since};
use gbcli_protocol::{ErrorBody, ErrorCode, MemoryMade, ResultBody, SessionFile, ViewArgs};

use super::build::Host;
use super::reads::warning_text;
use super::shot::write_atomically;
use super::{Failure, Handler, Need};
use crate::convert;

impl Handler {
    pub(super) fn view_memory(
        &mut self,
        store: &SessionStore,
        meta: &SessionFile,
        args: &ViewArgs,
        warnings: &mut Vec<String>,
    ) -> Result<ResultBody, Failure> {
        let (text, len) = checked(args)?;
        let (machine, loaded) = self.machine_for(store, meta, Need::Head, None)?;
        warnings.extend(loaded);
        let made = block(&machine, text, len, args, &self.host, warnings);
        self.keep_warm(store, meta, machine);
        Ok(ResultBody {
            memory: Some(made?),
            ..ResultBody::default()
        })
    }
}

/// The start and length, refused before anything loads when missing, out of
/// range, or with a VRAM view's options.
fn checked(args: &ViewArgs) -> Result<(&str, u32), Failure> {
    if args.png.is_some() {
        return Err(bad(
            "the memory view is text only",
            "drop --png; --out FILE writes the raw bytes",
        ));
    }
    if args.map.is_some() {
        return Err(bad(
            "only the map view takes a map choice",
            "drop --map, or view map",
        ));
    }
    let (Some(text), Some(len)) = (args.addr.as_deref(), args.len) else {
        return Err(bad(
            "the memory view needs a start and a length",
            "e.g. `gbcli view memory wMap 16` (addr and len on the wire)",
        ));
    };
    if !(1..=MAX_BLOCK).contains(&len) {
        return Err(bad(
            &format!("a memory view reads 1 to {MAX_BLOCK} bytes, not {len}"),
            "give LEN as a count of bytes, e.g. 16",
        ));
    }
    Ok((text, len))
}

fn block(
    machine: &Machine,
    text: &str,
    len: u32,
    args: &ViewArgs,
    host: &Host,
    warnings: &mut Vec<String>,
) -> Result<MemoryMade, Failure> {
    let start = BlockStart::compile(text, machine.symbols())
        .map_err(|e| Failure::new(convert::expr(&e)))?;
    let ctx = EvalCtx {
        core: machine.core(),
        symbols: machine.symbols(),
        access: None,
        base: machine.time_base(&Since::Boot),
        allow_uninit: args.allow_uninit,
    };
    let mut read: Vec<EvalWarning> = Vec::new();
    let block = start.read(len, &ctx, &mut read).map_err(|e| Failure {
        body: Box::new(convert::eval(&e)),
        gap: convert::gap_of_eval(&e),
    })?;
    warnings.extend(read.iter().map(warning_text));
    if let Some(out) = &args.out {
        write_atomically(&host.resolve(out), &block.bytes)?;
    }
    Ok(MemoryMade {
        expr: text.to_string(),
        addr: block.addr,
        bank: block.bank,
        bytes: block.bytes,
        out: args.out.clone(),
    })
}

fn bad(message: &str, remedy: &str) -> Failure {
    Failure::new(ErrorBody::new(ErrorCode::BadRequest, message, remedy))
}
